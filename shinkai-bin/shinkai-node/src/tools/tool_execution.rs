pub mod execution_built_in_tools;
pub mod execution_custom;
pub mod execution_deno_dynamic;
pub mod execution_python_dynamic;

use crate::llm_provider::job_manager::JobManager;
use crate::tools::generate_tool_definitions;
use serde_json::{Map, Value};
use shinkai_http_api::api_v2::api_v2_handlers_tools::{Language, ToolType};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use super::tool_execution::execution_built_in_tools::execute_built_in_tool;
use super::tool_execution::execution_custom::execute_custom_tool;
use super::tool_execution::execution_deno_dynamic::execute_deno_tool;
use super::tool_execution::execution_python_dynamic::execute_python_tool;
use tokio::sync::Mutex;

use crate::managers::IdentityManager;
use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use std::sync::Arc;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn execute_tool(
    tool_router_key: String,
    tool_type: ToolType,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
    db: Arc<ShinkaiDB>,
    sqlite_manager: Arc<SqliteManager>,
    bearer: String,
    node_name: ShinkaiName,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    eprintln!("[execute_tool] {} with tool_router_key: {}", tool_type, tool_router_key);

    // Route based on the prefix
    match tool_type {
        ToolType::Deno => {
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
        ToolType::DenoDynamic => {
            let header_code = generate_tool_definitions(Language::Typescript, sqlite_manager, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            execute_deno_tool(bearer.clone(), parameters, extra_config, header_code)
        }
        ToolType::PythonDynamic => execute_python_tool(tool_router_key.clone(), parameters, extra_config),
        ToolType::Internal => {
            execute_custom_tool(
                &tool_router_key,
                parameters,
                extra_config,
                bearer,
                db,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
            )
            .await
        }
        _ => Err(ToolError::ExecutionError(format!("Unknown tool type: {}", tool_type))),
    }
}
