use std::sync::Arc;

use serde_json::{Map, Value};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use tokio::sync::Mutex;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_implementation;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;

pub async fn try_to_execute_rust_tool(
    tool_router_key: &String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    _extra_config: Vec<ToolConfig>,
    bearer: String,
    db: Arc<SqliteManager>,
    llm_provider: String,
    node_name: ShinkaiName,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
    configs: &Vec<ToolConfig>,
) -> Result<Value, ToolError> {
    println!("[executing_rust_tool] {}", tool_router_key);

    let result = match tool_router_key {
        // TODO Keep in sync with definitions_custom.rs
        s if s == "local:::__official_shinkai:::shinkai_llm_map_reduce_processor" => {
            tool_implementation::native_tools::llm_map_reduce_processor::LlmMapReduceProcessorTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
                configs,
            )
            .await
        }
        s if s == "local:::__official_shinkai:::shinkai_typescript_unsafe_processor" => {
            tool_implementation::native_tools::typescript_unsafe_processor::TypescriptUnsafeProcessorTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
                configs,
            )
            .await
        }
        s if s == "local:::__official_shinkai:::shinkai_sqlite_query_executor" => {
            tool_implementation::native_tools::sql_processor::SQLProcessorTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
                configs,
            )
            .await
        }
        s if s == "local:::__official_shinkai:::shinkai_tool_config_updater" => {
            tool_implementation::native_tools::config_setup::ConfigSetupTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
                configs,
            )
            .await
        }
        s if s == "local:::__official_shinkai:::shinkai_llm_prompt_processor" => {
            tool_implementation::native_tools::llm_prompt_processor::LlmPromptProcessorTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
                configs,
            )
            .await
        }
        s if s == "local:::__official_shinkai:::shinkai_process_embeddings" => {
            tool_implementation::native_tools::tool_knowledge::KnowledgeTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
                configs,
            )
            .await
        }
        _ => return Err(ToolError::ToolNotFound(tool_router_key.to_string())),
    };
    let text_result = format!("{:?}", result);
    if text_result.chars().count() > 200 {
        let start = text_result.chars().take(100).collect::<String>();
        let end = text_result
            .chars()
            .rev()
            .take(100)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        println!("[executing_rust_tool] result: {}...{}", start, end);
    } else {
        println!("[executing_rust_tool] result: {}", text_result);
    }
    result
}
