use std::collections::HashMap;
use std::path::PathBuf;

use chaos_realpath::AbsolutePathBuf;

pub(crate) fn resolve_exporter(exporter: &OtelExporter) -> OtelExporter {
    exporter.clone()
}

#[derive(Clone, Debug)]
pub struct OtelSettings {
    pub environment: String,
    pub service_name: String,
    pub service_version: String,
    pub chaos_home: PathBuf,
    pub exporter: OtelExporter,
    pub trace_exporter: OtelExporter,
    pub metrics_exporter: OtelExporter,
    pub runtime_metrics: bool,
}

#[derive(Clone, Debug)]
pub enum OtelHttpProtocol {
    /// HTTP protocol with binary protobuf
    Binary,
    /// HTTP protocol with JSON payload
    Json,
}

#[derive(Clone, Debug, Default)]
pub struct OtelTlsConfig {
    /// PEM CA bundle added to native system trust roots.
    pub ca_certificate: Option<AbsolutePathBuf>,
    /// PEM client certificate chain, leaf first. Requires `client_private_key`.
    pub client_certificate: Option<AbsolutePathBuf>,
    /// PEM private key matching the client leaf certificate.
    /// Invalid or incomplete identities fail exporter initialization.
    pub client_private_key: Option<AbsolutePathBuf>,
}

#[derive(Clone, Debug)]
pub enum OtelExporter {
    None,
    OtlpHttp {
        endpoint: String,
        headers: HashMap<String, String>,
        protocol: OtelHttpProtocol,
        tls: Option<OtelTlsConfig>,
    },
}
