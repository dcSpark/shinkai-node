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
    openai::{handle_non_streaming_response, handle_streaming_response_with_identity_mode, ToolIdentityMode},
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
        // Shared UI settings apply only to verified models. Explicit custom parameters
        // below remain available for other Together endpoints.
        if matches!(
            model,
            "moonshotai/Kimi-K3"
                | "MiniMaxAI/MiniMax-M3"
                | "Qwen/Qwen3.5-9B"
                | "deepseek-ai/DeepSeek-V4-Pro-0813"
                | "deepseek-ai/DeepSeek-V4.1-Flash-1017"
        ) {
            if let Some(value) = config.thinking {
                payload["reasoning"] = json!({"enabled": value});
            }
            if let Some(value) = &config.reasoning_effort {
                payload["reasoning_effort"] = json!(value);
            }
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
    } else if model == "deepseek-ai/DeepSeek-V4.1-Flash-1017" {
        // Together requires top-level integer effort; DeepSeek defines these named budgets.
        // https://www.together.ai/models/deepseek-v4-1-flash-1017
        let effort = match payload["reasoning_effort"].as_str() {
            Some("low") => Some(50),
            Some("high") => Some(75),
            Some("max") => Some(100),
            _ => payload["reasoning_effort"]
                .as_u64()
                .filter(|value| (1..=100).contains(value)),
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

fn attach_together_reasoning(response: &mut LLMInferenceResponse) {
    let Some(reasoning) = response.reasoning_content.as_ref().filter(|value| !value.is_empty()) else {
        return;
    };
    // Together accepts reasoning_content on assistant history, including tool turns.
    // https://docs.together.ai/docs/kimi-k3-quickstart#preserve-the-thinking-history
    for call in &mut response.function_calls {
        let metadata = call.provider_metadata.get_or_insert_with(|| json!({}));
        if let Some(metadata) = metadata.as_object_mut() {
            metadata.insert("together_reasoning_content".into(), json!(reasoning));
        }
    }
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
        let mut response = if payload["stream"].as_bool().unwrap_or(true) {
            handle_streaming_response_with_identity_mode(
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
                ToolIdentityMode::Complete,
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
        }?;
        attach_together_reasoning(&mut response);
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn together_reasoning_survives_streaming_and_nonstreaming_tool_history() {
        use crate::llm_provider::execution::chains::inference_chain_trait::FunctionCall;
        use shinkai_embedding::model_type::EmbeddingModelType;
        use shinkai_message_primitives::schemas::subprompts::SubPromptType;

        for stream in [false, true] {
            let mut server = mockito::Server::new_async().await;
            let calls = json!([
                {"index":0, "id":"call_weather", "type":"function", "function":{"name":"weather", "arguments":"{\"city\":\"Santiago\"}"}},
                {"index":1, "id":"call_broken", "type":"function", "function":{"name":"broken", "arguments":"[1]"}}
            ]);
            let body = if stream {
                [
                    json!({"choices":[{"index":0, "delta":{"reasoning_content":"Preserve "}}]}),
                    json!({"choices":[{"index":0, "delta":{"reasoning":"the trace."}}]}),
                    json!({"choices":[{"index":0, "delta":{"tool_calls":calls}, "finish_reason":"tool_calls"}]}),
                ]
                .into_iter()
                .map(|chunk| format!("data: {chunk}\n\n"))
                .collect::<String>()
                    + "data: [DONE]\n\n"
            } else {
                json!({"id":"together-test", "object":"chat.completion", "created":1,
                    "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2},
                    "choices":[{"index":0,"message":{"role":"assistant", "content":null, "reasoning":"Preserve the trace.", "tool_calls":calls},"finish_reason":"tool_calls"}]
                }).to_string()
            };
            let mock = server
                .mock("POST", "/v1/chat/completions")
                .with_header(
                    "content-type",
                    if stream {
                        "text/event-stream"
                    } else {
                        "application/json"
                    },
                )
                .with_body(body)
                .create_async()
                .await;
            let temp = tempfile::tempdir().unwrap();
            let db = Arc::new(
                SqliteManager::new(
                    temp.path().join("history.db"),
                    String::new(),
                    EmbeddingModelType::default(),
                )
                .unwrap(),
            );
            let provider = TogetherAI {
                model_type: "moonshotai/Kimi-K3".into(),
            };
            let model = LLMProviderInterface::TogetherAI(provider.clone());
            let mut config = JobConfig::empty();
            config.stream = Some(stream);
            let response = provider
                .call_api(
                    &Client::new(),
                    Some(&server.url()),
                    Some(&"test-key".into()),
                    Prompt::new(),
                    model.clone(),
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
            assert_eq!(response.reasoning_content.as_deref(), Some("Preserve the trace."));
            assert_eq!(response.function_calls.len(), 2);
            for call in &response.function_calls {
                assert_eq!(
                    call.provider_metadata.as_ref().unwrap()["together_reasoning_content"],
                    "Preserve the trace."
                );
            }
            let broken = response
                .function_calls
                .iter()
                .find(|call| call.id.as_deref() == Some("call_broken"))
                .unwrap();
            assert!(broken.provider_metadata.as_ref().unwrap()["function_argument_error"].is_string());

            let stored = serde_json::to_string(&response.function_calls).unwrap();
            let calls: Vec<FunctionCall> = serde_json::from_str(&stored).unwrap();
            let mut history = Prompt::new();
            history.add_content(
                "Continue after these results.".into(),
                SubPromptType::UserLastMessage,
                100,
            );
            for call in &calls {
                history.add_function_call(json!({"name":call.name,"call_id":call.id,"arguments":call.arguments,"provider_metadata":call.provider_metadata}), 100);
            }
            for call in &calls {
                history.add_function_call_response(json!({"function_call":call,"response":"done"}), 100);
            }
            let history = Prompt::from_json(&history.to_json().unwrap()).unwrap();
            let prepared = openai_prepare_messages(&model, history).unwrap();
            let PromptResultEnum::Value(messages) = prepared.messages else {
                panic!("Expected messages")
            };
            let payload = chat_payload(
                &provider.model_type,
                messages,
                &[],
                prepared.remaining_output_tokens,
                None,
            );
            let messages = payload["messages"].as_array().unwrap();
            assert_eq!(messages.len(), 4); // User, one parallel assistant turn, and two results.
            assert_eq!(messages[1]["reasoning_content"], "Preserve the trace.");
            assert_eq!(messages[1]["tool_calls"].as_array().unwrap().len(), 2);
            for call in messages[1]["tool_calls"].as_array().unwrap() {
                assert!(call.get("provider_metadata").is_none());
                assert!(call["function"].get("provider_metadata").is_none());
            }
            assert_eq!(messages[2]["tool_call_id"], "call_weather");
            assert_eq!(messages[3]["tool_call_id"], "call_broken");
        }
    }

    #[test]
    fn together_request_uses_verified_output_budget_and_clamps_user_cap() {
        use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
        use shinkai_message_primitives::schemas::subprompts::SubPromptType;

        let model_id = "moonshotai/Kimi-K3";
        let model = LLMProviderInterface::TogetherAI(TogetherAI {
            model_type: model_id.into(),
        });
        let supported = ModelCapabilitiesManager::get_max_output_tokens(&model);
        let mut prompt = Prompt::new();
        prompt.add_content("Reply briefly.".into(), SubPromptType::UserLastMessage, 100);
        let prepared = openai_prepare_messages(&model, prompt).unwrap();
        let remaining = prepared.remaining_output_tokens;
        assert!(
            remaining > 8192,
            "A short prompt must permit more than the old 4096-token cap"
        );
        assert_eq!(remaining, supported);
        let PromptResultEnum::Value(messages) = prepared.messages else {
            panic!("Expected prepared chat messages");
        };

        for requested in [8192, 1024, remaining as u64 + 1] {
            let mut config = JobConfig::empty();
            config.max_tokens = Some(requested);
            let payload = chat_payload(model_id, messages.clone(), &[], remaining, Some(&config));
            assert_eq!(payload["max_tokens"].as_u64(), Some(requested.min(remaining as u64)));
            assert_eq!(payload["messages"], messages);
        }
        let payload = chat_payload(model_id, messages, &[], remaining, None);
        assert_eq!(payload["max_tokens"].as_u64(), Some(remaining as u64));
    }

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
    fn together_shared_reasoning_settings_require_verified_model() {
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        config.reasoning_effort = Some("high".into());
        config.temperature = Some(0.3);
        for model in ["togethercomputer/llama-2-70b-chat", "Qwen/Qwen3.5-9B-unknown"] {
            let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
            assert!(payload.get("reasoning").is_none(), "{model}");
            assert!(payload.get("reasoning_effort").is_none(), "{model}");
            assert_eq!(payload["temperature"], 0.3);
        }
        for (model, effort) in [
            ("moonshotai/Kimi-K3", json!("high")),
            ("MiniMaxAI/MiniMax-M3", Value::Null),
            ("Qwen/Qwen3.5-9B", json!("high")),
            ("deepseek-ai/DeepSeek-V4-Pro-0813", json!("high")),
            ("deepseek-ai/DeepSeek-V4.1-Flash-1017", json!(75)),
        ] {
            let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
            assert_eq!(payload["reasoning"], json!({"enabled": true}), "{model}");
            assert_eq!(payload["reasoning_effort"], effort, "{model}");
        }
        config.other_model_params = Some(json!({"reasoning": {"enabled": false}, "reasoning_effort": 42}));
        let payload = chat_payload("custom/model", json!([]), &[], 4096, Some(&config));
        assert_eq!(payload["reasoning"], json!({"enabled": false}));
        assert_eq!(payload["reasoning_effort"], 42);
    }

    #[test]
    fn together_deepseek_flash_uses_numeric_effort() {
        let model = "deepseek-ai/DeepSeek-V4.1-Flash-1017";
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        for (name, expected) in [("low", 50), ("high", 75), ("max", 100)] {
            config.reasoning_effort = Some(name.into());
            let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
            assert_eq!(payload["reasoning_effort"], expected);
            assert_eq!(payload["reasoning"], json!({"enabled": true}));
        }
        config.reasoning_effort = None;
        for value in [json!(1), json!(42), json!(100)] {
            config.other_model_params = Some(json!({"reasoning_effort": value}));
            let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
            assert_eq!(payload["reasoning_effort"], value);
        }
        for value in [
            json!(0),
            json!(101),
            json!(-1),
            json!(1.5),
            json!(true),
            json!("50"),
            json!("medium"),
            json!("none"),
        ] {
            config.other_model_params = Some(json!({"reasoning_effort": value}));
            let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
            assert!(payload.get("reasoning_effort").is_none(), "{value}");
        }
        config.other_model_params = Some(json!({"reasoning_effort": 75}));
        config.thinking = Some(false);
        let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
        assert_eq!(payload["reasoning"], json!({"enabled": false}));
        assert!(payload.get("reasoning_effort").is_none());
        config.thinking = Some(true);
        config.other_model_params = Some(json!({"reasoning": {"enabled": false}, "reasoning_effort": 75}));
        let payload = chat_payload(model, json!([]), &[], 4096, Some(&config));
        assert!(payload.get("reasoning_effort").is_none());
        config.other_model_params = None;
        config.reasoning_effort = Some("high".into());
        for other in [
            "deepseek-ai/DeepSeek-V4-Pro-0813",
            "deepseek-ai/DeepSeek-V4.1-Flash-1017-unknown",
        ] {
            let payload = chat_payload(other, json!([]), &[], 4096, Some(&config));
            if other == "deepseek-ai/DeepSeek-V4-Pro-0813" {
                assert_eq!(payload["reasoning_effort"], "high");
            } else {
                assert!(payload.get("reasoning_effort").is_none());
            }
        }
        let payload = chat_payload(model, json!([]), &[], 4096, None);
        assert!(payload.get("reasoning_effort").is_none());
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
