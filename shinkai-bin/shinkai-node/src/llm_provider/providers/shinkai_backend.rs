use std::sync::Arc;

use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::galxe_quests::generate_proof;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};
use rusqlite::params;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_sqlite::SqliteManager;

use super::super::error::LLMProviderError;
use super::openai::{
    add_options_to_payload, handle_non_streaming_response, handle_streaming_response, truncate_image_url_in_payload
};
use super::shared::openai_api::openai_prepare_messages;
use super::LLMService;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, ShinkaiBackend
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
impl LLMService for ShinkaiBackend {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
        _db: Arc<SqliteManager>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            let url = format!("{}/ai/chat/completions", base_url);
            if let Some(key) = api_key {
                let result = openai_prepare_messages(&model, prompt)?;
                let messages_json = match self.model_type().to_uppercase().as_str() {
                    "PREMIUM_TEXT_INFERENCE" | "STANDARD_TEXT_INFERENCE" | "FREE_TEXT_INFERENCE" => {
                        match result.messages {
                            PromptResultEnum::Value(v) => v,
                            _ => {
                                return Err(LLMProviderError::UnexpectedPromptResultVariant(
                                    "Expected Value variant in PromptResultEnum".to_string(),
                                ))
                            }
                        }
                    }
                    _ => {
                        return Err(LLMProviderError::InvalidModelType(format!(
                            "Unsupported model type: {:?}",
                            self.model_type()
                        )))
                    }
                };

                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                // Extract tools_json from the result
                let tools_json = result.functions.unwrap_or_else(Vec::new);

                // Print messages_json as a pretty JSON string
                match serde_json::to_string_pretty(&messages_json) {
                    Ok(pretty_json) => eprintln!("Messages JSON: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize messages_json: {:?}", e),
                };

                match serde_json::to_string_pretty(&tools_json) {
                    Ok(pretty_json) => eprintln!("Tools JSON: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize tools_json: {:?}", e),
                };

                // Get the node's signature public key from the database
                let (node_name, node_signature_public_key) = _db
                    .query_row(
                        "SELECT node_name, node_signature_public_key FROM local_node_keys LIMIT 1",
                        params![],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)), 
                    )
                    .map_err(|e| format!("Failed to get node signature public key: {}", e))?;

                // Generate proof using the node's signature public key
                let (signature, metadata) = generate_proof(hex::encode(node_signature_public_key), messages_json.to_string())?;

                // Set up initial payload with appropriate token limit field based on model capabilities
                let mut payload = if ModelCapabilitiesManager::has_reasoning_capabilities(&model) {
                    json!({
                        "model": self.model_type,
                        "messages": messages_json,
                        "max_completion_tokens": result.remaining_output_tokens,
                        "stream": is_stream,
                    })
                } else {
                    json!({
                        "model": self.model_type,
                        "messages": messages_json,
                        "max_tokens": result.remaining_output_tokens,
                        "stream": is_stream,
                    })
                };

                let headers = json!({
                    "x-shinkai-version": env!("CARGO_PKG_VERSION"),
                    "x-shinkai-identity": node_name,
                    "x-shinkai-signature": signature,
                    "x-shinkai-metadata": metadata,
                });

                // Conditionally add functions to the payload if tools_json is not empty
                if !tools_json.is_empty() {
                    payload["functions"] = serde_json::Value::Array(tools_json.clone());
                }

                // Only add options to payload for non-reasoning models
                if !ModelCapabilitiesManager::has_reasoning_capabilities(&model) {
                    add_options_to_payload(&mut payload, config.as_ref());
                }

                // Print payload as a pretty JSON string
                match serde_json::to_string_pretty(&payload) {
                    Ok(pretty_json) => eprintln!("cURL Payload: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                };

                let mut payload_log = payload.clone();
                truncate_image_url_in_payload(&mut payload_log);
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload_log).as_str(),
                );

                if is_stream {
                    handle_streaming_response(
                        client,
                        url,
                        payload,
                        key.clone(),
                        inbox_name,
                        ws_manager_trait,
                        llm_stopper,
                        session_id,
                        Some(tools_json),
                        Some(headers),
                    )
                    .await
                } else {
                    handle_non_streaming_response(
                        client,
                        url,
                        payload,
                        key.clone(),
                        inbox_name,
                        llm_stopper,
                        ws_manager_trait,
                        Some(tools_json),
                        Some(headers),
                    )
                    .await
                }
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}
