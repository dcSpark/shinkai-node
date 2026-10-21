use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::deepseek_api::deepseek_prepare_messages;
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::openai::{
    handle_non_streaming_response, handle_streaming_response, truncate_image_url_in_payload,
};
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{DeepSeek, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
impl LLMService for DeepSeek {
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
        db: Arc<SqliteManager>,
        tracing_message_id: Option<String>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                // DeepSeek API is compatible with OpenAI API format
                let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                // Use the OpenAI message preparation since DeepSeek API is compatible
                let result = deepseek_prepare_messages(&model, prompt, session_id.clone())?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Extract tools_json from the result
                let tools_json = result.functions.unwrap_or_else(Vec::new);

                // DeepSeek uses max_tokens for both thinking and non-thinking models.
                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_tokens": result.remaining_output_tokens,
                    "stream": is_stream,
                });

                // Conditionally add tools to the payload if present. The tools list
                // provided by message preparation is already in the correct format
                // expected by OpenAI-compatible APIs (each entry typically has
                // {"type":"function", "function": { name, parameters, ... }}),
                // so we add it directly without additional wrapping.
                if !tools_json.is_empty() {
                    payload["tools"] = serde_json::Value::Array(tools_json.clone());
                }

                add_deepseek_options(&mut payload, config.as_ref(), &self.model_type);
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

                if let Some(ref msg_id) = tracing_message_id {
                    if let Err(e) = db.add_tracing(
                        msg_id,
                        inbox_name.as_ref().map(|i| i.get_value()).as_deref(),
                        "llm_payload",
                        &payload_log,
                    ) {
                        eprintln!("failed to add payload trace: {:?}", e);
                    }
                }

                let mut response = if is_stream {
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
                        None,
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
                        None,
                    )
                    .await
                }?;
                if let Some(reasoning) = response.reasoning_content.as_ref() {
                    for call in &mut response.function_calls {
                        call.provider_metadata = Some(json!({"deepseek_reasoning_content": reasoning}));
                    }
                }
                Ok(response)
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

// Keep this list separate from OpenAI: DeepSeek does not accept OpenAI-only
// request fields such as max_completion_tokens, seed, or service_tier.
fn add_deepseek_options(payload: &mut serde_json::Value, config: Option<&JobConfig>, model: &str) {
    // Current documented IDs and the accepted V4 Flash compatibility aliases.
    let supports_thinking_options = matches!(
        model,
        "deepseek-flash" | "deepseek-v4-pro" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp"
    );
    if let Some(value) = config
        .and_then(|c| c.temperature)
        .or_else(|| std::env::var("LLM_TEMPERATURE").ok()?.parse::<f64>().ok())
    {
        payload["temperature"] = json!(value);
    }
    if let Some(value) = config
        .and_then(|c| c.top_p)
        .or_else(|| std::env::var("LLM_TOP_P").ok()?.parse::<f64>().ok())
    {
        payload["top_p"] = json!(value);
    }
    if let Some(value) = config
        .and_then(|c| c.max_tokens)
        .or_else(|| std::env::var("LLM_MAX_TOKENS").ok()?.parse::<u64>().ok())
    {
        payload["max_tokens"] = json!(value);
    }
    if let Some(thinking) = config.and_then(|c| c.thinking) {
        payload["thinking"] = json!({"type": if thinking { "enabled" } else { "disabled" }});
    }
    if let Some(effort) = config.and_then(|c| c.reasoning_effort.as_ref()) {
        payload["reasoning_effort"] = json!(effort);
    }
    if let Some(params) = config
        .and_then(|c| c.other_model_params.as_ref())
        .and_then(|p| p.as_object())
    {
        for (key, value) in params {
            match key.as_str() {
                // Accept the old configuration name, but use DeepSeek's wire field.
                "max_completion_tokens" if !params.contains_key("max_tokens") => payload["max_tokens"] = value.clone(),
                "max_tokens" | "thinking" | "reasoning_effort" | "response_format" | "stop" | "stream_options"
                | "tool_choice" | "logprobs" | "top_logprobs" => {
                    payload[key] = value.clone();
                }
                _ => (),
            }
        }
    }
    // Check after other_model_params too, so shared settings cannot bypass the model gate.
    if !supports_thinking_options {
        payload.as_object_mut().unwrap().remove("thinking");
        payload.as_object_mut().unwrap().remove("reasoning_effort");
    }
    if payload["stream"] != true {
        payload.as_object_mut().unwrap().remove("stream_options");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

    #[test]
    fn legacy_deepseek_models_omit_new_thinking_options() {
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        config.reasoning_effort = Some("high".into());
        config.max_tokens = Some(4096);
        config.other_model_params = Some(json!({"thinking": {"type": "enabled"}, "reasoning_effort": "max"}));
        for model in ["deepseek-chat", "deepseek-reasoner", "deepseek-custom"] {
            let mut payload = json!({"stream": true});
            add_deepseek_options(&mut payload, Some(&config), model);
            assert!(payload.get("thinking").is_none(), "{}", model);
            assert!(payload.get("reasoning_effort").is_none(), "{}", model);
            assert_eq!(payload["max_tokens"], 4096);
        }
        for model in [
            "deepseek-flash",
            "deepseek-v4-pro",
            "deepseek-v4-flash",
            "deepseek-v4-flash-vision-exp",
        ] {
            let mut payload = json!({"stream": true});
            add_deepseek_options(&mut payload, Some(&config), model);
            assert_eq!(payload["thinking"]["type"], "enabled");
            assert_eq!(payload["reasoning_effort"], "max");
        }
    }

    #[test]
    fn deepseek_tool_calls_keep_requested_thinking() {
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        config.reasoning_effort = Some("high".to_string());
        let mut payload = json!({"tools": [{"type": "function"}], "stream": true});
        add_deepseek_options(&mut payload, Some(&config), "deepseek-flash");
        assert_eq!(payload["thinking"]["type"], "enabled");
        assert_eq!(payload["reasoning_effort"], "high");
    }

    #[test]
    fn deepseek_options_use_provider_fields() {
        let mut config = JobConfig::empty();
        config.max_tokens = Some(4096);
        config.thinking = Some(false);
        config.reasoning_effort = Some("none".to_string());
        config.other_model_params = Some(json!({
            "max_completion_tokens": 8192,
            "stream_options": {"include_usage": true},
            "service_tier": "auto"
        }));
        let mut payload = json!({"stream": false});
        add_deepseek_options(&mut payload, Some(&config), "deepseek-flash");
        assert_eq!(payload["max_tokens"], 8192);
        assert_eq!(payload["thinking"]["type"], "disabled");
        assert_eq!(payload["reasoning_effort"], "none");
        assert!(payload.get("max_completion_tokens").is_none());
        assert!(payload.get("stream_options").is_none());
        assert!(payload.get("service_tier").is_none());
    }

    #[test]
    fn test_deepseek_provider_creation() {
        let deepseek = DeepSeek {
            model_type: "deepseek-chat".to_string(),
        };
        assert_eq!(deepseek.model_type, "deepseek-chat");
    }

    #[test]
    fn test_deepseek_serialized_provider() {
        let provider = SerializedLLMProvider {
            id: "test-id".to_string(),
            name: None,
            description: None,
            full_identity_name: ShinkaiName::new("@@test.shinkai/main/agent/deepseek_test".to_string()).unwrap(),
            model: LLMProviderInterface::DeepSeek(DeepSeek {
                model_type: "deepseek-chat".to_string(),
            }),
            api_key: Some("test-key".to_string()),
            external_url: Some("https://api.deepseek.com".to_string()),
        };

        if let LLMProviderInterface::DeepSeek(deepseek) = &provider.model {
            assert_eq!(deepseek.model_type, "deepseek-chat");
        } else {
            panic!("Expected DeepSeek provider");
        }
    }
}
