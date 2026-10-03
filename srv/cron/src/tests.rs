use super::catalog_tool_infos;

#[test]
fn spool_submit_hidden_without_backends_and_listed_with_them() {
    let names = |infos: Vec<mcp_host::prelude::ToolInfo>| {
        infos.into_iter().map(|t| t.name).collect::<Vec<_>>()
    };
    let without = names(catalog_tool_infos(false));
    assert!(!without.contains(&"spool_submit".to_string()));
    assert!(without.contains(&"cron_create".to_string()));
    let with = names(catalog_tool_infos(true));
    assert!(with.contains(&"spool_submit".to_string()));
}
