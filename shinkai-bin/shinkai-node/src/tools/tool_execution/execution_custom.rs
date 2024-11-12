use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_message_primitives::schemas::inbox_name;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::V2ChatMessage;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::api_v2::api_v2_handlers_tools::Language;
use shinkai_http_api::node_api_router::APIError;
use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use tokio::sync::{Mutex, RwLock};

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::tools::tool_generation::v2_create_and_send_job_message;
use crate::{llm_provider::job_manager::JobManager, network::Node};

use tokio::time::{sleep, Duration};

pub async fn execute_custom_tool(
    tool_router_key: &String,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
    bearer: String,
    db: Arc<ShinkaiDB>,
    node_name: ShinkaiName,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    match tool_router_key {
        s if s == "local:::llm" => {
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
            )
            .await
        }
        s if s == &String::from("local:::text_analyzer") => execute_text_analyzer(&parameters),
        s if s == &String::from("local:::calculator") => execute_calculator(&parameters),
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
) -> Result<Value, ToolError> {
    let llm_provider = "llama3_1_8b".to_string();
    let content = parameters
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let response = v2_create_and_send_job_message(
        bearer.clone(),
        JobCreationInfo {
            scope: JobScope::new_default(),
            is_hidden: Some(false),
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

fn execute_calculator(parameters: &Map<String, Value>) -> Result<Value, ToolError> {
    // Extract parameters
    let operation = parameters
        .get("operation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::SerializationError("Missing operation parameter".to_string()))?;

    let x = parameters
        .get("x")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ToolError::SerializationError("Missing or invalid x parameter".to_string()))?;

    let y = parameters
        .get("y")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ToolError::SerializationError("Missing or invalid y parameter".to_string()))?;

    // Perform calculation
    let result = match operation {
        "add" => x + y,
        "subtract" => x - y,
        "multiply" => x * y,
        "divide" => {
            if y == 0.0 {
                return Err(ToolError::ExecutionError("Division by zero".to_string()));
            }
            x / y
        }
        _ => return Err(ToolError::ExecutionError("Invalid operation".to_string())),
    };

    Ok(json!({
        "result": result
    }))
}

fn execute_text_analyzer(parameters: &Map<String, Value>) -> Result<Value, ToolError> {
    // Extract parameters
    let text = parameters
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::SerializationError("Missing text parameter".to_string()))?;

    let include_sentiment = parameters
        .get("include_sentiment")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Calculate basic statistics
    let word_count = text.split_whitespace().count();
    let character_count = text.chars().count();

    // Create response
    let mut response = json!({
        "word_count": word_count,
        "character_count": character_count,
    });

    // Add sentiment analysis if requested
    if include_sentiment {
        let sentiment_score = calculate_mock_sentiment(text);
        response
            .as_object_mut()
            .unwrap()
            .insert("sentiment_score".to_string(), json!(sentiment_score));
    }

    Ok(response)
}

fn calculate_mock_sentiment(text: &str) -> f64 {
    let positive_words = ["good", "great", "excellent", "happy", "wonderful"];
    let negative_words = ["bad", "terrible", "awful", "sad", "horrible"];

    let lowercase_text = text.to_lowercase();
    let words: Vec<&str> = lowercase_text.split_whitespace().collect();
    let mut score: f64 = 0.0;

    for word in words {
        if positive_words.contains(&word) {
            score += 0.2;
        }
        if negative_words.contains(&word) {
            score -= 0.2;
        }
    }

    score.clamp(-1.0, 1.0)
}
