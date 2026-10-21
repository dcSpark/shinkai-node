use std::sync::Arc;

use super::openai::{
    add_options_to_payload, handle_non_streaming_response_with_optional_auth,
    handle_streaming_response_with_optional_auth, truncate_image_url_in_payload, ToolIdentityMode,
};
use super::shared::openai_api::openai_prepare_messages;
use super::LLMService;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use shinkai_message_primitives::schemas::{
    inbox_name::InboxName,
    job_config::JobConfig,
    llm_providers::serialized_llm_provider::{Exo, LLMProviderInterface},
    prompts::Prompt,
    ws_types::WSUpdateHandler,
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
impl LLMService for Exo {
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
        // Exo accepts custom HuggingFace IDs through its OpenAI-compatible API.
        // https://github.com/exo-explore/exo/blob/main/docs/api.md
        let base_url = url.ok_or(LLMProviderError::UrlNotSet)?;
        let endpoint = super::openai_responses::openai_endpoint(base_url, "chat/completions");
        let result = openai_prepare_messages(&model, prompt)?;
        let messages = match result.messages {
            PromptResultEnum::Value(messages) => messages,
            _ => {
                return Err(LLMProviderError::UnexpectedPromptResultVariant(
                    "Expected Value variant in PromptResultEnum".into(),
                ))
            }
        };
        let tools = result.functions.unwrap_or_default();
        let stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);
        let mut payload = json!({"model":self.model_type,"messages":messages,"stream":stream,"max_tokens":result.remaining_output_tokens});
        if !tools.is_empty() {
            let mut wire_tools = tools.clone();
            for tool in &mut wire_tools {
                if let Some(function) = tool.get_mut("function").and_then(serde_json::Value::as_object_mut) {
                    function.remove("tool_router_key");
                }
            }
            payload["tools"] = json!(wire_tools);
        }
        add_options_to_payload(&mut payload, config.as_ref());
        // Exo uses max_tokens and enable_thinking, not OpenAI reasoning_effort.
        if let Some(tokens) = payload.as_object_mut().and_then(|p| p.remove("max_completion_tokens")) {
            payload["max_tokens"] = json!(tokens
                .as_u64()
                .unwrap_or(result.remaining_output_tokens as u64)
                .min(result.remaining_output_tokens as u64));
        }
        if let Some(thinking) = config.as_ref().and_then(|c| c.thinking) {
            payload["enable_thinking"] = json!(thinking);
        }
        if let Some(top_k) = config.as_ref().and_then(|c| c.top_k) {
            payload["top_k"] = json!(top_k);
        }
        if let Some(message_id) = tracing_message_id {
            let mut logged = payload.clone();
            truncate_image_url_in_payload(&mut logged);
            let _ = db.add_tracing(
                &message_id,
                inbox_name.as_ref().map(|i| i.get_value()).as_deref(),
                "llm_payload",
                &logged,
            );
        }
        // Local Exo does not require a configured API key. A key can be supplied for a proxy.
        let key = api_key.filter(|key| !key.trim().is_empty()).cloned();
        if stream {
            handle_streaming_response_with_optional_auth(
                client,
                endpoint,
                payload,
                key,
                inbox_name,
                ws_manager_trait,
                llm_stopper,
                Uuid::new_v4().to_string(),
                Some(tools),
                None,
                ToolIdentityMode::Fragments,
            )
            .await
        } else {
            handle_non_streaming_response_with_optional_auth(
                client,
                endpoint,
                payload,
                key,
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
    use mockito::Matcher;
    use shinkai_embedding::model_type::EmbeddingModelType;
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};

    #[tokio::test]
    async fn exo_optional_auth_headers_cover_streaming_and_json() {
        for stream in [true, false] {
            for key in [
                None,
                Some(String::new()),
                Some(" \t ".to_string()),
                Some("proxy-key".to_string()),
            ] {
                let mut server = mockito::Server::new_async().await;
                let response_call =
                    json!({"id":"native-weather","type":"function","function":{"name":"weather","arguments":"{}"}});
                let body = if stream {
                    let mut delta_call = response_call.clone();
                    delta_call["index"] = json!(0);
                    format!(
                        "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
                        json!({"choices":[{"index":0,"delta":{"tool_calls":[delta_call]}}]}),
                        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]})
                    )
                } else {
                    json!({"id":"response","object":"chat.completion","created":1,
                        "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2},
                        "choices":[{"index":0,"message":{"role":"assistant","content":null,"tool_calls":[response_call]},"finish_reason":"tool_calls"}]
                    }).to_string()
                };
                let mock = server
                    .mock("POST", "/v1/chat/completions")
                    .match_header(
                        "authorization",
                        key.as_ref()
                            .filter(|key| !key.trim().is_empty())
                            .map(|key| Matcher::Exact(format!("Bearer {key}")))
                            .unwrap_or(Matcher::Missing),
                    )
                    .match_body(Matcher::PartialJson(
                        json!({"model":"mlx-community/Llama-3.2-1B-Instruct-4bit","stream":stream}),
                    ))
                    .match_request(|request| {
                        let payload: serde_json::Value = serde_json::from_slice(request.body().unwrap()).unwrap();
                        payload["tools"][0]["function"]["name"] == "weather"
                            && payload["tools"][0]["function"].get("tool_router_key").is_none()
                    })
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
                        temp.path().join("test.db"),
                        String::new(),
                        EmbeddingModelType::default(),
                    )
                    .unwrap(),
                );
                let provider = Exo {
                    model_type: "mlx-community/Llama-3.2-1B-Instruct-4bit".into(),
                };
                let mut prompt = Prompt::new();
                prompt.add_content("Check weather".into(), SubPromptType::User, 100);
                prompt.add_tool(
                    json!({"function":{"name":"weather","description":"Get weather","tool_router_key":"local:::weather",
                    "parameters":{"type":"object","properties":{},"required":[]}}}),
                    SubPromptType::AvailableTool,
                    100,
                );
                let mut config = JobConfig::empty();
                config.stream = Some(stream);
                let response = provider
                    .call_api(
                        &Client::new(),
                        Some(&server.url()),
                        key.as_ref(),
                        prompt,
                        LLMProviderInterface::Exo(provider.clone()),
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
                assert_eq!(response.function_calls[0].id.as_deref(), Some("native-weather"));
                assert_eq!(response.function_calls[0].name, "weather");
                assert_eq!(
                    response.function_calls[0].tool_router_key.as_deref(),
                    Some("local:::weather")
                );
                assert!(response.function_calls[0].arguments.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn custom_exo_model_reads_sse_without_a_configured_api_key() {
        let mut server = mockito::Server::new_async().await;
        let body = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        let mock = server.mock("POST", "/v1/chat/completions")
            .match_header("authorization", Matcher::Missing)
            .match_body(Matcher::PartialJson(json!({"model":"mlx-community/Llama-3.2-1B-Instruct-4bit","stream":true,"temperature":0.2,"enable_thinking":false})))
            .match_request(|request| {
                let body: serde_json::Value = serde_json::from_slice(request.body().unwrap()).unwrap();
                body["tools"][0]["function"]["name"] == "weather"
                    && body["tools"][0]["function"].get("tool_router_key").is_none()
            })
            .with_header("content-type", "text/event-stream").with_body(body).create_async().await;
        let temp = tempfile::tempdir().unwrap();
        let db = Arc::new(
            SqliteManager::new(
                temp.path().join("test.db"),
                String::new(),
                EmbeddingModelType::default(),
            )
            .unwrap(),
        );
        let provider = Exo {
            model_type: "mlx-community/Llama-3.2-1B-Instruct-4bit".into(),
        };
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![SubPrompt::Content(SubPromptType::User, "Hello".into(), 100)]);
        prompt.add_tool(
            json!({"function": {
                "name": "weather", "description": "Get the weather", "tool_router_key": "local:::weather",
                "parameters": {"type": "object", "properties": {}, "required": []}
            }}),
            SubPromptType::AvailableTool,
            100,
        );
        let mut config = JobConfig::empty();
        config.stream = Some(true);
        config.temperature = Some(0.2);
        config.thinking = Some(false);
        let response = provider
            .call_api(
                &Client::new(),
                Some(&format!("{}/v1/", server.url())),
                None,
                prompt,
                LLMProviderInterface::Exo(provider.clone()),
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
        assert_eq!(response.response_string, "Hello");
    }
}
