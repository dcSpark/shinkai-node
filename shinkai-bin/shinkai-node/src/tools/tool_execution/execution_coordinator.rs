use crate::llm_provider::job_manager::JobManager;
use crate::tools::tool_definitions::definition_generation::generate_tool_definitions;
use crate::tools::tool_execution::execution_custom::execute_custom_tool;
use crate::tools::tool_execution::execution_deno_dynamic::execute_deno_tool;
use crate::utils::environment::fetch_node_environment;
use serde_json::json;
use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::shinkai_tools::DynamicToolType;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use tokio::sync::{Mutex, RwLock};

use crate::managers::IdentityManager;
use ed25519_dalek::SigningKey;

use std::collections::HashMap;
use std::sync::Arc;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn execute_tool(
    bearer: String,
    node_name: ShinkaiName,
    db: Arc<RwLock<SqliteManager>>,
    vector_fs: Arc<VectorFS>,
    tool_router_key: String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    extra_config: Option<String>,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    eprintln!("[execute_tool] with tool_router_key: {}", tool_router_key);

    // Determine the tool type based on the tool_router_key
    if tool_router_key.contains("rust_toolkit") {
        // Execute as a Rust tool
        execute_custom_tool(
            &tool_router_key,
            parameters,
            tool_id,
            app_id,
            extra_config,
            bearer,
            db,
            vector_fs,
            llm_provider,
            node_name,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
        )
        .await
    } else {
        // Assume it's a Deno tool if not Rust
        let tool = db
            .read()
            .await
            .get_tool_by_key(&tool_router_key)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to get tool: {}", e)))?;

        match tool {
            ShinkaiTool::Deno(deno_tool, _) => {
                let mut envs = HashMap::new();
                envs.insert("BEARER".to_string(), bearer);
                envs.insert("X_SHINKAI_TOOL_ID".to_string(), tool_id.clone());
                envs.insert("X_SHINKAI_APP_ID".to_string(), app_id.clone());
                envs.insert("X_SHINKAI_INSTANCE_ID".to_string(), "".to_string()); // TODO Pass data from the API
                envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), llm_provider.clone());

                let node_env = fetch_node_environment();
                let node_storage_path = node_env
                    .node_storage_path
                    .clone()
                    .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
                let header_code = generate_tool_definitions(
                    deno_tool.tools.clone().unwrap_or_default(),
                    CodeLanguage::Typescript,
                    db,
                    false,
                )
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;

                deno_tool
                    .run(
                        envs,
                        node_env.api_listen_address.ip().to_string(),
                        node_env.api_listen_address.port(),
                        header_code,
                        parameters,
                        extra_config,
                        node_storage_path,
                        // TODO REMOVE UNWRAP ONCE THE FRONTEND SENDS THE APP ID AND TOOL ID
                        app_id.clone(),
                        tool_id.clone(),
                        true,
                    )
                    .map(|result| json!(result.data))
                    .map_err(|e| ToolError::ExecutionError(e.to_string()))
            }
            _ => Err(ToolError::ExecutionError(format!("Unsupported tool type: {:?}", tool))),
        }
    }
}

pub async fn execute_code(
    tool_type: DynamicToolType,
    code: String,
    tools: Vec<String>,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
    sqlite_manager: Arc<RwLock<SqliteManager>>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    bearer: String,
) -> Result<Value, ToolError> {
    eprintln!("[execute_code] tool_type: {}", tool_type);

    // Route based on the prefix
    match tool_type {
        DynamicToolType::DenoDynamic => {
            let header_code = generate_tool_definitions(tools, CodeLanguage::Typescript, sqlite_manager, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            execute_deno_tool(
                bearer.clone(),
                parameters,
                tool_id,
                app_id,
                llm_provider,
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
