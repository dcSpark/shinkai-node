use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use shinkai_message_primitives::schemas::{
    inbox_name::InboxName,
    job_config::JobConfig,
    llm_providers::serialized_llm_provider::{LLMProviderInterface, TogetherAI},
    prompts::Prompt,
    ws_types::WSUpdateHandler,
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    openai::{handle_non_streaming_response, handle_streaming_response},
    shared::openai_api::openai_prepare_messages,
    LLMService,
};
use crate::{
    llm_provider::{
        error::LLMProviderError, execution::chains::inference_chain_trait::LLMInferenceResponse,
        llm_stopper::LLMStopper,
    },
    managers::model_capabilities_manager::PromptResultEnum,
};

// Current Together models use the OpenAI-compatible chat API.
// https://docs.together.ai/reference/chat-completions
fn chat_payload(model: &str, messages: Value, tools: &[Value], max_tokens: usize, config: Option<&JobConfig>) -> Value {
    let mut payload = json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "stream": config.and_then(|c| c.stream).unwrap_or(true),
    });
    if !tools.is_empty() {
        let mut wire_tools = tools.to_vec();
        for tool in &mut wire_tools {
            if let Some(function) = tool.get_mut("function").and_then(Value::as_object_mut) {
                function.remove("tool_router_key");
            }
        }
        payload["tools"] = json!(wire_tools);
    }
    if let Some(config) = config {
        if let Some(value) = config.max_tokens {
            payload["max_tokens"] = json!(value.min(max_tokens as u64));
        }
        if let Some(value) = config.temperature {
            payload["temperature"] = json!(value);
        }
        if let Some(value) = config.top_p {
            payload["top_p"] = json!(value);
        }
        if let Some(value) = config.top_k {
            payload["top_k"] = json!(value);
        }
        if let Some(value) = config.seed {
            payload["seed"] = json!(value);
        }
        if let Some(value) = config.thinking {
            payload["reasoning"] = json!({"enabled": value});
        }
        if let Some(value) = &config.reasoning_effort {
            payload["reasoning_effort"] = json!(value);
        }
        if let Some(params) = config.other_model_params.as_ref().and_then(Value::as_object) {
            for key in [
                "frequency_penalty",
                "presence_penalty",
                "repetition_penalty",
                "min_p",
                "response_format",
                "stop",
                "tool_choice",
                "logit_bias",
                "logprobs",
                "reasoning",
                "reasoning_effort",
                "chat_template_kwargs",
            ] {
                if let Some(value) = params.get(key) {
                    payload[key] = value.clone();
                }
            }
        }
    }
    // Model-level restrictions apply after shared/custom settings.
    // https://docs.together.ai/docs/kimi-k3-quickstart
    if model == "moonshotai/Kimi-K3" {
        let effort = match payload["reasoning_effort"].as_str() {
            Some("low") => Some("low"),
            Some("medium" | "high") => Some("high"),
            Some("max") => Some("max"),
            _ => None,
        };
        if let Some(effort) = effort.filter(|_| payload["reasoning"]["enabled"] != false) {
            payload["reasoning_effort"] = json!(effort);
        } else {
            payload.as_object_mut().unwrap().remove("reasoning_effort");
        }
    } else if model == "MiniMaxAI/MiniMax-M3" {
        // MiniMax supports the reasoning toggle, but not adjustable effort.
        payload.as_object_mut().unwrap().remove("reasoning_effort");
    }
    payload
}

#[async_trait]
impl LLMService for TogetherAI {
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
        let base_url = url.ok_or(LLMProviderError::UrlNotSet)?;
        let key = api_key.ok_or(LLMProviderError::ApiKeyNotSet)?;
        let url = super::openai_responses::openai_endpoint(base_url, "chat/completions");
        let result = openai_prepare_messages(&model, prompt)?;
        let messages = match result.messages {
            PromptResultEnum::Value(value) => value,
            _ => {
                return Err(LLMProviderError::UnexpectedPromptResultVariant(
                    "Expected Value variant in PromptResultEnum".to_string(),
                ))
            }
        };
        let tools = result.functions.unwrap_or_default();
        let payload = chat_payload(
            &self.model_type,
            messages,
            &tools,
            result.remaining_output_tokens,
            config.as_ref(),
        );

        if let Some(msg_id) = tracing_message_id {
            if let Err(error) = db.add_tracing(
                &msg_id,
                inbox_name.as_ref().map(|i| i.get_value()).as_deref(),
                "llm_payload",
                &payload,
            ) {
                eprintln!("failed to add payload trace: {error}");
            }
        }
        if payload["stream"].as_bool().unwrap_or(true) {
            handle_streaming_response(
                client,
                url,
                payload,
                key.clone(),
                inbox_name,
                ws_manager_trait,
                llm_stopper,
                Uuid::new_v4().to_string(),
                Some(tools),
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
                Some(tools),
                None,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn together_reasoning_options_follow_each_model_contract() {
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        for (configured, expected) in [
            ("low", Some("low")),
            ("medium", Some("high")),
            ("high", Some("high")),
            ("max", Some("max")),
            ("invalid", None),
        ] {
            config.reasoning_effort = Some("low".into());
            config.other_model_params = Some(json!({"reasoning_effort": configured}));
            let kimi = chat_payload("moonshotai/Kimi-K3", json!([]), &[], 4096, Some(&config));
            assert_eq!(kimi["reasoning_effort"].as_str(), expected);
            assert_eq!(kimi["reasoning"]["enabled"], true);
            let minimax = chat_payload("MiniMaxAI/MiniMax-M3", json!([]), &[], 4096, Some(&config));
            assert!(minimax.get("reasoning_effort").is_none());
            assert_eq!(minimax["reasoning"]["enabled"], true);
        }
        config.other_model_params = None;
        config.thinking = Some(false);
        for model in ["moonshotai/Kimi-K3", "MiniMaxAI/MiniMax-M3"] {
            let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
            assert_eq!(payload["reasoning"]["enabled"], false);
            assert!(payload.get("reasoning_effort").is_none());
        }
    }

    #[test]
    fn current_together_chat_request_preserves_messages_and_tools() {
        let messages = json!([
            {"role": "system", "content": "Use the supplied tools."},
            {"role": "user", "content": [{"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}]},
            {"role": "assistant", "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "weather", "arguments": "{}"}}]},
            {"role": "tool", "tool_call_id": "call_1", "content": "Sunny"}
        ]);
        let tools = vec![json!({"type": "function", "function": {
            "name": "weather", "parameters": {"type": "object"}, "tool_router_key": "local:::weather"
        }})];
        let mut config = JobConfig::empty();
        config.stream = Some(false);
        config.max_tokens = Some(512);
        config.thinking = Some(true);
        config.reasoning_effort = Some("high".to_string());
        config.other_model_params = Some(json!({"reasoning": {"enabled": false}, "model": "ignored"}));
        let payload = chat_payload("moonshotai/Kimi-K3", messages.clone(), &tools, 4096, Some(&config));
        assert_eq!(payload["messages"], messages);
        assert_eq!(
            payload["tools"],
            json!([{"type": "function", "function": {
                "name": "weather", "parameters": {"type": "object"}
            }}])
        );
        assert_eq!(tools[0]["function"]["tool_router_key"], "local:::weather");
        assert_eq!(payload["model"], "moonshotai/Kimi-K3");
        assert_eq!(payload["max_tokens"], 512);
        assert_eq!(payload["stream"], false);
        assert_eq!(payload["reasoning"]["enabled"], false);
        assert!(payload.get("reasoning_effort").is_none());
        assert!(payload.get("prompt").is_none());
        assert!(payload.get("max_completion_tokens").is_none());
    }

    #[test]
    fn current_together_chat_request_caps_output_and_omits_empty_tools() {
        let mut config = JobConfig::empty();
        config.max_tokens = Some(100_000);
        config.thinking = Some(false);
        let payload = chat_payload("Qwen/Qwen3.5-9B", json!([]), &[], 4096, Some(&config));
        assert_eq!(payload["max_tokens"], 4096);
        assert_eq!(payload["reasoning"]["enabled"], false);
        assert!(payload.get("tools").is_none());
        assert!(payload.get("stop").is_none());
    }

    #[tokio::test]
    async fn current_together_uses_chat_endpoint_and_reads_tool_response() {
        use mockito::Matcher;
        use shinkai_embedding::model_type::EmbeddingModelType;
        use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer test-key")
            .match_body(Matcher::PartialJson(json!({
                "model": "moonshotai/Kimi-K3", "stream": false,
                "messages": [{"role": "user", "content": "Check the weather"}]
            })))
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "id": "chatcmpl-test", "object": "chat.completion", "created": 1,
                    "usage": {"prompt_tokens": 4, "completion_tokens": 8, "total_tokens": 12},
                    "choices": [{"index": 0, "message": {"role": "assistant", "content": null,
                        "tool_calls": [{"id": "call_weather", "type": "function", "function": {
                            "name": "weather", "arguments": "{\"city\":\"Santiago\"}"
                        }}]}, "finish_reason": "tool_calls"}]
                })
                .to_string(),
            )
            .create_async()
            .await;
        let temp = tempfile::tempdir().unwrap();
        let db = Arc::new(
            SqliteManager::new(
                temp.path().join("test.db"),
                String::new(),
                EmbeddingModelType::default(),
            )
            .unwrap(),
        );
        let provider = TogetherAI {
            model_type: "moonshotai/Kimi-K3".to_string(),
        };
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![SubPrompt::Content(
            SubPromptType::User,
            "Check the weather".to_string(),
            100,
        )]);
        let mut config = JobConfig::empty();
        config.stream = Some(false);
        let response = provider
            .call_api(
                &Client::new(),
                Some(&format!("{}/v1/", server.url())),
                Some(&"test-key".to_string()),
                prompt,
                LLMProviderInterface::TogetherAI(provider.clone()),
                None,
                None,
                Some(config),
                Arc::new(LLMStopper::new()),
                db,
                None,
            )
            .await
            .unwrap();
        mock.assert_async().await;
        assert_eq!(response.function_calls.len(), 1);
        assert_eq!(response.function_calls[0].name, "weather");
        assert_eq!(response.function_calls[0].arguments["city"], "Santiago");
        assert_eq!(response.function_calls[0].id.as_deref(), Some("call_weather"));
    }
}
