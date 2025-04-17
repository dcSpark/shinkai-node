use std::{collections::HashMap, path::PathBuf};

use super::execution_header_generator::{check_tool, generate_execution_environment};
use crate::llm_provider::job_manager::JobManager;
use crate::tools::agent_execution::v2_create_and_send_job_message_for_agent;
use crate::tools::tool_generation::v2_send_basic_job_message_for_existing_job;
use crate::utils::environment::fetch_node_environment;
use crate::{managers::IdentityManager, network::Node};
use ed25519_dalek::SigningKey;
use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::{inbox_name::InboxName, shinkai_name::ShinkaiName};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{
    error::ToolError, parameters::Parameters, python_tools::PythonTool, tool_config::{OAuth, ToolConfig}, tool_output_arg::ToolOutputArg, tool_types::{OperatingSystem, RunnerType, ToolResult}
};
use std::sync::Arc;
use tokio::{
    sync::Mutex, time::{sleep, Duration}
};
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn execute_agent_tool(
    bearer: String,
    db: Arc<SqliteManager>,
    parameters: Map<String, Value>,
    node_name: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<Value, ToolError> {
    // Extract session_id and prompt from parameters
    let mut session_id: Option<String> = parameters
        .get("session_id")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let prompt = match parameters.get("prompt") {
        Some(prompt_value) => prompt_value.as_str().unwrap_or_default().to_string(),
        None => String::new(),
    };

    // Set up inbox name and channel for retrieving initial message count
    let mut initial_message_count = 0;

    // Create a new job if session_id is None
    if session_id.is_none() {
        // Get agent_id from parameters or return error
        let agent_id = parameters
            .get("agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| ToolError::ExecutionError("Missing agent_id parameter".to_string()))?;

        // Create a new chat
        let job_id = v2_create_and_send_job_message_for_agent(
            db.clone(),
            agent_id.clone(),
            prompt,
            node_name,
            identity_manager_clone,
            job_manager_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
        )
        .await
        .map_err(|e| ToolError::ExecutionError(format!("Function execution error: {}", e.message)))?;

        // Set the session ID to the created job ID
        session_id = Some(job_id);
        // For new chats, we expect to have 2 messages (system message + agent response)
        initial_message_count = 0;
    } else {
        // Get the current message count before sending a new message
        let inbox_name = InboxName::get_job_inbox_name_from_params(session_id.clone().unwrap())
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        let (count_sender, count_receiver) = async_channel::bounded(1);
        let _ = Node::v2_get_last_messages_from_inbox_with_branches(
            db.clone(),
            bearer.clone(),
            inbox_name.to_string(),
            100,
            None,
            count_sender.clone(),
        )
        .await;

        let existing_messages = count_receiver
            .recv()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?
            .map_err(|_| ToolError::ExecutionError("Failed to get existing messages".to_string()))?;

        // Count the total messages across all branches
        initial_message_count = existing_messages.iter().map(|branch| branch.len()).sum();

        // We should send the message to the existing job
        v2_send_basic_job_message_for_existing_job(
            bearer.clone(),
            session_id.clone().unwrap(),
            prompt,
            None,
            None,
            None,
            db.clone(),
            node_name,
            identity_manager_clone,
            job_manager_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
        )
        .await
        .map_err(|e| ToolError::ExecutionError(format!("Failed to send message to existing job: {}", e.message)))?;
    }

    // Unwrap session_id (we know it's Some at this point)
    let session_id = session_id.unwrap();

    // Set up channel for receiving messages
    let (res_sender, res_receiver) = async_channel::bounded(1);
    let inbox_name = InboxName::get_job_inbox_name_from_params(session_id.clone())
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    // Configure timeout and polling parameters
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(60 * 5); // 5 minutes timeout
    let delay = Duration::from_secs(1); // 1 second delay between polls

    // For new chats, we wait for at least 2 messages; for existing chats, we wait for initial_count + 1
    let expected_min_messages = if initial_message_count == 0 {
        2
    } else {
        initial_message_count + 2
    };

    // Poll for messages until we get a response or timeout
    let messages = loop {
        let _ = Node::v2_get_last_messages_from_inbox_with_branches(
            db.clone(),
            bearer.clone(),
            inbox_name.to_string(),
            100,
            None,
            res_sender.clone(),
        )
        .await;

        let messages = res_receiver
            .recv()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?
            .map_err(|_| ToolError::ExecutionError("Failed to get messages".to_string()))?;

        // Count total messages across all branches
        let current_message_count: usize = messages.iter().map(|branch| branch.len()).sum();

        if current_message_count >= expected_min_messages {
            break messages;
        }

        if start_time.elapsed() >= timeout {
            return Err(ToolError::ExecutionError(
                "Timeout waiting for agent response".to_string(),
            ));
        }

        sleep(delay).await;
    };

    // Extract and return the agent response
    if let Some(last_message) = messages.last().and_then(|branch| branch.last()) {
        let agent_response = last_message.job_message.content.clone();

        // Create a response that includes both the message content and session_id
        let mut response_obj = serde_json::Map::new();
        response_obj.insert("message".to_string(), Value::String(agent_response));
        response_obj.insert("session_id".to_string(), Value::String(session_id));
        response_obj.insert("status".to_string(), Value::String("completed".to_string()));

        Ok(Value::Object(response_obj))
    } else {
        Err(ToolError::ExecutionError("No agent response received".to_string()))
    }
}
