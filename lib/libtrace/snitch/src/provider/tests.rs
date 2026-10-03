use super::*;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn incomplete_mtls_exporter() -> OtelExporter {
    OtelExporter::OtlpHttp {
        endpoint: "https://127.0.0.1:1/v1/traces".into(),
        headers: Default::default(),
        protocol: OtelHttpProtocol::Binary,
        tls: Some(OtelTlsConfig {
            client_certificate: Some(
                chaos_realpath::AbsolutePathBuf::try_from(
                    std::env::temp_dir().join("unused-otel-client.pem"),
                )
                .expect("absolute test path"),
            ),
            ..Default::default()
        }),
    }
}

#[test]
fn all_exporter_signals_propagate_invalid_tls_configuration() {
    for signal in ["logs", "traces", "metrics"] {
        let mut settings = test_otel_settings();
        match signal {
            "logs" => settings.exporter = incomplete_mtls_exporter(),
            "traces" => settings.trace_exporter = incomplete_mtls_exporter(),
            "metrics" => settings.metrics_exporter = incomplete_mtls_exporter(),
            _ => unreachable!(),
        }
        assert!(
            OtelProvider::from(&settings).is_err(),
            "{signal} must not silently ignore an incomplete mTLS identity"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_trace_exporter_propagates_invalid_tls_configuration() {
    let mut settings = test_otel_settings();
    settings.trace_exporter = incomplete_mtls_exporter();
    assert!(OtelProvider::from(&settings).is_err());
}

#[test]
fn resource_attributes_include_host_name_when_present() {
    let attrs = resource_attributes(
        &test_otel_settings(),
        Some("opentelemetry-test"),
        ResourceKind::Logs,
    );

    let host_name = attrs
        .iter()
        .find(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
        .map(|kv| kv.value.as_str().to_string());

    assert_eq!(host_name, Some("opentelemetry-test".to_string()));
}

#[test]
fn resource_attributes_omit_host_name_when_missing_or_empty() {
    let missing = resource_attributes(&test_otel_settings(), None, ResourceKind::Logs);
    let empty = resource_attributes(&test_otel_settings(), Some("   "), ResourceKind::Logs);
    let trace_attrs = resource_attributes(
        &test_otel_settings(),
        Some("opentelemetry-test"),
        ResourceKind::Traces,
    );

    assert!(
        !missing
            .iter()
            .any(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
    );
    assert!(
        !empty
            .iter()
            .any(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
    );
    assert!(
        !trace_attrs
            .iter()
            .any(|kv| kv.key.as_str() == HOST_NAME_ATTRIBUTE)
    );
}

#[test]
fn log_export_target_excludes_trace_safe_events() {
    assert!(is_log_export_target("chaos_snitch.log_only"));
    assert!(is_log_export_target("chaos_snitch.pf"));
    assert!(!is_log_export_target("chaos_snitch.trace_safe"));
    assert!(!is_log_export_target("chaos_snitch.trace_safe.debug"));
}

#[test]
fn trace_export_target_only_includes_trace_safe_prefix() {
    assert!(is_trace_safe_target("chaos_snitch.trace_safe"));
    assert!(is_trace_safe_target("chaos_snitch.trace_safe.summary"));
    assert!(!is_trace_safe_target("chaos_snitch.log_only"));
    assert!(!is_trace_safe_target("chaos_snitch.pf"));
}

fn test_otel_settings() -> OtelSettings {
    OtelSettings {
        environment: "test".to_string(),
        service_name: "chaos-test".to_string(),
        service_version: "0.0.0".to_string(),
        chaos_home: PathBuf::from("."),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::None,
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
    }
}
