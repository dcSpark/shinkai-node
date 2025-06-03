use serde_json::{Map, Value};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use std::sync::Arc;

pub async fn execute_mcp_server_dynamic(
    db: Arc<SqliteManager>,
    tool_id: String,
    parameters: Map<String, Value>,
    extra_config: Vec<ToolConfig>,
) -> Result<Value, ToolError> {
    // Get the tool from the database
    let tool = db
        .get_tool_by_key(&tool_id)
        .map_err(|_| ToolError::ExecutionError("Failed to get tool from database".to_string()))?;

    match tool {
        ShinkaiTool::MCPServer(tool, _) => {
            let mcp_server_id = tool
                .mcp_server_ref
                .parse::<i64>()
                .map_err(|_| ToolError::ExecutionError("Failed to parse MCP server reference".to_string()))?;
            // Get the MCP server from the database
            let mcp_server = db
                .get_mcp_server(mcp_server_id)
                .map_err(|_| ToolError::ExecutionError("Failed to get MCP server from database".to_string()))?
                .ok_or_else(|| ToolError::ExecutionError("MCP server not found in database".to_string()))?;

            // Run the tool using the MCP server
            let result = tool
                .run(mcp_server, parameters, extra_config)
                .await
                .map_err(|e| ToolError::ExecutionError(format!("Failed to run MCP server tool: {}", e)))?;

            // Extract and return the data
            let data = result.data;

            Ok(data)
        }
        _ => return Err(ToolError::ExecutionError("Tool is not an MCP server".to_string())),
    }
}
