use serde_json::json;
use tokio::sync::Mutex;

use super::*;
use crate::handler::NoopClientHandler;
use crate::protocol::ClientCapabilities;
use crate::protocol::Implementation;
use crate::protocol::JsonRpcMessage;
use crate::protocol::JsonRpcResponse;
use crate::protocol::UiResourceMeta;
use crate::protocol::apps;
use crate::runtime::ConnectionOptions;
use crate::runtime::connect_with_transport;
use crate::transport::TransportFuture;

const URI: &str = "ui://example/view";

struct AppsTransport {
    incoming_tx: mpsc::UnboundedSender<JsonRpcMessage>,
    incoming_rx: Mutex<mpsc::UnboundedReceiver<JsonRpcMessage>>,
    sent: Mutex<Vec<JsonRpcMessage>>,
    read_result: Value,
    listed_resources: Vec<Value>,
}

impl AppsTransport {
    fn new(read_result: Value, listed_resources: Vec<Value>) -> Arc<Self> {
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            incoming_tx,
            incoming_rx: Mutex::new(incoming_rx),
            sent: Mutex::new(Vec::new()),
            read_result,
            listed_resources,
        })
    }

    async fn connect(self: &Arc<Self>, apps_enabled: bool) -> Result<McpSession, GuestError> {
        let capabilities = if apps_enabled {
            ClientCapabilities::default().with_mcp_apps()
        } else {
            ClientCapabilities::default()
        };
        connect_with_transport(
            Arc::clone(self) as Arc<dyn MessageTransport>,
            ConnectionOptions {
                client_info: Implementation::new("test-client", "1.0.0"),
                capabilities,
                handler: Arc::new(NoopClientHandler),
                default_timeout: Duration::from_secs(5),
            },
        )
        .await
    }

    async fn request_count(&self, method: &str) -> usize {
        self.sent
            .lock()
            .await
            .iter()
            .filter(|message| {
                matches!(message, JsonRpcMessage::Request(request) if request.method == method)
            })
            .count()
    }
}

impl MessageTransport for AppsTransport {
    fn send<'a>(&'a self, message: JsonRpcMessage) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            if let JsonRpcMessage::Request(request) = &message {
                let id = request
                    .id
                    .clone()
                    .ok_or_else(|| GuestError::Protocol("mock request has no id".to_string()))?;
                let result = match request.method.as_str() {
                    "initialize" => json!({
                        "protocolVersion": "2025-11-25",
                        "capabilities": {
                            "resources": {},
                            "tools": {},
                            "extensions": {apps::EXTENSION_ID: apps::extension_settings()}
                        },
                        "serverInfo": {"name": "apps-server", "version": "0.6.0"}
                    }),
                    "tools/list" => json!({"tools": [{
                        "name": "view", "inputSchema": {"type": "object"},
                        "_meta": {"ui": {"resourceUri": URI}, "vendor/tool": true}
                    }]}),
                    "resources/list" => json!({"resources": self.listed_resources}),
                    "resources/read" => {
                        assert_eq!(
                            request.params.as_ref().and_then(|p| p.get("uri")),
                            Some(&json!(URI))
                        );
                        self.read_result.clone()
                    }
                    method => return Err(GuestError::MethodNotSupported(method.to_string())),
                };
                self.incoming_tx
                    .send(JsonRpcMessage::Response(JsonRpcResponse::success(
                        id, result,
                    )))
                    .map_err(|_| GuestError::Disconnected)?;
            }
            self.sent.lock().await.push(message);
            Ok(())
        })
    }

    fn recv<'a>(&'a self) -> TransportFuture<'a, JsonRpcMessage> {
        Box::pin(async move {
            self.incoming_rx
                .lock()
                .await
                .recv()
                .await
                .ok_or(GuestError::Disconnected)
        })
    }

    fn shutdown<'a>(&'a self) -> TransportFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn host_contents() -> Value {
    // Generated with the upgraded host's API, rather than a parallel wire model.
    mcp_host::content::resource::ResourceContent::mcp_app(URI, "<p>Hello</p>")
        .with_meta(serde_json::Map::from_iter([
            ("ui".to_string(), json!({"prefersBorder": false})),
            ("vendor/content".to_string(), json!({"opaque": 42})),
        ]))
        .to_value()
}

#[tokio::test]
async fn handshake_and_tool_link_consume_unlisted_host_views() -> Result<(), GuestError> {
    let transport = AppsTransport::new(json!({"contents": [host_contents()]}), vec![]);
    let session = transport.connect(true).await?;
    assert!(session.server_info().capabilities.supports_mcp_apps());

    let tools = session.list_tools().await?;
    let tool = tools
        .first()
        .ok_or_else(|| GuestError::Protocol("missing tool".to_string()))?;
    let view = session
        .read_tool_ui(tool)
        .await?
        .ok_or_else(|| GuestError::Protocol("missing view".to_string()))?;
    assert_eq!(view.html()?, "<p>Hello</p>");
    assert_eq!(view.ui.prefers_border, Some(false));
    assert_eq!(
        serde_json::to_value(&view.contents)?["_meta"]["vendor/content"]["opaque"],
        42
    );
    assert_eq!(transport.request_count("resources/list").await, 0);

    let sent = transport.sent.lock().await;
    let Some(JsonRpcMessage::Request(initialize)) = sent.first() else {
        return Err(GuestError::Protocol("missing initialize".to_string()));
    };
    let capabilities: mcp_host::protocol::capabilities::ClientCapabilities =
        serde_json::from_value(initialize.params.as_ref()
            .ok_or_else(|| GuestError::Protocol("missing initialize params".to_string()))?
            ["capabilities"].clone())?;
    assert!(capabilities.supports_mcp_apps());
    drop(sent);
    session.disconnect().await?;
    Ok(())
}

#[tokio::test]
async fn listed_views_are_filtered_and_cached_policy_is_a_fallback() -> Result<(), GuestError> {
    let transport = AppsTransport::new(
        json!({"contents": [{"uri": URI, "mimeType": apps::MIME_TYPE, "text": "<p>Hello</p>"}]}),
        vec![
            json!({"uri": URI, "name": "view", "mimeType": apps::MIME_TYPE,
                "_meta": {"ui": {"prefersBorder": true}}}),
            json!({"uri": "file:///ordinary", "name": "ordinary", "mimeType": "text/plain"}),
            json!({"uri": "ui://plain/html", "name": "plain-html", "mimeType": "text/html"}),
        ],
    );
    let session = transport.connect(false).await?;
    let views = session.list_ui_resources().await?;
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].uri, URI);
    let view = session.read_ui_resource(URI).await?;
    assert_eq!(view.ui.prefers_border, Some(true));
    assert_eq!(session.list_resources().await?.len(), 3);
    assert_eq!(transport.request_count("resources/list").await, 1);

    let sent = transport.sent.lock().await;
    let Some(JsonRpcMessage::Request(initialize)) = sent.first() else {
        return Err(GuestError::Protocol("missing initialize".to_string()));
    };
    assert!(
        initialize
            .params
            .as_ref()
            .and_then(|params| params["capabilities"].get("extensions"))
            .is_none()
    );
    drop(sent);
    session.disconnect().await?;
    Ok(())
}

#[tokio::test]
async fn content_policy_overrides_cached_listing_policy() -> Result<(), GuestError> {
    let transport = AppsTransport::new(
        json!({"contents": [host_contents()]}),
        vec![
            json!({"uri": URI, "name": "view", "mimeType": apps::MIME_TYPE,
            "_meta": {"ui": {"prefersBorder": true, "permissions": {"camera": {}}}}}),
        ],
    );
    let session = transport.connect(false).await?;
    session.list_ui_resources().await?;
    let view = session.read_ui_resource(URI).await?;
    assert_eq!(view.ui.prefers_border, Some(false));
    assert!(view.ui.permissions.is_none());
    session.disconnect().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_ui_uri_is_rejected_before_sending_a_request() -> Result<(), GuestError> {
    let transport = AppsTransport::new(json!({"contents": []}), vec![]);
    let session = transport.connect(false).await?;
    for uri in [
        "https://example.test/view",
        "file:///tmp/view",
        "ui://",
        "ui://bad view",
    ] {
        assert!(matches!(
            session.read_ui_resource(uri).await,
            Err(GuestError::InvalidParams(_))
        ));
    }
    assert_eq!(transport.request_count("resources/read").await, 0);
    session.disconnect().await?;
    Ok(())
}

#[tokio::test]
async fn missing_mismatched_ambiguous_and_non_apps_contents_are_rejected() -> Result<(), GuestError>
{
    for contents in [
        json!([]),
        json!([{"uri": "ui://other/view", "mimeType": apps::MIME_TYPE, "text": "wrong"}]),
        json!([host_contents(), host_contents()]),
        json!([{"uri": URI, "mimeType": "text/html", "text": "not an app"}]),
        json!([{"uri": URI, "mimeType": apps::MIME_TYPE, "text": "bad CSP",
            "_meta": {"ui": {"csp": {"connectDomains": ["invalid"]}}}}]),
    ] {
        let transport = AppsTransport::new(json!({"contents": contents}), vec![]);
        let session = transport.connect(false).await?;
        assert!(matches!(
            session.read_ui_resource(URI).await,
            Err(GuestError::Protocol(_))
        ));
        session.disconnect().await?;
    }
    Ok(())
}

#[tokio::test]
async fn ordinary_and_visibility_only_tools_do_not_trigger_resource_reads() -> Result<(), GuestError>
{
    let transport = AppsTransport::new(json!({"contents": []}), vec![]);
    let session = transport.connect(false).await?;
    for meta in [json!({}), json!({"ui": {"visibility": ["app"]}})] {
        let tool: ToolInfo = serde_json::from_value(json!({
            "name": "refresh", "inputSchema": {"type": "object"}, "_meta": meta
        }))?;
        assert!(session.read_tool_ui(&tool).await?.is_none());
    }
    assert_eq!(transport.request_count("resources/read").await, 0);
    session.disconnect().await?;
    Ok(())
}

#[tokio::test]
async fn unlisted_view_without_ui_metadata_has_deny_by_default_policy() -> Result<(), GuestError> {
    let transport = AppsTransport::new(
        json!({"contents": [{
            "uri": URI, "mimeType": apps::MIME_TYPE, "text": "<p>Hello</p>"
        }]}),
        vec![],
    );
    let session = transport.connect(false).await?;
    let view = session.read_ui_resource(URI).await?;
    assert_eq!(view.ui, UiResourceMeta::default());
    session.disconnect().await?;
    Ok(())
}
