use super::*;
use chaos_realpath::AbsolutePathBuf;
use opentelemetry_http::HttpClient;
use pretty_assertions::assert_eq;
use rama::bytes::Bytes;
use rama::crypto::dep::rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use rama::tls::client::{TlsClientAuth, TlsServerTrust};
use rama::tls::rustls::dep::tokio_rustls::TlsAcceptor;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Builder;

const TIMEOUT_VAR: &str = "OTEL_EXPORTER_OTLP_TIMEOUT";

struct TlsFixture {
    dir: TempDir,
    ca: CertificateDer<'static>,
    server_certificate: CertificateDer<'static>,
    server_key: PrivateKeyDer<'static>,
    client_certificate: CertificateDer<'static>,
}

impl TlsFixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("TLS fixture directory");
        let ca_key = KeyPair::generate().expect("CA key");
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "OTLP Test CA");
        let ca = ca_params.self_signed(&ca_key).expect("CA certificate");
        let issuer = Issuer::from_params(&ca_params, &ca_key);

        let server_key = KeyPair::generate().expect("server key");
        let mut server_params = CertificateParams::new(vec!["127.0.0.1".into()])
            .expect("server certificate parameters");
        server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let server = server_params
            .signed_by(&server_key, &issuer)
            .expect("server certificate");

        let client_key = KeyPair::generate().expect("client key");
        let mut client_params = CertificateParams::default();
        client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let client = client_params
            .signed_by(&client_key, &issuer)
            .expect("client certificate");

        std::fs::write(dir.path().join("ca.pem"), ca.pem()).expect("write CA");
        std::fs::write(
            dir.path().join("client.pem"),
            format!("{}{}", client.pem(), ca.pem()),
        )
        .expect("write client chain");
        std::fs::write(dir.path().join("client.key"), client_key.serialize_pem())
            .expect("write client key");
        Self {
            dir,
            ca: ca.der().clone(),
            server_certificate: server.der().clone(),
            server_key: rustls::pki_types::PrivatePkcs8KeyDer::from(server_key.serialize_der())
                .into(),
            client_certificate: client.der().clone(),
        }
    }

    fn path(&self, name: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(self.dir.path().join(name)).expect("absolute fixture path")
    }

    fn config(&self, client_auth: bool) -> OtelTlsConfig {
        OtelTlsConfig {
            ca_certificate: Some(self.path("ca.pem")),
            client_certificate: client_auth.then(|| self.path("client.pem")),
            client_private_key: client_auth.then(|| self.path("client.key")),
        }
    }

    async fn server(
        &self,
        require_client_auth: bool,
    ) -> (String, tokio::task::JoinHandle<Result<bool, String>>) {
        let builder = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("server TLS versions");
        let builder = if require_client_auth {
            let mut roots = rustls::RootCertStore::empty();
            roots.add(self.ca.clone()).expect("server client CA");
            let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                Arc::new(roots),
                Arc::new(rustls::crypto::ring::default_provider()),
            )
            .build()
            .expect("mTLS verifier");
            builder.with_client_cert_verifier(verifier)
        } else {
            builder.with_no_client_auth()
        };
        let config = builder
            .with_single_cert(
                vec![self.server_certificate.clone(), self.ca.clone()],
                self.server_key.clone_key(),
            )
            .expect("server identity");
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("TLS test listener");
        let url = format!(
            "https://{}/v1/traces",
            listener.local_addr().expect("listener address")
        );
        let client_certificate = self.client_certificate.clone();
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.map_err(|error| error.to_string())?;
            let mut stream = acceptor
                .accept(socket)
                .await
                .map_err(|error| error.to_string())?;
            let authenticated = stream
                .get_ref()
                .1
                .peer_certificates()
                .is_some_and(|chain| chain.first() == Some(&client_certificate));
            let mut request = Vec::new();
            loop {
                let byte = stream.read_u8().await.map_err(|error| error.to_string())?;
                request.push(byte);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
                if request.len() > 8192 {
                    return Err("request headers too large".into());
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .map_err(|error| error.to_string())?;
            stream.flush().await.map_err(|error| error.to_string())?;
            Ok(authenticated)
        });
        (url, task)
    }
}

#[test]
fn tls_config_preserves_native_roots_and_installs_the_whole_client_chain() {
    let fixture = TlsFixture::new();
    let config = build_tls_config(&fixture.config(true)).expect("TLS config");
    let extensions = config.as_extensions();
    let trust = extensions
        .get_ref::<TlsServerTrust>()
        .expect("custom server trust");
    assert_eq!(trust.roots(), TlsServerTrust::default().roots());
    assert_eq!(
        trust
            .additional_anchors()
            .expect("additional CA")
            .certificates(),
        std::slice::from_ref(&fixture.ca)
    );
    let auth = extensions
        .get_ref::<TlsClientAuth>()
        .expect("client authentication");
    let ClientAuth::Single(auth) = &auth.0 else {
        panic!("expected configured client identity");
    };
    assert_eq!(
        auth.cert_chain,
        vec![fixture.client_certificate, fixture.ca]
    );
}

#[test]
fn tls_config_rejects_incomplete_client_identity_and_unreadable_files() {
    let fixture = TlsFixture::new();
    for tls in [
        OtelTlsConfig {
            client_certificate: Some(fixture.path("client.pem")),
            ..Default::default()
        },
        OtelTlsConfig {
            client_private_key: Some(fixture.path("client.key")),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            build_tls_config(&tls),
            Err(OtelHttpClientError::IncompleteClientIdentity)
        ));
    }
    let tls = OtelTlsConfig {
        ca_certificate: Some(fixture.path("missing.pem")),
        ..Default::default()
    };
    assert!(matches!(
        build_http_client(&tls, TIMEOUT_VAR),
        Err(OtelHttpClientError::ReadFile { .. })
    ));
    assert!(matches!(
        build_async_http_client(Some(&tls), TIMEOUT_VAR),
        Err(OtelHttpClientError::ReadFile { .. })
    ));
}

#[test]
fn tls_config_rejects_empty_malformed_and_invalid_certificates() {
    let fixture = TlsFixture::new();
    for (name, contents) in [
        ("empty.pem", ""),
        (
            "malformed.pem",
            "-----BEGIN CERTIFICATE-----\n!invalid!\n-----END CERTIFICATE-----\n",
        ),
        (
            "invalid.pem",
            "-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\n",
        ),
    ] {
        std::fs::write(fixture.dir.path().join(name), contents).expect("write invalid CA");
        let tls = OtelTlsConfig {
            ca_certificate: Some(fixture.path(name)),
            ..Default::default()
        };
        assert!(
            build_tls_config(&tls).is_err(),
            "{name} must not be ignored"
        );
    }
}

#[test]
fn tls_config_rejects_malformed_and_mismatched_private_keys() {
    let fixture = TlsFixture::new();
    let mut tls = fixture.config(true);
    std::fs::write(fixture.dir.path().join("bad.key"), "not a private key").expect("write bad key");
    tls.client_private_key = Some(fixture.path("bad.key"));
    assert!(matches!(
        build_tls_config(&tls),
        Err(OtelHttpClientError::Pem { .. })
    ));

    let other_key = KeyPair::generate().expect("unrelated private key");
    std::fs::write(
        fixture.dir.path().join("other.key"),
        other_key.serialize_pem(),
    )
    .expect("write unrelated key");
    tls.client_private_key = Some(fixture.path("other.key"));
    assert!(matches!(
        build_tls_config(&tls),
        Err(OtelHttpClientError::ClientIdentity(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_export_client_trusts_custom_ca_without_disabling_verification() {
    let fixture = TlsFixture::new();
    let client = build_http_client(&fixture.config(false), TIMEOUT_VAR).expect("custom CA client");
    let (url, server) = fixture.server(false).await;
    let request = http::Request::builder()
        .uri(url)
        .body(Bytes::new())
        .expect("OTLP request");
    let response = tokio::time::timeout(Duration::from_secs(5), client.send_bytes(request))
        .await
        .expect("TLS request deadline")
        .expect("custom CA accepted");
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.body().as_ref(), b"ok");
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("server deadline")
            .expect("server task")
            .expect("server exchange")
    );

    let client = build_http_client(&OtelTlsConfig::default(), TIMEOUT_VAR).expect("default client");
    let (url, server) = fixture.server(false).await;
    let request = http::Request::builder()
        .uri(url)
        .body(Bytes::new())
        .expect("OTLP request");
    assert!(
        tokio::time::timeout(Duration::from_secs(5), client.send_bytes(request))
            .await
            .expect("TLS request deadline")
            .is_err(),
        "untrusted CA must be rejected"
    );
    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_http_export_client_authenticates_with_configured_mtls_identity() {
    let fixture = TlsFixture::new();
    let client =
        build_async_http_client(Some(&fixture.config(true)), TIMEOUT_VAR).expect("mTLS client");
    let (url, server) = fixture.server(true).await;
    let request = http::Request::builder()
        .uri(url)
        .body(Bytes::new())
        .expect("OTLP request");
    let response = tokio::time::timeout(Duration::from_secs(5), client.send_bytes(request))
        .await
        .expect("mTLS request deadline")
        .expect("mTLS authentication");
    assert_eq!(response.status(), http::StatusCode::OK);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("server deadline")
            .expect("server task")
            .expect("server exchange")
    );

    let client = build_async_http_client(Some(&fixture.config(false)), TIMEOUT_VAR)
        .expect("anonymous client");
    let (url, server) = fixture.server(true).await;
    let request = http::Request::builder()
        .uri(url)
        .body(Bytes::new())
        .expect("OTLP request");
    assert!(
        tokio::time::timeout(Duration::from_secs(5), client.send_bytes(request))
            .await
            .expect("mTLS request deadline")
            .is_err(),
        "mTLS must reject anonymous clients"
    );
    server.abort();
    let _ = server.await;
}

#[test]
fn current_tokio_runtime_is_multi_thread_detects_runtime_flavor() {
    assert!(!current_tokio_runtime_is_multi_thread());

    let current_thread_runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime");
    assert_eq!(
        current_thread_runtime.block_on(async { current_tokio_runtime_is_multi_thread() }),
        false
    );

    let multi_thread_runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("multi-thread runtime");
    assert_eq!(
        multi_thread_runtime.block_on(async { current_tokio_runtime_is_multi_thread() }),
        true
    );
}
