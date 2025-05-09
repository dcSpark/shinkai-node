use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::{Parameters, Property};
use shinkai_tools_primitives::tools::{
    error::ToolError, shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg,
};
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;

use ed25519_dalek::SigningKey;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use tokio::sync::Mutex;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::tools::tool_generation::v2_create_and_send_job_message;
use crate::{llm_provider::job_manager::JobManager, network::Node};

use async_trait::async_trait;
use tokio::time::{sleep, Duration};

use crate::tools::tool_implementation::tool_traits::ToolExecutor;

pub struct AgentPromptProcessorTool {
    pub tool: ShinkaiToolHeader,
}

impl AgentPromptProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai Agent Prompt Processor".to_string(),
                description: r#"Tool for processing any prompt using a Agent. 
Analyzing the input prompt and returning a string with the result of the prompt.
This can be used to process complex requests, text analysis, text matching, text generation, and any other AI LLM task."#
                    .to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_agent_prompt_processor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Tool for processing prompts with Agent".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                mcp_enabled: Some(false),
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("prompt".to_string(), "string".to_string(), "The prompt the agent will process".to_string(), true, None);
                    // Add the optional llm_provider parameter
                    params.add_property("agent".to_string(), "string".to_string(), "The Agent to use.".to_string(), true, None);
                    let image_paths_property = Property::with_array_items(
                        "List of image file paths to be used with the prompt".to_string(),
                        Property::new("string".to_string(), "Image path".to_string(), None)
                    );
                    params.properties.insert("image_paths".to_string(), image_paths_property);
                    params
                },
                output_arg: ToolOutputArg {
                    json: r#"{"type": "object", "properties": {"message": {"type": "string"}}}"#.to_string(),
                },
                config: None,
                usage_type: None,
                tool_offering: None,
            },
        }
    }
}

#[async_trait]
impl ToolExecutor for AgentPromptProcessorTool {
    async fn execute(
        bearer: String,
        _tool_id: String,
        _app_id: String,
        db_clone: Arc<SqliteManager>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        _default_llm_provider: String,
    ) -> Result<Value, ToolError> {
        let content = parameters
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let image_paths = if let Some(paths_array) = parameters.get("image_paths").and_then(|v| v.as_array()) {
            Some(
                paths_array
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<String>>(),
            )
        } else {
            None
        };

        let agent_param = parameters
            .get("agent")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let available_agents = db_clone
            .get_all_agents()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to get agents: {}", e)))?;

        let agent_to_use = available_agents.iter().find(|p| p.agent_id == agent_param);

        if !agent_to_use.is_some() {
            let available_ids: Vec<String> = available_agents.iter().map(|p| p.agent_id.clone()).collect();
            let error_message = format!(
                "Agent '{}' not found. Available agents: {}",
                agent_param,
                available_ids.join(", ")
            );
            return Err(ToolError::ExecutionError(error_message));
        }

        let agent_to_use = agent_to_use.unwrap();
        let response = v2_create_and_send_job_message(
            bearer.clone(),
            JobCreationInfo {
                scope: MinimalJobScope::default(),
                is_hidden: Some(true),
                associated_ui: None,
            },
            agent_to_use.agent_id.clone(),
            content,
            Some(
                agent_to_use
                    .tools
                    .iter()
                    .map(|t| t.to_string_without_version())
                    .collect(),
            ),
            image_paths,
            None,
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
        let timeout = Duration::from_secs(60 * 5); // 5 minutes timeout
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
        println!("messages-agent-processor: {} {:?}", x.len(), x);

        Ok(json!({
            "message": x.last().unwrap().last().unwrap().job_message.content.clone()
        }))
    }
}
