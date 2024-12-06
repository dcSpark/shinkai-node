use std::sync::Arc;

use serde_json::{Map, Value};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use tokio::sync::Mutex;

use tokio::sync::RwLock;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_implementation;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;

pub async fn execute_custom_tool(
    tool_router_key: &String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    _extra_config: Vec<ToolConfig>,
    _oauth: Vec<ToolConfig>,
    bearer: String,
    db: Arc<RwLock<SqliteManager>>,
    vector_fs: Arc<VectorFS>,
    llm_provider: String,
    node_name: ShinkaiName,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    println!("[executing_rust_tool] {}", tool_router_key);
    // TODO: if it is, find it and call it

    // Check if the tool_router_key contains "rust_toolkit"
    if !tool_router_key.contains("rust_toolkit") {
        return Err(ToolError::InvalidFunctionArguments(
            "The tool_router_key does not contain 'rust_toolkit'".to_string(),
        ));
    }

    let result = match tool_router_key {
        // TODO Keep in sync with definitions_custom.rs
        s if s == "local:::rust_toolkit:::shinkai_sqlite_query_executor" => {
            tool_implementation::native_tools::sql_processor::SQLProcessorTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                vector_fs,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
            )
            .await
        }
        s if s == "local:::rust_toolkit:::shinkai_llm_prompt_processor" => {
            tool_implementation::native_tools::llm_prompt_processor::LmPromptProcessorTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                vector_fs,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
            )
            .await
        }
        s if s == "local:::rust_toolkit:::shinkai_process_embeddings" => {
            tool_implementation::native_tools::tool_knowledge::KnowledgeTool::execute(
                bearer,
                tool_id,
                app_id,
                db,
                vector_fs,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
            )
            .await
        }
        _ => Err(ToolError::InvalidFunctionArguments(
            "The specified tool_router_key does not match any known custom tools.".to_string(),
        )),
    };
    let text_result = format!("{:?}", result);
    if text_result.len() > 200 {
        println!(
            "[executing_rust_tool] result: {}...{}",
            &text_result[..100],
            &text_result[text_result.len() - 100..]
        );
    } else {
        println!("[executing_rust_tool] result: {}", text_result);
    }
    result
}
