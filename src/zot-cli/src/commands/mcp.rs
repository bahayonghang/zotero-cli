use anyhow::Result;

use crate::cli::McpCommand;
use crate::context::AppContext;

pub(crate) async fn handle(_ctx: &AppContext, command: McpCommand) -> Result<()> {
    match command {
        McpCommand::Serve => Err(zot_core::ZotError::Unsupported {
            code: "mcp-not-implemented".to_string(),
            message: "MCP server is not implemented yet in this Rust port".to_string(),
            hint: Some("Use the CLI commands directly until rmcp integration lands".to_string()),
        }
        .into()),
    }
}
