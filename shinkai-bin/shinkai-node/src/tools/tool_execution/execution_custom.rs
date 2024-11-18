use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use tokio::sync::Mutex;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::tools::tool_generation::v2_create_and_send_job_message;
use crate::{llm_provider::job_manager::JobManager, network::Node};

use tokio::time::{sleep, Duration};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

pub async fn execute_custom_tool(
    tool_router_key: &String,
    parameters: Map<String, Value>,
    bearer: String,
    db: Arc<ShinkaiDB>,
    llm_provider: String,
    node_name: ShinkaiName,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    match tool_router_key {
        // TODO this can be fetched from the tool definition
        s if s == "local:::rust_toolkit:::shinkai_sqlite_query_executor" => execute_sqlite_query(&parameters),
        s if s == "local:::rust_toolkit:::shinkai_llm_prompt_processor" => {
            execute_llm(
                bearer,
                db,
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
        s if s == "local:::shinkai_custom:::sqlite_executor" => execute_sqlite_query(&parameters),
        _ => Ok(json!({})), // Not a custom tool
    }
}

async fn execute_llm(
    bearer: String,
    db_clone: Arc<ShinkaiDB>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
    parameters: &Map<String, Value>,
    llm_provider: String,
) -> Result<Value, ToolError> {
    let content = parameters
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let response = v2_create_and_send_job_message(
        bearer.clone(),
        JobCreationInfo {
            scope: JobScope::new_default(),
            is_hidden: Some(true),
            associated_ui: None,
        },
        llm_provider,
        content,
        db_clone.clone(),
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
    )
    .await
    .map_err(|_| ToolError::ExecutionError("Failed to create job".to_string()))?;

    let (res_sender, res_receiver) = async_channel::bounded(1);
    let inbox_name = InboxName::get_job_inbox_name_from_params(response.clone())
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(180); // 3 minutes timeout
    let delay = Duration::from_secs(1); // 1 second delay between polls

    let x = loop {
        let _ = Node::v2_get_last_messages_from_inbox_with_branches(
            db_clone.clone(),
            bearer.clone(),
            inbox_name.to_string(),
            100,
            None,
            res_sender.clone(),
        )
        .await;

        let x = res_receiver
            .recv()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?
            .map_err(|_| ToolError::ExecutionError("Failed to get messages".to_string()))?;

        if x.len() >= 2 {
            break x;
        }

        if start_time.elapsed() >= timeout {
            return Err(ToolError::ExecutionError("Timeout waiting for messages".to_string()));
        }

        sleep(delay).await;
    };
    println!("messages-llm-bot: {} {:?}", x.len(), x);

    Ok(json!({ "message": x.last().unwrap().last().unwrap().job_message.content.clone() }))
}

fn execute_sqlite_query(parameters: &Map<String, Value>) -> Result<Value, ToolError> {
    let query = parameters
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::ExecutionError("Query parameter is required".to_string()))?;

    let relative_path = parameters
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::ExecutionError("Path parameter is required".to_string()))?;

    // Create full path in temporary directory
    let temp_dir = std::env::temp_dir();
    let full_path = temp_dir.join(relative_path);

    // Ensure parent directory exists
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create directory structure: {}", e)))?;
    }

    let manager = SqliteConnectionManager::file(full_path);
    let pool = Pool::new(manager)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to create connection pool: {}", e)))?;

    let conn = pool
        .get()
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get connection: {}", e)))?;

    // Execute the query
    match conn.execute(query, params![]) {
        Ok(rows_affected) => Ok(json!({
            "result": format!("Query executed successfully. Rows affected: {}", rows_affected)
        })),
        Err(e) => Err(ToolError::ExecutionError(format!("Query execution failed: {}", e))),
    }
}
