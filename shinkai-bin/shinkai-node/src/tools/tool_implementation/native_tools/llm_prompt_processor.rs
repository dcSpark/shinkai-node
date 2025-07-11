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
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
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


pub struct LlmPromptProcessorTool {
    pub tool: ShinkaiToolHeader,
}

impl LlmPromptProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai LLM Prompt Processor".to_string(),
                description: r#"Tool for processing any prompt using an AI LLM. 
Analyzing the input prompt and returning a string with the result of the prompt.
This can be used to process complex requests, text analysis, text matching, text generation, and any other AI LLM task."#
                    .to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_llm_prompt_processor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Tool for processing prompts with LLM".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                mcp_enabled: Some(false),
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("format".to_string(), "string".to_string(), "Response type. The only valid option is 'text'".to_string(), true, None);
                    params.add_property("prompt".to_string(), "string".to_string(), "The prompt to process".to_string(), true, None);
                    // Add the optional llm_provider parameter
                    params.add_property("llm_provider".to_string(), "string".to_string(), "The LLM provider to use, if not provided, the default provider will be used".to_string(), false, None);
                    
                    // Add the optional tools array parameter
                    let tools_property = Property::with_array_items(
                        "List of tools names or tool router keys to be used with the prompt".to_string(),
                        Property::new("string".to_string(), "Tool".to_string(), None)
                    );
                    params.properties.insert("tools".to_string(), tools_property);
                    
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
impl ToolExecutor for LlmPromptProcessorTool {
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
        default_llm_provider: String,
    ) -> Result<Value, ToolError> {
        let content = parameters
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tools = if let Some(tools_array) = parameters.get("tools").and_then(|v| v.as_array()) {
            Some(tools_array.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect::<Vec<String>>())
        } else {
            None
        };

        let image_paths = if let Some(paths_array) = parameters.get("image_paths").and_then(|v| v.as_array()) {
            Some(paths_array.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect::<Vec<String>>())
        } else {
            None
        };

        let llm_provider_param_opt = parameters.get("llm_provider").and_then(|v| v.as_str());
        
        let mut llm_provider_to_use = default_llm_provider;

        if let Some(llm_provider_param) = llm_provider_param_opt {
            if !llm_provider_param.is_empty() {
                let available_providers = db_clone.get_all_llm_providers()
                    .map_err(|e| ToolError::ExecutionError(format!("Failed to get LLM providers: {}", e)))?;
                
                let provider_exists = available_providers.iter().any(|p: &SerializedLLMProvider| p.id == llm_provider_param);

                if provider_exists {
                    llm_provider_to_use = llm_provider_param.to_string();
                } else {
                    let available_ids: Vec<String> = available_providers.iter().map(|p| p.id.clone()).collect();
                    let error_message = format!(
                        "LLM provider '{}' not found. Available providers: {}",
                        llm_provider_param,
                        available_ids.join(", ")
                    );
                    return Err(ToolError::ExecutionError(error_message));
                }
            }
        }

        let response = v2_create_and_send_job_message(
            bearer.clone(),
            JobCreationInfo {
                scope: MinimalJobScope::default(),
                is_hidden: Some(true),
                associated_ui: None,
            },
            llm_provider_to_use,
            content,
            tools,
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
        println!("messages-llm-bot: {} {:?}", x.len(), x);

        Ok(json!({
            "message": x.last().unwrap().last().unwrap().job_message.content.clone()
        }))
    }
}
