use super::*;

#[test]
fn tool_items_preserve_optional_server_metadata() {
    for server in [None, Some("external".to_string())] {
        let item = McpToolCallItem {
            server: server.clone(),
            tool: "read_mcp_resource".into(),
            arguments: serde_json::json!({"uri": "chaos://sessions"}),
            result: None,
            error: None,
            status: McpToolCallStatus::InProgress,
        };
        let value = serde_json::to_value(&item).unwrap();
        assert_eq!(
            value.get("server").and_then(JsonValue::as_str),
            server.as_deref()
        );
        assert_eq!(
            serde_json::from_value::<McpToolCallItem>(value).unwrap(),
            item
        );
    }
}
