pub mod execution_built_in_tools;
pub mod execution_custom;
pub mod execution_deno_dynamic;
pub mod execution_python_dynamic;

use serde_json::{Map, Value};
use shinkai_http_api::api_v2::api_v2_handlers_tools::ToolType;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use super::tool_execution::execution_built_in_tools::execute_built_in_tool;
use super::tool_execution::execution_custom::execute_custom_tool;
use super::tool_execution::execution_deno_dynamic::execute_deno_tool;
use super::tool_execution::execution_python_dynamic::execute_python_tool;

use shinkai_db::db::ShinkaiDB;
use std::sync::Arc;

pub async fn execute_tool(
    tool_router_key: String,
    tool_type: ToolType,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
    db: Arc<ShinkaiDB>,
    sqlite_manager: Arc<SqliteManager>,
    bearer: String,
) -> Result<Value, ToolError> {
    // Split the tool name by ":::"
    eprintln!("[execute_tool] {} with tool_router_key: {}", tool_type, tool_router_key);

    // Route based on the prefix
    match tool_type {
        ToolType::JS => {
            execute_built_in_tool(
                tool_type,
                tool_router_key,
                parameters,
                extra_config,
                db,
                sqlite_manager,
                bearer,
            )
            .await
        }
        ToolType::DenoDynamic => execute_deno_tool(tool_router_key.clone(), parameters, extra_config),
        ToolType::PythonDynamic => execute_python_tool(tool_router_key.clone(), parameters, extra_config),
        ToolType::Internal => execute_custom_tool(&tool_router_key, parameters, extra_config),
        _ => Err(ToolError::ExecutionError(format!("Unknown tool type: {}", tool_type))),
    }
}
