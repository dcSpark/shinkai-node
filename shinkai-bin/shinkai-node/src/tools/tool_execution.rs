pub mod execution_custom;
pub mod execution_built_in_tools;
pub mod execution_deno;
pub mod execution_python;
pub mod execution_rust;
pub mod execution_network;

use async_std::println;
use serde_json::{Map, Value};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use shinkai_tools_primitives::tools::error::ToolError;

use super::tool_execution::execution_custom::execute_custom_tool;
use super::tool_execution::execution_built_in_tools::execute_built_in_tool;
use super::tool_execution::execution_deno::execute_deno_tool;
use super::tool_execution::execution_python::execute_python_tool;
use super::tool_execution::execution_rust::execute_rust_tool;
use super::tool_execution::execution_network::execute_network_tool;

use shinkai_db::db::ShinkaiDB;
use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn execute_tool(
    tool_router_key: String,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
    db: Arc<ShinkaiDB>,
    lance_db: Arc<RwLock<LanceShinkaiDb>>,
    bearer: String,
) -> Result<Value, ToolError> {
    // Split the tool name by ":::"
    eprintln!("[execute_tool] {}", tool_router_key);

    let parts: Vec<&str> = tool_router_key.split(":::").collect();
    
    if parts.len() < 1 {
        return Err(ToolError::ExecutionError("Invalid tool name format".to_string()));
    }

    // Route based on the prefix
    match parts[0] {
        "local" => {
            execute_built_in_tool(
                tool_router_key,
                parameters,
                extra_config,
                db,
                lance_db,
                bearer
            ).await
        }
        "deno" => execute_deno_tool(tool_router_key.clone(), parameters, extra_config),
        "python" => execute_python_tool(tool_router_key.clone(), parameters, extra_config),
        "rust" => execute_rust_tool(tool_router_key.clone(), parameters, extra_config),
        "network" => execute_network_tool(tool_router_key.clone(), parameters, extra_config),
        "internal" => execute_custom_tool(&tool_router_key, parameters, extra_config)
            .ok_or_else(|| ToolError::ExecutionError("Custom tool execution failed".to_string()))?,
        _ => Err(ToolError::ExecutionError(format!("Unknown tool prefix: {}", parts[0])))
    }
}

