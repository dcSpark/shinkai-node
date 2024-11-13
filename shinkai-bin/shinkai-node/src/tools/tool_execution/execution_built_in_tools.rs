use crate::network::Node;
use async_channel::bounded;
use serde_json::{json, Map, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::api_v2::api_v2_handlers_tools::ToolType;
use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn get_shinkai_tool(
    db: Arc<ShinkaiDB>,
    lance_db: Arc<RwLock<LanceShinkaiDb>>,
    bearer: String,
    tool_router_key: String,
) -> Result<ShinkaiTool, ToolError> {
    let (tool_res_sender, tool_res_receiver) = bounded(1);
    Node::v2_api_get_shinkai_tool(db, lance_db, bearer, tool_router_key, tool_res_sender)
        .await
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    // Receive and unwrap the nested Result
    let tool_result = tool_res_receiver
        .recv()
        .await
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    // Convert the Value to ShinkaiTool
    match tool_result {
        Ok(value) => serde_json::from_value(value)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to deserialize tool: {}", e))),
        Err(api_error) => Err(ToolError::ExecutionError(api_error.message)),
    }
}

pub async fn execute_built_in_tool(
    tool_type: ToolType,
    tool_router_key: String,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
    db: Arc<ShinkaiDB>,
    lance_db: Arc<RwLock<LanceShinkaiDb>>,
    bearer: String,
) -> Result<Value, ToolError> {
    match tool_type {
        ToolType::Deno => {
            let tool: ShinkaiTool = get_shinkai_tool(db, lance_db, bearer, tool_router_key).await?;
            if let ShinkaiTool::Deno(js_tool, enabled) = tool {
                if !enabled {
                    return Err(ToolError::ToolNotRunnable(
                        "This tool is currently disabled".to_string(),
                    ));
                }
                match js_tool.run(parameters, extra_config) {
                    Ok(result) => {
                        println!("[execute_built_in_tool] JS tool execution successful");
                        Ok(json!(result))
                    }
                    Err(e) => {
                        eprintln!("[execute_built_in_tool] JS tool execution failed: {}", e);
                        Err(ToolError::ExecutionError(format!("JS tool execution failed: {}", e)))
                    }
                }
            } else {
                Err(ToolError::ToolNotRunnable(
                    "This tool is currently disabled".to_string(),
                ))
            }
        }
        ToolType::Deno => todo!(),
        ToolType::DenoDynamic => todo!(),
        ToolType::Python => todo!(),
        ToolType::PythonDynamic => todo!(),
        ToolType::Network => todo!(),
        ToolType::Internal => todo!(),
    }
    // ShinkaiTool::Network(_, _) => Err(ToolError::ToolNotRunnable(
    //     "Network tools are currently disabled".to_string(),
    // )),
    // ShinkaiTool::Rust(_, _) => Err(ToolError::ToolNotRunnable(
    //     "Rust tools are currently disabled".to_string(),
    // )),
    // ShinkaiTool::Workflow(_, _) => Err(ToolError::ToolNotRunnable(
    //     "Workflow tools are currently disabled".to_string(),
    // )),
    // ShinkaiTool::Deno(_, _) => Err(ToolError::ToolNotRunnable(
    //     "Deno tools are currently disabled".to_string(),
    // )),
    // ShinkaiTool::Python(_, _) => Err(ToolError::ToolNotRunnable(
    //     "Python tools are currently disabled".to_string(),
    // )),
    // ShinkaiTool::Internal(_, _) => Err(ToolError::ToolNotRunnable(
    //     "Internal tools are currently disabled".to_string(),
    // )),
}
// }
