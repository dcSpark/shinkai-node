use crate::llm_provider::job_manager::JobManager;
use crate::tools::tool_definitions::definition_generation::generate_tool_definitions;
use crate::tools::tool_execution::execution_custom::execute_custom_tool;
use crate::tools::tool_execution::execution_deno_dynamic::execute_deno_tool;
use serde_json::json;
use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::shinkai_tools::DynamicToolType;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use tokio::sync::Mutex;

use crate::managers::IdentityManager;
use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use std::collections::HashMap;
use std::sync::Arc;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn execute_tool(
    bearer: String,
    node_name: ShinkaiName,
    db: Arc<ShinkaiDB>,
    sqlite_manager: Arc<SqliteManager>,
    tool_router_key: String,
    parameters: Map<String, Value>,
    tool_id: Option<String>,
    app_id: Option<String>,
    llm_provider: String,
    extra_config: Option<String>,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    eprintln!("[execute_tool] with tool_router_key: {}", tool_router_key);

    // Get the tool from the database
    let tool = sqlite_manager
        .get_tool_by_key(&tool_router_key)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get tool: {}", e)))?;

    // Match the tool type and execute the appropriate function
    match tool {
        ShinkaiTool::Deno(deno_tool, _) => {
            let mut envs = HashMap::new();
            envs.insert("BEARER".to_string(), bearer);
            envs.insert("x-shinkai-tool-id".to_string(), tool_id.unwrap_or("".to_owned()));
            envs.insert("x-shinkai-app-id".to_string(), app_id.unwrap_or("".to_owned()));
            // TODO: add header_code
            deno_tool
                .run(envs, "".to_string(), parameters, extra_config)
                .map(|result| json!(result))
                .map_err(|e| ToolError::ExecutionError(e.to_string()))
        }
        ShinkaiTool::Rust(_, _) => {
            execute_custom_tool(
                &tool_router_key,
                parameters,
                tool_id,
                app_id,
                extra_config,
                bearer,
                db,
                llm_provider,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
            )
            .await
        }
        _ => Err(ToolError::ExecutionError(format!("Unsupported tool type: {:?}", tool))),
    }
}

pub async fn execute_code(
    tool_type: DynamicToolType,
    code: String,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
    sqlite_manager: Arc<SqliteManager>,
    tool_id: Option<String>,
    app_id: Option<String>,
    bearer: String,
) -> Result<Value, ToolError> {
    eprintln!("[execute_code] tool_type: {}", tool_type);

    // Route based on the prefix
    match tool_type {
        DynamicToolType::DenoDynamic => {
            let header_code = generate_tool_definitions(CodeLanguage::Typescript, sqlite_manager, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            execute_deno_tool(
                bearer.clone(),
                parameters,
                tool_id,
                app_id,
                extra_config,
                header_code,
                code,
            )
        }
        DynamicToolType::PythonDynamic => {
            return Err(ToolError::ExecutionError("NYI Python".to_string()));
        }
    }
}
