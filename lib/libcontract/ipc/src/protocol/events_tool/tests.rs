use super::*;

#[test]
fn internal_invocation_omits_server_and_prefix() {
    let invocation = McpInvocation {
        server: None,
        tool: "read_mcp_resource".into(),
        arguments: None,
    };
    let value = serde_json::to_value(&invocation).unwrap();
    assert!(value.get("server").is_none());
    assert_eq!(invocation.display_name(), "read_mcp_resource");
    assert_eq!(
        serde_json::from_value::<McpInvocation>(value).unwrap(),
        invocation
    );
    let schema = serde_json::to_value(schemars::schema_for!(McpInvocation)).unwrap();
    assert!(
        !schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("server"))
    );
}

#[test]
fn external_invocation_retains_server_and_prefix() {
    let invocation = McpInvocation {
        server: Some("external".into()),
        tool: "search".into(),
        arguments: None,
    };
    let value = serde_json::to_value(&invocation).unwrap();
    assert_eq!(value["server"], "external");
    assert_eq!(invocation.display_name(), "external.search");
    assert_eq!(
        serde_json::from_value::<McpInvocation>(value).unwrap(),
        invocation
    );
}
