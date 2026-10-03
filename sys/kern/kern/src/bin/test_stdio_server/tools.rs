mod echo;
mod image;
mod project_task;
mod select_project;

use mcp_host::registry::router::McpToolRouter;

use crate::TestStdioServer;

pub(super) fn router() -> McpToolRouter<TestStdioServer> {
    McpToolRouter::new()
        .with_tool(
            TestStdioServer::echo_tool_info(),
            TestStdioServer::echo_handler,
            None,
        )
        .with_tool(
            TestStdioServer::image_tool_info(),
            TestStdioServer::image_handler,
            None,
        )
        .with_tool(
            TestStdioServer::select_project_tool_info(),
            TestStdioServer::select_project_handler,
            None,
        )
        .with_tool(
            TestStdioServer::project_task_tool_info(),
            TestStdioServer::project_task_handler,
            None,
        )
}
