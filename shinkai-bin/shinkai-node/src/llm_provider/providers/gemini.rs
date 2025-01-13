use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::openai_api::openai_prepare_messages;
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Gemini, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSMessageType;
use shinkai_message_primitives::schemas::ws_types::WSMetadata;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::error::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct GeminiStreamingResponse {
    choices: Vec<StreamingChoice>,
    created: u64,
    model: String,
    object: String,
}

#[derive(Debug, Deserialize)]
struct StreamingChoice {
    delta: StreamingDelta,
    index: u32,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamingDelta {
    content: Option<String>,
    role: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Serialize, Deserialize)]
struct Choice {
    message: Message,
    finish_reason: String,
    index: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[async_trait]
impl LLMService for Gemini {
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
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let base_url = if base_url.ends_with('/') {
                    base_url.to_string()
                } else {
                    format!("{}/", base_url)
                };

                let session_id = Uuid::new_v4().to_string();
                let url = format!("https://generativelanguage.googleapis.com/v1beta/openai/chat/completions");

                let result = openai_prepare_messages(&model, prompt)?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_tokens": result.remaining_output_tokens,
                    "stream": true,
                    "temperature": 0.9,
                    "top_p": 1,
                });

                // Print payload as a pretty JSON string only if IS_TESTING is true
                if std::env::var("LOG_ALL").unwrap_or_default() == "true"
                    || std::env::var("LOG_ALL").unwrap_or_default() == "1"
                {
                    match serde_json::to_string_pretty(&payload) {
                        Ok(pretty_json) => eprintln!("cURL Payload: {}", pretty_json),
                        Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                    };
                }

                let res = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Bearer {}", key))
                    .json(&payload)
                    .send()
                    .await?;
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Status: {:?}", res.status()).as_str(),
                );

                let mut stream = res.bytes_stream();
                let mut response_text = String::new();
                let mut buffer = String::new();
                let mut is_done = false;
                let mut finish_reason = None;

                while let Some(item) = stream.next().await {
                    match item {
                        Ok(chunk) => {
                            process_chunk(
                                &chunk,
                                &mut buffer,
                                &mut response_text,
                                &session_id,
                                &ws_manager_trait,
                                &inbox_name,
                                &mut is_done,
                                &mut finish_reason,
                            )
                            .await?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                format!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source())
                                    .as_str(),
                            );
                            return Err(LLMProviderError::NetworkError(e.to_string()));
                        }
                    }
                }
                Ok(LLMInferenceResponse::new(response_text, json!({}), Vec::new(), None))
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_chunk(
    chunk: &[u8],
    buffer: &mut String,
    response_text: &mut String,
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    is_done: &mut bool,
    finish_reason: &mut Option<String>,
) -> Result<(), LLMProviderError> {
    let chunk_str = String::from_utf8_lossy(chunk);
    
    // Check for [DONE] message
    if chunk_str.contains("[DONE]") {
        *is_done = true;
        return Ok(());
    }

    // Remove "data: " prefix if present
    let json_str = if chunk_str.starts_with("data: ") {
        chunk_str.trim_start_matches("data: ").to_string()
    } else {
        chunk_str.to_string()
    };

    // Try to parse the JSON
    match serde_json::from_str::<JsonValue>(&json_str) {
        Ok(value) => {
            process_gemini_response(
                value,
                response_text,
                session_id,
                ws_manager_trait,
                inbox_name,
                is_done,
                finish_reason,
            )
            .await?;
        }
        Err(e) => {
            eprintln!("Failed to parse JSON: {:?}", e);
        }
    }

    Ok(())
}

async fn process_gemini_response(
    value: JsonValue,
    response_text: &mut String,
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    is_done: &mut bool,
    finish_reason: &mut Option<String>,
) -> Result<(), LLMProviderError> {
    if let Ok(response) = serde_json::from_value::<GeminiStreamingResponse>(value) {
        for choice in &response.choices {
            if let Some(content) = &choice.delta.content {
                response_text.push_str(content);
                finish_reason.clone_from(&choice.finish_reason);

                if let Some(ref manager) = ws_manager_trait {
                    if let Some(ref inbox_name) = inbox_name {
                        let m = manager.lock().await;
                        let inbox_name_string = inbox_name.to_string();

                        let metadata = WSMetadata {
                            id: Some(session_id.to_string()),
                            is_done: *is_done,
                            done_reason: finish_reason.clone(),
                            total_duration: None,
                            eval_count: None,
                        };

                        let ws_message_type = WSMessageType::Metadata(metadata);

                        let _ = m
                            .queue_message(
                                WSTopic::Inbox,
                                inbox_name_string.clone(),
                                content.to_string(),
                                ws_message_type,
                                true,
                            )
                            .await;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_process_first_chunk() {
        let chunk = b"[{
            \"choices\": [
                {
                    \"delta\": {
                        \"content\": \"The\"
                    },
                    \"finish_reason\": \"stop\",
                    \"index\": 0
                }
            ]
        }";

        let mut buffer = String::new();
        let mut response_text = String::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let mut is_done = false;
        let mut finish_reason = None;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &mut is_done,
            &mut finish_reason,
        )
        .await
        .unwrap();

        assert_eq!(response_text, "The");
        assert!(!is_done);
        assert_eq!(finish_reason, Some("stop".to_string()));
    }

    #[tokio::test]
    async fn test_process_second_chunk() {
        let chunk = b",
        {
            \"choices\": [
                {
                    \"delta\": {
                        \"content\": \" Roman Empire was a vast and powerful civilization that dominated much of Europe, North Africa\"
                    },
                    \"finish_reason\": \"stop\",
                    \"index\": 0
                }
            ]
        }";

        let mut buffer = String::new();
        let mut response_text = String::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let mut is_done = false;
        let mut finish_reason = None;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &mut is_done,
            &mut finish_reason,
        )
        .await
        .unwrap();

        assert_eq!(
            response_text,
            " Roman Empire was a vast and powerful civilization that dominated much of Europe, North Africa"
        );
        assert!(!is_done);
        assert_eq!(finish_reason, Some("stop".to_string()));
    }

    #[tokio::test]
    async fn test_process_last_chunk() {
        let chunk = b",
        {
            \"choices\": [
                {
                    \"delta\": {
                        \"content\": \" in greater detail. \\n\"
                    },
                    \"finish_reason\": \"stop\",
                    \"index\": 0
                }
            ]
        }]";

        let mut buffer = String::new();
        let mut response_text = String::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let mut is_done = false;
        let mut finish_reason = None;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &mut is_done,
            &mut finish_reason,
        )
        .await
        .unwrap();

        assert_eq!(response_text, " in greater detail. \n");
        assert!(is_done);
        assert_eq!(finish_reason, Some("stop".to_string()));
    }
}
