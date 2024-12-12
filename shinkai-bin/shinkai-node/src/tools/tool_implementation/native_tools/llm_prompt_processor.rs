use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::{tool_output_arg::ToolOutputArg, error::ToolError, shinkai_tool::ShinkaiToolHeader};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;

use ed25519_dalek::SigningKey;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use tokio::sync::{Mutex, RwLock};

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::tools::tool_generation::v2_create_and_send_job_message;
use crate::{llm_provider::job_manager::JobManager, network::Node};

use async_trait::async_trait;
use tokio::time::{sleep, Duration};

use crate::tools::tool_implementation::tool_traits::ToolExecutor;

pub struct LmPromptProcessorTool {
    pub tool: ShinkaiToolHeader,
}

impl LmPromptProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai LLM Prompt Processor".to_string(),
                toolkit_name: "shinkai_custom".to_string(),
                description: r#"Tool for processing any prompt using an AI LLM. 
Analyzing the input prompt and returning a string with the result of the prompt.
This can be used to process complex requests, text analysis, text matching, text generation, and any other AI LLM task."#
                    .to_string(),
                tool_router_key: "local:::rust_toolkit:::shinkai_llm_prompt_processor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Tool for processing prompts with LLM".to_string(),
                author: "Shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("format".to_string(), "string".to_string(), "The format of the prompt".to_string(), true);
                    params.add_property("prompt".to_string(), "string".to_string(), "The prompt to process".to_string(), true);
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
impl ToolExecutor for LmPromptProcessorTool {
    async fn execute(
        bearer: String,
        _tool_id: String,
        _app_id: String,
        db_clone: Arc<RwLock<SqliteManager>>,
        _vector_fs_clone: Arc<VectorFS>,
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

        Ok(json!({
            "message": x.last().unwrap().last().unwrap().job_message.content.clone()
        }))
    }
}
