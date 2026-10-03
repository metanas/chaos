//! MCP tool: echo — echo a message and optional test environment values.

use chaos_mcp_runtime::CHAOS_MCP_CLIENT_ID_ENV;
use mcp_host::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::TestStdioServer;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
struct EchoParams {
    message: String,
}

impl TestStdioServer {
    #[mcp_tool(name = "echo", read_only = true, open_world = false)]
    async fn echo(&self, _ctx: Ctx<'_>, params: Parameters<EchoParams>) -> ToolResult {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "echo".to_string(),
            serde_json::Value::String(format!("ECHOING: {}", params.0.message)),
        );
        if let Ok(value) = std::env::var("MCP_TEST_VALUE") {
            payload.insert("env".to_string(), serde_json::Value::String(value));
        }
        if std::env::var_os("MCP_TEST_INCLUDE_CLIENT_ID").is_some() {
            let client_id = std::env::var(CHAOS_MCP_CLIENT_ID_ENV).map_err(|err| {
                ToolError::Execution(format!("missing {CHAOS_MCP_CLIENT_ID_ENV}: {err}"))
            })?;
            payload.insert(
                "client_id".to_string(),
                serde_json::Value::String(client_id),
            );
        }
        Ok(ToolOutput::json(payload))
    }
}
