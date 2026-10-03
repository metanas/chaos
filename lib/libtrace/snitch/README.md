# chaos-snitch

Telemetry, observability, and audit trail. Structured events, local-first metrics, token usage tracking, and session diagnostics. Opt-in remote reporting only.

`MetricsClient::start_timer` is infallible. OTLP HTTP-client construction,
exporter setup, network requests, metric/tag validation, snapshots, and shutdown
remain fallible. Global and session timer helpers also remain fallible because
a global exporter may be unavailable and session metadata tags may be invalid.

## OTLP TLS

HTTP exporters for logs, traces, and metrics honor the optional `tls` settings:

- `ca_certificate`: a PEM CA bundle added to native system trust roots.
- `client_certificate`: a PEM client certificate chain, with the leaf first.
- `client_private_key`: a matching PEM private key (PKCS#8, PKCS#1, or SEC1).

The client certificate and key must be configured together. Unreadable or
invalid files, empty certificate bundles, and mismatched identities fail
exporter initialization rather than falling back to anonymous authentication.
Server certificate and hostname verification remain enabled.
