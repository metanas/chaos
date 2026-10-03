//! MCP tool: project_task — return the enabled project task fixture.

use mcp_host::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::TestStdioServer;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
struct ProjectTaskParams {}

impl TestStdioServer {
    #[mcp_tool(name = "project_task", read_only = true, open_world = false)]
    async fn project_task(
        &self,
        _ctx: Ctx<'_>,
        _params: Parameters<ProjectTaskParams>,
    ) -> ToolResult {
        Ok(ToolOutput::json(serde_json::Map::from_iter([(
            "task".to_string(),
            serde_json::Value::String("available".to_string()),
        )])))
    }
}
