use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

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
    _extra_config: Option<String>,
    bearer: String,
    db: Arc<ShinkaiDB>,
    vector_fs: Arc<VectorFS>,
    sqlite_manager: Arc<RwLock<SqliteManager>>,
    llm_provider: String,
    node_name: ShinkaiName,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    println!("[executing_rust_tool] {}", tool_router_key);

    if tool_router_key.contains("rust_toolkit") {
        let tools = NativeToolsList::static_tools().await;
        if let Some(tool) = tools.iter().find(|t| t.name() == tool_router_key) {
            return tool.execute(
                bearer,
                tool_id,
                app_id,
                db,
                vector_fs,
                sqlite_manager,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
                &parameters,
                llm_provider,
            ).await;
        }
    }

    // Fallback for non-custom tools
    Ok(json!({}))
}
