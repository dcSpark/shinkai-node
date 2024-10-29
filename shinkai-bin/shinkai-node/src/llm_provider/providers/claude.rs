use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_db::schemas::ws_types::ToolMetadata;
use shinkai_db::schemas::ws_types::ToolStatus;
use shinkai_db::schemas::ws_types::ToolStatusType;
use shinkai_db::schemas::ws_types::WSMessageType;
use shinkai_db::schemas::ws_types::WSUpdateHandler;
use shinkai_db::schemas::ws_types::WidgetMetadata;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        job_config::JobConfig,
        llm_providers::serialized_llm_provider::{Claude, LLMProviderInterface},
        prompts::Prompt,
    },
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use tokio::sync::Mutex;

use crate::llm_provider::execution::chains::inference_chain_trait::FunctionCall;
use crate::llm_provider::{
    error::LLMProviderError, execution::chains::inference_chain_trait::LLMInferenceResponse, llm_stopper::LLMStopper,
};
use crate::managers::model_capabilities_manager::PromptResultEnum;

use super::{
    shared::ollama_api::{ollama_conversation_prepare_messages_with_tooling, ollama_prepare_messages},
    LLMService,
};

#[async_trait]
impl LLMService for Claude {
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

                let url = format!("{}{}", base_url, "v1/messages");

                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);
                let messages_result = if is_stream {
                    ollama_prepare_messages(&model, prompt)?
                } else {
                    ollama_conversation_prepare_messages_with_tooling(&model, prompt)?
                };

                let messages_json = match messages_result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Extract tools_json from the result
                let tools_json = messages_result.functions.unwrap_or_else(Vec::new);

                match serde_json::to_string_pretty(&tools_json) {
                    Ok(pretty_json) => eprintln!("Tools JSON: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize tools_json: {:?}", e),
                };

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_tokens": messages_result.remaining_tokens,
                });

                handle_non_streaming_response(
                    client,
                    url,
                    key.to_string(),
                    payload,
                    inbox_name,
                    ws_manager_trait,
                    llm_stopper,
                    Some(tools_json),
                )
                .await
            } else {
                return Err(LLMProviderError::ApiKeyNotSet);
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

async fn handle_non_streaming_response(
    client: &Client,
    url: String,
    api_key: String,
    payload: JsonValue,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<LLMStopper>,
    tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    let response_fut = client
        .post(url)
        .header("anthropic-version", "2023-06-01")
        .header("x-api-key", api_key)
        .header("content-type", "application/json")
        .json(&payload)
        .send();
    let mut response_fut = Box::pin(response_fut);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Some(ref inbox_name) = inbox_name {
                    if llm_stopper.should_stop(&inbox_name.to_string()) {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Info,
                            "LLM job stopped by user request",
                        );
                        llm_stopper.reset(&inbox_name.to_string());

                        return Ok(LLMInferenceResponse::new("".to_string(), json!({}), None, None));
                    }
                }
            },
            response = &mut response_fut => {
                let res = response?;
                let response_body = res.text().await?;
                let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

                if let Some(message) = response_json.get("message") {
                    if let Some(content) = message.get("content") {
                        if let Some(content_str) = content.as_str() {
                            let function_call = message
                                .get("tool_calls")
                                .and_then(|tool_calls| {
                                    tool_calls.as_array().and_then(|calls| {
                                        calls.iter().find_map(|call| {
                                            call.get("function").map(|function| {
                                                let name = function.get("name")?.as_str()?.to_string();
                                                let arguments = function.get("arguments")
                                                    .and_then(|args_value| args_value.as_object().cloned())
                                                    .unwrap_or_else(|| serde_json::Map::new());

                                                // Search for the tool_router_key in the tools array
                                                let tool_router_key = tools.as_ref().and_then(|tools_array| {
                                                    tools_array.iter().find_map(|tool| {
                                                        if tool.get("name")?.as_str()? == name {
                                                            tool.get("tool_router_key").and_then(|key| key.as_str().map(|s| s.to_string()))
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                });

                                                Some(FunctionCall { name, arguments, tool_router_key })
                                            })
                                        })
                                    })
                                })
                                .flatten();

                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Info,
                                format!("Function Call: {:?}", function_call).as_str(),
                            );


                            // Send WS message if a function call is detected
                            if let Some(ref manager) = ws_manager_trait {
                                if let Some(ref inbox_name) = inbox_name {
                                    if let Some(ref function_call) = function_call {
                                        let m = manager.lock().await;
                                        let inbox_name_string = inbox_name.to_string();

                                        // Serialize FunctionCall to JSON value
                                        let function_call_json = serde_json::to_value(function_call)
                                            .unwrap_or_else(|_| serde_json::json!({}));

                                        // Prepare ToolMetadata
                                        let tool_metadata = ToolMetadata {
                                            tool_name: function_call.name.clone(),
                                            tool_router_key: None,
                                            args: function_call_json
                                                .as_object()
                                                .cloned()
                                                .unwrap_or_default(),
                                            result: None,
                                            status: ToolStatus {
                                                type_: ToolStatusType::Running,
                                                reason: None,
                                            },
                                        };

                                        let ws_message_type = WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

                                        let _ = m
                                            .queue_message(
                                                WSTopic::Inbox,
                                                inbox_name_string,
                                                serde_json::to_string(&function_call)
                                                    .unwrap_or_else(|_| "{}".to_string()),
                                                ws_message_type,
                                                true,
                                            )
                                            .await;
                                    }
                                }
                            }

                            // Calculate tps
                            let eval_count = response_json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
                            let eval_duration =
                                response_json.get("eval_duration").and_then(|v| v.as_u64()).unwrap_or(1); // Avoid division by zero
                            let tps = if eval_duration > 0 {
                                Some(eval_count as f64 / eval_duration as f64 * 1e9)
                            } else {
                                None
                            };

                            break Ok(LLMInferenceResponse::new(
                                content_str.to_string(),
                                json!({}),
                                function_call,
                                tps,
                            ));
                        } else {
                            break Err(LLMProviderError::UnexpectedResponseFormat(
                                "Content is not a string".to_string(),
                            ));
                        }
                    } else {
                        break Err(LLMProviderError::UnexpectedResponseFormat(
                            "No content field in message".to_string(),
                        ));
                    }
                } else {
                    break Err(LLMProviderError::UnexpectedResponseFormat(
                        "No message field in response".to_string(),
                    ));
                }
            }
        }
    }
}
