//! MCP tool: select_project — enable the project task fixture.

use mcp_host::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::TestStdioServer;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
struct SelectProjectParams {}

impl TestStdioServer {
    #[mcp_tool(name = "select_project", read_only = false, open_world = false)]
    async fn select_project(
        &self,
        _ctx: Ctx<'_>,
        _params: Parameters<SelectProjectParams>,
    ) -> ToolResult {
        self.tool_registry.enable_tool("project_task");
        Ok(ToolOutput::json(serde_json::Map::from_iter([(
            "selected".to_string(),
            serde_json::Value::Bool(true),
        )])))
    }
}
