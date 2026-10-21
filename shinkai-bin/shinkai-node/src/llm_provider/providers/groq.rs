use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::shared::groq_api::groq_prepare_messages;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Groq, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
impl LLMService for Groq {
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
                let url = format!("{}{}", base_url, "/chat/completions");
                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                let result = groq_prepare_messages(&model, prompt)?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Extract tools_json from the result and keep original for matching
                let tools_json = result.functions.clone().unwrap_or_else(Vec::new);
                let original_tools = tools_json.clone(); // Keep original for matching

                // Set up initial payload with appropriate token limit field based on model capabilities
                let mut payload = initial_groq_payload(
                    &self.model_type,
                    messages_json,
                    result.remaining_output_tokens,
                    is_stream,
                    ModelCapabilitiesManager::has_reasoning_capabilities(&model),
                );

                // Add tools to payload if they exist, but remove tool_router_key
                if !tools_json.is_empty() {
                    let tools: Vec<JsonValue> = tools_json
                        .iter()
                        .map(|tool| {
                            let mut tool_copy = tool.clone();
                            // Remove tool_router_key from the function object
                            if let Some(function_obj) = tool_copy.get_mut("function") {
                                if let Some(obj) = function_obj.as_object_mut() {
                                    obj.remove("tool_router_key");
                                }
                            }
                            tool_copy
                        })
                        .collect();
                    payload["tools"] = serde_json::Value::Array(tools);
                    payload["tool_choice"] = json!("auto");
                }

                // Clean up message content if needed
                if let Some(messages) = payload.get_mut("messages") {
                    if let Some(messages_array) = messages.as_array_mut() {
                        for message in messages_array {
                            if let Some(content) = message.get_mut("content") {
                                if let Some(content_array) = content.as_array() {
                                    // If content is an array with a single text element, simplify it
                                    if content_array.len() == 1 {
                                        if let Some(text_obj) = content_array[0].get("text") {
                                            *content = text_obj.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Handle reasoning parameters for supported models, otherwise add regular options
                if ModelCapabilitiesManager::has_reasoning_capabilities(&model) {
                    // Check thinking parameter from JobConfig
                    let thinking_enabled = config.as_ref().and_then(|c| c.thinking).unwrap_or(true); // Default to true if not set

                    if self.model_type.starts_with("qwen/qwen3") {
                        if thinking_enabled {
                            payload["reasoning_effort"] = serde_json::json!("default");
                            payload["reasoning_format"] = serde_json::json!("parsed");
                        } else {
                            payload["reasoning_effort"] = serde_json::json!("none");
                            payload["reasoning_format"] = serde_json::json!("hidden");
                        }
                    } else if self.model_type.starts_with("openai/gpt-oss") {
                        if thinking_enabled {
                            payload["reasoning_effort"] = serde_json::json!(config
                                .as_ref()
                                .and_then(|c| c.reasoning_effort.clone())
                                .unwrap_or_else(|| "medium".to_string()));
                            payload["include_reasoning"] = serde_json::json!(true);
                        } else {
                            // Don't set reasoning_effort when thinking is disabled
                            payload["include_reasoning"] = serde_json::json!(false);
                        }
                    } else {
                        // For other reasoning models, use default parsed format when thinking enabled
                        if thinking_enabled {
                            payload["reasoning_format"] = serde_json::json!("parsed");
                        } else {
                            payload["reasoning_format"] = serde_json::json!("hidden");
                        }
                    }
                }
                add_options_to_payload(&mut payload, config.as_ref());

                let payload_log = payload.clone();
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

                if is_stream {
                    handle_streaming_response(
                        client,
                        url,
                        payload,
                        key.clone(),
                        inbox_name,
                        ws_manager_trait.clone(),
                        llm_stopper,
                        session_id,
                        Some(original_tools), // Pass original tools with router keys
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
                        ws_manager_trait.clone(),
                        Some(original_tools), // Pass original tools with router keys
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

// Groq uses the OpenAI Chat Completions delta format. Share its indexed
// accumulator and execution guards instead of parsing each argument fragment.
async fn handle_streaming_response(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<LLMStopper>,
    session_id: String,
    tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    super::openai::handle_streaming_response_with_policy(
        client,
        url,
        payload,
        Some(api_key),
        inbox_name,
        ws_manager_trait,
        llm_stopper,
        session_id,
        tools,
        None,
        super::openai::ToolIdentityMode::Fragments,
        super::openai::ChatStreamPolicy::Groq,
    )
    .await
}

async fn handle_non_streaming_response(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    llm_stopper: Arc<LLMStopper>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    super::openai::handle_non_streaming_response(
        client,
        url,
        payload,
        api_key,
        inbox_name,
        llm_stopper,
        ws_manager_trait,
        tools,
        None,
    )
    .await
}

fn initial_groq_payload(
    model: &str,
    messages: JsonValue,
    remaining_output_tokens: usize,
    is_stream: bool,
    is_reasoning_model: bool,
) -> JsonValue {
    let mut payload = json!({"model": model, "messages": messages, "stream": is_stream});
    if is_reasoning_model {
        payload["max_completion_tokens"] = json!(remaining_output_tokens);
    }
    payload
}

fn add_options_to_payload(payload: &mut serde_json::Value, config: Option<&JobConfig>) {
    // Keep the prepared ceiling before config, environment, and custom options are applied.
    let prepared_output_budget = payload["max_completion_tokens"].as_u64();

    // Helper function to read and parse environment variables
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        std::env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }

    // Helper function to get value from env or config
    fn get_value<T: Clone + std::str::FromStr>(env_key: &str, config_value: Option<&T>) -> Option<T> {
        config_value.cloned().or_else(|| read_env_var::<T>(env_key))
    }

    // Read options from environment variables or config and add them directly to the payload
    if let Some(seed) = get_value("LLM_SEED", config.and_then(|c| c.seed.as_ref())) {
        payload["seed"] = serde_json::json!(seed);
    }
    if let Some(temp) = get_value("LLM_TEMPERATURE", config.and_then(|c| c.temperature.as_ref())) {
        payload["temperature"] = serde_json::json!(temp);
    }
    if let Some(top_p) = get_value("LLM_TOP_P", config.and_then(|c| c.top_p.as_ref())) {
        payload["top_p"] = serde_json::json!(top_p);
    }

    if let Some(max_tokens) = get_value("LLM_MAX_TOKENS", config.and_then(|c| c.max_tokens.as_ref())) {
        payload["max_completion_tokens"] = serde_json::json!(max_tokens);
    }

    // Handle other model params
    if let Some(other_params) = config.and_then(|c| c.other_model_params.as_ref()) {
        if let Some(obj) = other_params.as_object() {
            for (key, value) in obj {
                match key.as_str() {
                    "frequency_penalty" => payload["frequency_penalty"] = value.clone(),
                    "logit_bias" => payload["logit_bias"] = value.clone(),
                    "logprobs" => payload["logprobs"] = value.clone(),
                    "top_logprobs" => payload["top_logprobs"] = value.clone(),
                    "n" => payload["n"] = value.clone(),
                    "presence_penalty" => payload["presence_penalty"] = value.clone(),
                    "response_format" => payload["response_format"] = value.clone(),
                    "service_tier" => payload["service_tier"] = value.clone(),
                    "stop" => payload["stop"] = value.clone(),
                    "stream_options" => payload["stream_options"] = value.clone(),
                    "parallel_tool_calls" => payload["parallel_tool_calls"] = value.clone(),
                    "max_completion_tokens" => payload["max_completion_tokens"] = value.clone(),
                    _ => (),
                };
            }
        }
    }
    if let Some(budget) = prepared_output_budget {
        let requested_limit = payload["max_completion_tokens"]
            .as_u64()
            .filter(|limit| *limit > 0)
            .unwrap_or(budget);
        payload["max_completion_tokens"] = json!(requested_limit.min(budget));
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};

    fn prepared_request(id: &str) -> (JsonValue, u64) {
        let model = LLMProviderInterface::Groq(Groq { model_type: id.into() });
        assert!(ModelCapabilitiesManager::has_reasoning_capabilities(&model));
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![SubPrompt::Content(SubPromptType::User, "Hello".into(), 100)]);
        let prepared = groq_prepare_messages(&model, prompt).unwrap();
        let budget = prepared.remaining_output_tokens as u64;
        assert!(budget > 1024);
        let PromptResultEnum::Value(messages) = prepared.messages else {
            panic!("Expected prepared JSON messages");
        };
        let payload = initial_groq_payload(
            id,
            messages.clone(),
            prepared.remaining_output_tokens,
            false,
            ModelCapabilitiesManager::has_reasoning_capabilities(&model),
        );
        assert_eq!(payload["max_completion_tokens"], budget);
        assert_eq!(payload["messages"], messages);
        assert_eq!(payload["model"], id);
        assert_eq!(payload["stream"], false);
        (payload, budget)
    }

    #[test]
    fn groq_reasoning_requests_keep_lower_limits_within_prepared_budget() {
        for id in ["qwen/qwen3.8-27b", "qwen/qwen3.6-27b", "openai/gpt-oss-120b"] {
            let (mut initial, budget) = prepared_request(id);
            // Options must not change the reasoning or sampling fields already selected by the request.
            initial["reasoning_effort"] = json!("medium");
            initial["temperature"] = json!(0.25);
            let mut config = JobConfig::empty();
            config.temperature = Some(0.25);
            for (limit, expected) in [(1024, 1024), (budget + 1, budget), (0, budget)] {
                config.max_tokens = Some(limit);
                let mut payload = initial.clone();
                add_options_to_payload(&mut payload, Some(&config));
                assert_eq!(payload["max_completion_tokens"], expected, "{}", id);
            }
            config.max_tokens = Some(budget + 1);
            for (limit, expected) in [
                (json!(512), 512),
                (json!(budget + 1), budget),
                (json!(null), budget),
                (json!(0), budget),
                (json!(-1), budget),
                (json!(1.5), budget),
                (json!("invalid"), budget),
            ] {
                config.other_model_params = Some(json!({"max_completion_tokens": limit}));
                let mut payload = initial.clone();
                add_options_to_payload(&mut payload, Some(&config));
                assert_eq!(payload["max_completion_tokens"], expected, "{}", id);
                assert_eq!(payload["reasoning_effort"], initial["reasoning_effort"]);
                assert_eq!(payload["temperature"], initial["temperature"]);
                assert_eq!(payload["messages"], initial["messages"]);
                assert_eq!(payload["model"], id);
                assert_eq!(payload["stream"], false);
                assert!(payload.get("max_tokens").is_none());
            }
        }
    }

    #[test]
    fn groq_environment_limit_keeps_prepared_budget() {
        const CHILD_MARKER: &str = "SHINKAI_GROQ_LIMIT_TEST_CHILD";
        if std::env::var_os(CHILD_MARKER).is_none() {
            // A child process keeps environment changes out of the other provider tests.
            for limit in [1024, u64::MAX] {
                let output = std::process::Command::new(std::env::current_exe().unwrap())
                    .arg("groq_environment_limit_keeps_prepared_budget")
                    .env(CHILD_MARKER, "1")
                    .env("LLM_MAX_TOKENS", limit.to_string())
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "Groq environment limit test failed for {}",
                    limit
                );
                assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            }
            return;
        }
        let limit = std::env::var("LLM_MAX_TOKENS").unwrap().parse::<u64>().unwrap();
        for id in ["qwen/qwen3.8-27b", "qwen/qwen3.6-27b", "openai/gpt-oss-120b"] {
            let (mut payload, budget) = prepared_request(id);
            add_options_to_payload(&mut payload, None);
            assert_eq!(payload["max_completion_tokens"], limit.min(budget), "{}", id);
        }
    }
}

#[cfg(test)]
mod response_tests {
    use super::*;
    use crate::llm_provider::execution::chains::generic_chain::generic_inference_chain::execute_valid_function_call;
    use crate::managers::tool_router::ToolCallFunctionResponse;

    async fn try_response(body: String, streaming: bool) -> Result<LLMInferenceResponse, LLMProviderError> {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/chat/completions")
            .with_header(
                "content-type",
                if streaming {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            )
            .with_body(body)
            .create_async()
            .await;
        let tools = Some(vec![json!({"type":"function","function":{
            "name":"weather", "tool_router_key":"weather-router"}})]);
        let client = Client::new();
        let url = format!("{}/chat/completions", server.url());
        let stopper = Arc::new(LLMStopper::new());
        let result = if streaming {
            handle_streaming_response(
                &client,
                url,
                json!({}),
                "test".into(),
                None,
                None,
                stopper,
                "test".into(),
                tools,
            )
            .await
        } else {
            handle_non_streaming_response(&client, url, json!({}), "test".into(), None, stopper, None, tools).await
        };
        mock.assert_async().await;
        result
    }

    async fn response(body: String, streaming: bool) -> LLMInferenceResponse {
        try_response(body, streaming).await.unwrap()
    }

    fn sse(events: Vec<JsonValue>, done: bool) -> String {
        let mut body = events
            .into_iter()
            .map(|event| format!("data: {}\n\n", event))
            .collect::<String>();
        if done {
            body.push_str("data: [DONE]\n\n");
        }
        body
    }

    #[tokio::test]
    async fn groq_stream_accumulates_parallel_arguments_and_keeps_native_identity() {
        let body = sse(
            vec![
                json!({"choices":[{"index":0,"delta":{"reasoning":"Plan", "content":"Start", "tool_calls":[
                    {"index":0,"id":"call_a","type":"function","function":{"name":"weather","arguments":"{\"city\":"}},
                    {"index":1,"id":"call_b","type":"function","function":{"name":"weather","arguments":"{\"city\":"}}
                ]}}]}),
                json!({"choices":[{"index":0,"delta":{"tool_calls":[
                    {"index":1,"function":{"arguments":"\"Rome\"}"}},
                    {"index":0,"function":{"arguments":"\"Paris\"}"}}
                ]}}]}),
                json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
            ],
            true,
        );
        let result = response(body, true).await;
        assert_eq!(result.response_string, "Start");
        assert_eq!(result.reasoning_content.as_deref(), Some("Plan"));
        assert_eq!(result.function_calls.len(), 2);
        for (call, id, city) in [
            (&result.function_calls[0], "call_a", "Paris"),
            (&result.function_calls[1], "call_b", "Rome"),
        ] {
            assert_eq!(call.id.as_deref(), Some(id));
            assert_eq!(call.name, "weather");
            assert_eq!(call.arguments["city"], city);
            assert_eq!(call.tool_router_key.as_deref(), Some("weather-router"));
            assert!(call.provider_metadata.is_none());
        }
    }

    #[tokio::test]
    async fn groq_malformed_arguments_cannot_execute_zero_parameter_tools() {
        for streaming in [false, true] {
            for arguments in ["{", "[]", "null", "{}"] {
                let valid = arguments == "{}";
                let body = if streaming {
                    sse(
                        vec![
                            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                                {"index":0,"id":"call_guard","type":"function","function":{"name":"weather","arguments":arguments}}
                            ]}}]}),
                            json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
                        ],
                        true,
                    )
                } else {
                    json!({"id":"chatcmpl-groq-test", "object":"chat.completion", "created":1720000000,
                        "model":"qwen/qwen3.8-27b", "system_fingerprint":"fp_test",
                        "usage":{"prompt_tokens":10,"completion_tokens":8,"total_tokens":18},
                        "x_groq":{"id":"req_test"},
                        "choices":[{"index":0,"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"reasoning":"Plan","tool_calls":[
                        {"id":"call_guard","type":"function","function":{"name":"weather","arguments":arguments}}
                    ]}}]}).to_string()
                };
                let result = response(body, streaming).await;
                let call = &result.function_calls[0];
                assert_eq!(call.id.as_deref(), Some("call_guard"));
                assert_eq!(call.tool_router_key.as_deref(), Some("weather-router"));
                let mut executed = false;
                let execution_call = call.clone();
                let outcome = execute_valid_function_call(call, async {
                    executed = true;
                    Ok(ToolCallFunctionResponse {
                        function_call: execution_call,
                        response: "executed".into(),
                    })
                })
                .await;
                assert_eq!(executed, valid, "streaming={streaming}, arguments={arguments}");
                assert_eq!(outcome.is_ok(), valid);
            }
        }
    }

    #[tokio::test]
    async fn groq_raw_eof_requires_confirmation_even_for_valid_arguments() {
        for arguments in ["", "{}", "{\"city\":\"Paris\"}"] {
            let body = sse(
                vec![json!({"choices":[{"index":0,"delta":{"tool_calls":[
                    {"index":0,"id":"unfinished","type":"function","function":{"name":"weather","arguments":arguments}}
                ]}}]})],
                false,
            );
            let error = try_response(body, true).await.unwrap_err();
            assert!(error.to_string().contains("before completion"));
        }
    }

    #[tokio::test]
    async fn groq_aborted_finish_cannot_execute_valid_arguments_even_after_done() {
        for reason in ["length", "content_filter"] {
            let body = sse(
                vec![json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"id":"aborted","type":"function","function":{"name":"weather","arguments":"{}"}}
            ]},"finish_reason":reason}]})],
                true,
            );
            let error = try_response(body, true).await.unwrap_err();
            assert!(error.to_string().contains("incomplete finish reason"));
        }
    }

    #[tokio::test]
    async fn groq_terminal_frames_survive_split_newline_boundaries() {
        for reason in ["length", "content_filter", "tool_calls"] {
            let mut server = mockito::Server::new_async().await;
            let first = sse(
                vec![json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"id":"split","type":"function","function":{"name":"weather","arguments":"{}"}}
            ]},"finish_reason":reason}]})],
                false,
            );
            let mock = server
                .mock("POST", "/chat/completions")
                .with_header("content-type", "text/event-stream")
                .with_chunked_body(move |writer| {
                    writer.write_all(&first.as_bytes()[..first.len() - 1])?;
                    writer.flush()?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    writer.write_all(b"\ndata: [DONE]\n\n")?;
                    writer.flush()?;
                    Ok(())
                })
                .create_async()
                .await;
            let result = handle_streaming_response(
                &Client::new(),
                format!("{}/chat/completions", server.url()),
                json!({}),
                "test".into(),
                None,
                None,
                Arc::new(LLMStopper::new()),
                "test".into(),
                None,
            )
            .await;
            mock.assert_async().await;
            if reason == "tool_calls" {
                assert_eq!(result.unwrap().function_calls[0].id.as_deref(), Some("split"));
            } else {
                assert!(result.unwrap_err().to_string().contains("incomplete finish reason"));
            }
        }
    }

    #[tokio::test]
    async fn groq_crlf_terminal_frames_survive_split_cr_lf_boundaries() {
        for reason in ["length", "content_filter", "tool_calls"] {
            let mut server = mockito::Server::new_async().await;
            let first = sse(
                vec![json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"id":"split","type":"function","function":{"name":"weather","arguments":"{}"}}
            ]},"finish_reason":reason}]})],
                false,
            )
            .replace("\n", "\r\n");
            let mock = server
                .mock("POST", "/chat/completions")
                .with_header("content-type", "text/event-stream")
                .with_chunked_body(move |writer| {
                    writer.write_all(&first.as_bytes()[..first.len() - 1])?;
                    writer.flush()?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    writer.write_all(b"\ndata: [DONE]\r\n\r\n")?;
                    writer.flush()?;
                    Ok(())
                })
                .create_async()
                .await;
            let result = handle_streaming_response(
                &Client::new(),
                format!("{}/chat/completions", server.url()),
                json!({}),
                "test".into(),
                None,
                None,
                Arc::new(LLMStopper::new()),
                "test".into(),
                None,
            )
            .await;
            mock.assert_async().await;
            if reason == "tool_calls" {
                assert_eq!(result.unwrap().function_calls[0].id.as_deref(), Some("split"));
            } else {
                assert!(result.unwrap_err().to_string().contains("incomplete finish reason"));
            }
        }
    }

    #[tokio::test]
    async fn groq_stop_after_completed_call_discards_calls_and_keeps_text() {
        let mut server = mockito::Server::new_async().await;
        let inbox = InboxName::get_job_inbox_name_from_params("groq-stop".into()).unwrap();
        let stopper = Arc::new(LLMStopper::new());
        let writer_stopper = stopper.clone();
        let writer_inbox = inbox.clone();
        let first = sse(
            vec![
                json!({"choices":[{"index":0,"delta":{"content":"Text", "reasoning":"Plan", "tool_calls":[
            {"index":0,"id":"completed","type":"function","function":{"name":"weather","arguments":"{}"}}
        ]},"finish_reason":"tool_calls"}]}),
            ],
            false,
        );
        let mock = server
            .mock("POST", "/chat/completions")
            .with_header("content-type", "text/event-stream")
            .with_chunked_body(move |writer| {
                writer.write_all(first.as_bytes())?;
                writer.flush()?;
                // Keep the stream open after the complete tool call, then stop it.
                std::thread::sleep(std::time::Duration::from_millis(100));
                writer_stopper.stop(&writer_inbox.to_string());
                writer.write_all(b"data: [DONE]\n\n")?;
                writer.flush()?;
                Ok(())
            })
            .create_async()
            .await;
        let result = handle_streaming_response(
            &Client::new(),
            format!("{}/chat/completions", server.url()),
            json!({}),
            "test".into(),
            Some(inbox.clone()),
            None,
            stopper.clone(),
            "test".into(),
            None,
        )
        .await
        .unwrap();
        mock.assert_async().await;
        assert_eq!(result.response_string, "Text");
        assert_eq!(result.reasoning_content.as_deref(), Some("Plan"));
        assert!(result.function_calls.is_empty());
        assert!(!stopper.should_stop(&inbox.to_string()));
    }
}
