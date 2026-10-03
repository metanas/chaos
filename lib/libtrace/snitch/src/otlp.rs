use crate::config::OtelTlsConfig;
use crate::rama_otel_client::RamaOtelClient;
use rama::tls::client::{ClientAuth, ClientAuthData, TlsClientConfig};
use rama::tls::rustls::dep::rustls;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub(crate) enum OtelHttpClientError {
    #[error("failed to read OTLP TLS file {path}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid PEM in OTLP TLS file {path}")]
    Pem {
        path: PathBuf,
        #[source]
        source: rustls::pki_types::pem::Error,
    },
    #[error("OTLP TLS certificate file {0} contains no certificates")]
    EmptyCertificates(PathBuf),
    #[error("invalid certificate in OTLP TLS file {path}")]
    Certificate {
        path: PathBuf,
        #[source]
        source: rustls::Error,
    },
    #[error("OTLP mTLS requires both client_certificate and client_private_key")]
    IncompleteClientIdentity,
    #[error("invalid OTLP mTLS client certificate/private key pair")]
    ClientIdentity(#[source] rustls::Error),
    #[error("failed to configure OTLP server trust roots")]
    Trust(#[source] rama::error::BoxError),
}

fn read_tls_file(path: &Path) -> Result<Vec<u8>, OtelHttpClientError> {
    std::fs::read(path).map_err(|source| OtelHttpClientError::ReadFile {
        path: path.to_owned(),
        source,
    })
}

fn read_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, OtelHttpClientError> {
    let pem = read_tls_file(path)?;
    let certificates = CertificateDer::pem_slice_iter(&pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| OtelHttpClientError::Pem {
            path: path.to_owned(),
            source,
        })?;
    if certificates.is_empty() {
        return Err(OtelHttpClientError::EmptyCertificates(path.to_owned()));
    }
    // Validate now, rather than silently accepting invalid files until export.
    let mut roots = rustls::RootCertStore::empty();
    for certificate in &certificates {
        roots
            .add(certificate.clone())
            .map_err(|source| OtelHttpClientError::Certificate {
                path: path.to_owned(),
                source,
            })?;
    }
    Ok(certificates)
}

fn build_tls_config(tls: &OtelTlsConfig) -> Result<TlsClientConfig, OtelHttpClientError> {
    let mut config = TlsClientConfig::default_http();
    if let Some(path) = &tls.ca_certificate {
        // Preserve native system roots and add the configured CA bundle.
        config = config
            .try_with_extra_server_trust_anchors(read_certificates(path.as_path())?)
            .map_err(OtelHttpClientError::Trust)?;
    }
    match (&tls.client_certificate, &tls.client_private_key) {
        (None, None) => {}
        (Some(certificate_path), Some(key_path)) => {
            let cert_chain = read_certificates(certificate_path.as_path())?;
            let pem = read_tls_file(key_path.as_path())?;
            let private_key =
                PrivateKeyDer::from_pem_slice(&pem).map_err(|source| OtelHttpClientError::Pem {
                    path: key_path.to_path_buf(),
                    source,
                })?;
            rustls::sign::CertifiedKey::from_der(
                cert_chain.clone(),
                private_key.clone_key(),
                &rustls::crypto::ring::default_provider(),
            )
            .map_err(OtelHttpClientError::ClientIdentity)?;
            config = config.with_client_auth(ClientAuth::Single(ClientAuthData {
                private_key,
                cert_chain,
            }));
        }
        _ => return Err(OtelHttpClientError::IncompleteClientIdentity),
    }
    Ok(config)
}

/// Build an HTTP client for OTLP HTTP exporters.
///
/// Returns a rama-based `RamaOtelClient` that implements the OpenTelemetry
/// `HttpClient` trait. Exporters outside Tokio use a shared background runtime
/// to drive the async client. Construction validates configured TLS files and
/// fails instead of falling back to default trust or anonymous authentication.
pub(crate) fn build_http_client(
    tls: &OtelTlsConfig,
    _timeout_var: &str,
) -> Result<RamaOtelClient, OtelHttpClientError> {
    Ok(RamaOtelClient::new(build_tls_config(tls)?))
}

pub(crate) fn current_tokio_runtime_is_multi_thread() -> bool {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread,
        Err(_) => false,
    }
}

pub(crate) fn build_async_http_client(
    tls: Option<&OtelTlsConfig>,
    timeout_var: &str,
) -> Result<RamaOtelClient, OtelHttpClientError> {
    build_http_client(tls.unwrap_or(&OtelTlsConfig::default()), timeout_var)
}

#[cfg(test)]
mod tests;
