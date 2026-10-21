use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

use super::shared_model_logic::{self, sanitize_tool_name};

#[derive(Debug, Deserialize)]
pub struct OpenAIResponse {
    #[serde(default)]
    id: Option<String>,
    object: String,
    created: u64,
    pub choices: Vec<Choice>,
    usage: Usage,
    system_fingerprint: Option<String>,
    #[serde(rename = "x_groq", default)]
    groq: Option<GroqInfo>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: i32,
    pub message: OpenAIApiMessage,
    #[serde(rename = "finish_reason")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GroqInfo {
    id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIApiMessage {
    pub role: String,
    pub content: Option<MessageContent>,
    pub function_call: Option<FunctionCall>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCallResponse {
    pub response: String,
    pub function_call: FunctionCall,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    FunctionCall(FunctionCallResponse),
    Text(String),
    ImageUrl { url: String },
}

impl Serialize for OpenAIApiMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_struct("OpenAIApiMessage", 4)?;
        map.serialize_field("role", &self.role)?;
        if let Some(content) = &self.content {
            map.serialize_field("content", content)?;
        }
        if let Some(function_call) = &self.function_call {
            map.serialize_field("function_call", function_call)?;
        }
        if let Some(tool_calls) = &self.tool_calls {
            map.serialize_field("tool_calls", tool_calls)?;
        }
        map.end()
    }
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
    #[serde(default)]
    queue_time: Option<f64>,
    #[serde(default)]
    prompt_time: Option<f64>,
    #[serde(default)]
    completion_time: Option<f64>,
    #[serde(default)]
    total_time: Option<f64>,
}

pub fn openai_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_llm_messages(
        Some(max_input_tokens),
        Some("function".to_string()),
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

    // Get a more accurate estimate of the number of used tokens
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&chat_completion_messages);
    // Calculate the remaining output tokens available
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    // Separate messages into those with a valid role and those without
    let (messages_with_role, tools): (Vec<_>, Vec<_>) = chat_completion_messages
        .into_iter()
        .partition(|message| message.role.is_some());

    // Convert both sets of messages to serde Value
    let messages_json = serde_json::to_value(messages_with_role)?;

    // Convert tools to serde Value with name transformation
    let tools_json = serde_json::to_value(
        tools
            .clone()
            .into_iter()
            .map(|mut tool| {
                if let Some(functions) = tool.functions.as_mut() {
                    for function in functions {
                        function.name = sanitize_tool_name(&function.name);
                    }
                }
                tool
            })
            .collect::<Vec<_>>(),
    )?;

    // Convert messages_json and tools_json to Vec<serde_json::Value>
    let mut replayed_openrouter_turns = std::collections::HashSet::new();
    let messages_vec = match messages_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .filter_map(|mut message| {
                let images = message.get("images").cloned();
                let text = message.get("content").cloned();

                if let Some(serde_json::Value::Array(images_array)) = images {
                    let mut content = vec![];
                    if let Some(text) = text {
                        content.push(serde_json::json!({"type": "text", "text": text}));
                    }
                    for image in images_array {
                        if let serde_json::Value::String(image_str) = image {
                            if let Some(image_type) = shared_model_logic::get_image_type(&image_str) {
                                content.push(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {"url": format!("data:image/{};base64,{}", image_type, image_str)}
                                }));
                            }
                        }
                    }
                    message["content"] = serde_json::json!(content);
                    message.as_object_mut().unwrap().remove("images");
                }

                // Convert function role to tool format
                if message.get("role") == Some(&serde_json::Value::String("function".to_string())) {
                    // Get the function name to use as tool_call_id
                    let tool_call_id = message
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    // Create new message in tool format
                    let mut new_message = serde_json::Map::new();
                    new_message.insert("role".to_string(), serde_json::Value::String("tool".to_string()));
                    new_message.insert("tool_call_id".to_string(), serde_json::Value::String(tool_call_id));

                    // Copy content to the new message
                    if let Some(content) = message.get("content") {
                        new_message.insert("content".to_string(), content.clone());
                    }

                    return Some(serde_json::Value::Object(new_message));
                }

                // Convert function_call to tool_calls format if needed
                if message.get("role") == Some(&serde_json::Value::String("assistant".to_string()))
                    && message.get("function_call").is_some()
                    && message.get("tool_calls").is_none()
                {
                    if let Some(function_call) = message.get("function_call").cloned() {
                        // Extract the id
                        let call_id = function_call.get("id").and_then(|n| n.as_str()).unwrap_or("unknown");

                        let metadata = function_call.get("provider_metadata");
                        if matches!(model, LLMProviderInterface::OpenRouter(_)) {
                            if let Some(native) = metadata
                                .and_then(|m| m.get("openrouter_assistant"))
                                .filter(|m| m["role"] == "assistant" && m["tool_calls"].is_array())
                            {
                                // The stored Prompt alternates call/result pairs. Restore the complete
                                // native assistant turn only at its first call, before all tool results.
                                // Provider IDs and call payloads can repeat in later inference turns.
                                // Keep tagged identities separate from the legacy payload fallback.
                                let identity = match metadata
                                    .and_then(|metadata| metadata["inference_turn_id"].as_str())
                                    .filter(|turn| !turn.is_empty())
                                {
                                    Some(turn) => (true, turn.to_owned()),
                                    None => (false, serde_json::to_string(&native["tool_calls"]).unwrap_or_default()),
                                };
                                if replayed_openrouter_turns.insert(identity) {
                                    return Some(native.clone());
                                }
                                return None;
                            }
                        }

                        let mut tool_call = serde_json::json!({
                            "id": call_id,
                            "type": "function",
                            "function": {
                                "name": function_call["name"],
                                "arguments": function_call["arguments"]
                            }
                        });
                        let uses_responses = match model {
                            LLMProviderInterface::OpenAI(_) => true,
                            LLMProviderInterface::OpenAILegacy(provider) => {
                                ModelCapabilitiesManager::is_gpt6(&provider.model_type)
                            }
                            _ => false,
                        };
                        if uses_responses {
                            if let Some(output) = metadata.and_then(|m| m.get("openai_responses")) {
                                tool_call["provider_metadata"] = serde_json::json!({"openai_responses": output});
                                // The Responses transform consumes this local tag before building API input.
                                if let Some(turn) = metadata.and_then(|metadata| metadata.get("inference_turn_id")) {
                                    tool_call["provider_metadata"]["inference_turn_id"] = turn.clone();
                                }
                            }
                        }
                        restore_assistant_reasoning(&mut message, metadata, model);
                        let tool_calls = serde_json::json!([tool_call]);

                        // Add tool_calls to the message and set content to null
                        if let Some(obj) = message.as_object_mut() {
                            obj.insert("tool_calls".to_string(), tool_calls);
                            obj.insert("content".to_string(), serde_json::Value::Null);
                            obj.remove("function_call");
                        }
                    }
                }

                Some(message)
            })
            .collect(),
        _ => vec![],
    };

    // Only adjacent fragments can belong to the same assistant tool turn.
    let merged_messages = merge_adjacent_tool_calls(messages_vec);
    let merged_messages = if matches!(model, LLMProviderInterface::OpenRouter(_)) {
        complete_openrouter_tool_results(merged_messages)
    } else {
        merged_messages
    };

    // Flatten the tools array to extract functions directly
    let tools_vec = match tools_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .flat_map(|tool| {
                if let serde_json::Value::Object(mut map) = tool {
                    // TODO: functions is deprecated in favor of tools. Update it
                    map.remove("functions")
                        .and_then(|functions| {
                            if let serde_json::Value::Array(funcs) = functions {
                                Some(
                                    funcs
                                        .into_iter()
                                        .map(|func| {
                                            serde_json::json!({
                                                "type": "function",
                                                "function": func
                                            })
                                        })
                                        .collect::<Vec<_>>(),
                                )
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                } else {
                    vec![]
                }
            })
            .collect(),
        _ => vec![],
    };

    Ok(PromptResult {
        messages: PromptResultEnum::Value(serde_json::Value::Array(merged_messages)),
        functions: Some(tools_vec),
        remaining_output_tokens,
        tokens_used: used_tokens,
    })
}

fn restore_assistant_reasoning(
    message: &mut serde_json::Value,
    metadata: Option<&serde_json::Value>,
    model: &LLMProviderInterface,
) {
    let Some(metadata) = metadata else { return };
    match model {
        LLMProviderInterface::TogetherAI(_) => {
            if let Some(content) = metadata.get("together_reasoning_content").and_then(|v| v.as_str()) {
                message["reasoning_content"] = serde_json::json!(content);
            }
        }
        LLMProviderInterface::DeepSeek(_) => {
            if let Some(content) = metadata.get("deepseek_reasoning_content").and_then(|v| v.as_str()) {
                message["reasoning_content"] = serde_json::json!(content);
            }
        }
        LLMProviderInterface::OpenRouter(_) => {
            if let Some(details) = metadata.get("reasoning_details").filter(|v| v.is_array()) {
                message["reasoning_details"] = details.clone();
            }
        }
        _ => (),
    }
}

// Native assistant turns can include calls skipped after an earlier validation error.
// Keep the signed assistant data and all recorded results unchanged. Supply a result
// for each missing call before the next conversational turn.
fn complete_openrouter_tool_results(messages: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut completed = Vec::new();
    let mut pending = Vec::<String>::new();
    fn append_missing(completed: &mut Vec<serde_json::Value>, pending: &mut Vec<String>) {
        for id in pending.drain(..) {
            completed.push(serde_json::json!({
                "role": "tool", "tool_call_id": id,
                "content": "Tool execution did not produce a result. The call was skipped after an earlier tool error."
            }));
        }
    }
    for message in messages {
        if message["role"] == "tool" {
            if let Some(id) = message["tool_call_id"].as_str() {
                pending.retain(|pending_id| pending_id != id);
            }
        } else {
            append_missing(&mut completed, &mut pending);
            if message["role"] == "assistant" {
                if let Some(calls) = message["tool_calls"].as_array() {
                    for call in calls {
                        if let Some(id) = call["id"].as_str().filter(|id| !id.is_empty()) {
                            if !pending.iter().any(|pending_id| pending_id == id) {
                                pending.push(id.into());
                            }
                        }
                    }
                }
            }
        }
        completed.push(message);
    }
    append_missing(&mut completed, &mut pending);
    completed
}

fn merge_adjacent_tool_calls(messages: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut merged: Vec<serde_json::Value> = Vec::with_capacity(messages.len());
    for message in messages {
        let is_tool_fragment = |message: &serde_json::Value| {
            message["role"] == "assistant"
                && message["tool_calls"].is_array()
                && message.get("content").map_or(true, |content| content.is_null())
        };
        if is_tool_fragment(&message) {
            if let Some(previous) = merged.last_mut().filter(|previous| {
                is_tool_fragment(previous)
                    && previous.get("reasoning_content") == message.get("reasoning_content")
                    && previous.get("reasoning_details").is_none()
                    && message.get("reasoning_details").is_none()
            }) {
                previous["tool_calls"]
                    .as_array_mut()
                    .unwrap()
                    .extend(message["tool_calls"].as_array().unwrap().iter().cloned());
                continue;
            }
        }
        merged.push(message);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptAssetType, SubPromptType};

    #[test]
    fn responses_metadata_only_reaches_actual_responses_routes() {
        use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{OpenAI, OpenAILegacy};
        let output = json!({"output": [{"type":"reasoning","encrypted_content":"signed"}]});
        let metadata =
            json!({"openai_responses": output, "inference_turn_id": "turn-first", "private": "must stay local"});
        for (model, expected) in [
            (
                LLMProviderInterface::OpenAI(OpenAI {
                    model_type: "gpt-4o".into(),
                }),
                true,
            ),
            (
                LLMProviderInterface::OpenAILegacy(OpenAILegacy {
                    model_type: "gpt-6.1-sol".into(),
                }),
                true,
            ),
            (
                LLMProviderInterface::OpenAILegacy(OpenAILegacy {
                    model_type: "gpt-4o".into(),
                }),
                false,
            ),
            (
                LLMProviderInterface::OpenAILegacy(OpenAILegacy {
                    model_type: "gpt-6-custom".into(),
                }),
                false,
            ),
        ] {
            let mut prompt = Prompt::new();
            prompt.add_function_call(
                json!({"name":"weather", "call_id":"call-1", "arguments":{}, "provider_metadata":metadata}),
                100,
            );
            let prompt = serde_json::from_str(&serde_json::to_string(&prompt).unwrap()).unwrap();
            let result = openai_prepare_messages(&model, prompt).unwrap();
            let PromptResultEnum::Value(messages) = result.messages else {
                panic!("expected messages")
            };
            assert_eq!(
                messages[0]["tool_calls"][0].get("provider_metadata").is_some(),
                expected
            );
            if expected {
                assert_eq!(
                    messages[0]["tool_calls"][0]["provider_metadata"],
                    json!({"openai_responses": output, "inference_turn_id": "turn-first"})
                );
            }
            assert!(messages[0].get("inference_turn_id").is_none());
            assert!(messages[0]["tool_calls"][0].get("inference_turn_id").is_none());
            assert!(messages[0]["tool_calls"][0]["function"]
                .get("inference_turn_id")
                .is_none());
            assert!(messages[0]["tool_calls"][0]["function"]
                .get("provider_metadata")
                .is_none());
        }
    }

    #[test]
    fn openrouter_native_turns_keep_signatures_and_complete_missing_results() {
        use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::OpenRouter;
        let native = json!({"role":"assistant", "content":null,
        "reasoning_details":[{"type":"reasoning.encrypted", "data":"signed"}],
        "tool_calls":[
            {"id":"A", "type":"function", "function":{"name":"first", "arguments":"{}"}},
            {"id":"B", "type":"function", "function":{"name":"second", "arguments":"{}"}}
        ]});
        let model = LLMProviderInterface::OpenRouter(OpenRouter {
            model_type: "anthropic/claude-opus-5.5".into(),
        });
        for complete in [false, true] {
            let mut prompt = Prompt::new();
            for (name, id) in [("first", "A"), ("second", "B")]
                .into_iter()
                .take(if complete { 2 } else { 1 })
            {
                prompt.add_function_call(json!({"name":name,"call_id":id,"arguments":{},"provider_metadata":{"openrouter_assistant":native}}),100);
                prompt.add_function_call_response(
                    json!({"function_call":{"name":name,"id":id},"response":format!("valid {id}")}),
                    100,
                );
            }
            let prepared = openai_prepare_messages(&model, prompt).unwrap();
            let PromptResultEnum::Value(messages) = prepared.messages else {
                panic!("expected messages")
            };
            assert_eq!(messages[0], native);
            assert_eq!(
                messages[1],
                json!({"role":"tool","tool_call_id":"A","content":"valid A"})
            );
            assert_eq!(messages[2]["tool_call_id"], "B");
            if complete {
                assert_eq!(messages[2]["content"], "valid B");
            } else {
                assert!(messages[2]["content"].as_str().unwrap().contains("skipped"));
            }
            assert_eq!(messages.as_array().unwrap().len(), 3);
        }
    }

    fn prepare_saved_openrouter_history(prompt: Prompt) -> serde_json::Value {
        use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::OpenRouter;
        let saved = serde_json::to_string(&prompt).unwrap();
        let prompt = serde_json::from_str(&saved).unwrap();
        let model = LLMProviderInterface::OpenRouter(OpenRouter {
            model_type: "anthropic/claude-opus-5.5".into(),
        });
        let prepared = openai_prepare_messages(&model, prompt).unwrap();
        let PromptResultEnum::Value(messages) = prepared.messages else {
            panic!("Expected messages");
        };
        messages
    }

    #[test]
    fn tagged_openrouter_turns_keep_identical_calls_and_each_signed_assistant_turn() {
        let native = json!({"role": "assistant", "content": null,
            "reasoning_details": [{"type": "reasoning.encrypted", "data": "signed-first", "index": 0}],
            "tool_calls": [
                {"id": "A", "type": "function", "function": {"name": "first", "arguments": "{}"}},
                {"id": "B", "type": "function", "function": {"name": "second", "arguments": "{}"}}
            ]
        });
        for same_reasoning in [false, true] {
            for grouped in [false, true] {
                let mut prompt = Prompt::new();
                let mut expected_turns = Vec::new();
                for turn in ["first-turn", "second-turn"] {
                    let mut assistant = native.clone();
                    if turn == "second-turn" && !same_reasoning {
                        assistant["reasoning_details"][0]["data"] = json!("signed-second");
                    }
                    expected_turns.push(assistant.clone());
                    let mut responses = Vec::new();
                    for (name, id) in [("first", "A"), ("second", "B")] {
                        prompt.add_function_call(
                            json!({"name": name, "call_id": id, "arguments": {},
                                "provider_metadata": {"inference_turn_id": turn, "openrouter_assistant": assistant}
                            }),
                            100,
                        );
                        let response = json!({"function_call": {"name": name, "id": id}, "response": format!("{turn}: actual {id}")});
                        if grouped {
                            responses.push(response);
                        } else {
                            prompt.add_function_call_response(response, 100);
                        }
                    }
                    for response in responses {
                        prompt.add_function_call_response(response, 100);
                    }
                }
                let messages = prepare_saved_openrouter_history(prompt);
                assert_eq!(messages.as_array().unwrap().len(), 6);
                for (offset, turn, expected) in [
                    (0, "first-turn", &expected_turns[0]),
                    (3, "second-turn", &expected_turns[1]),
                ] {
                    // Each turn replays once, even when its complete signed payload is identical.
                    assert_eq!(&messages[offset], expected);
                    assert_eq!(
                        messages[offset + 1],
                        json!({"role": "tool", "tool_call_id": "A", "content": format!("{turn}: actual A")})
                    );
                    assert_eq!(
                        messages[offset + 2],
                        json!({"role": "tool", "tool_call_id": "B", "content": format!("{turn}: actual B")})
                    );
                }
            }
        }
    }

    #[test]
    fn tagged_turn_identity_cannot_collide_with_legacy_payload_identity() {
        let native = json!({"role": "assistant", "content": null,
            "reasoning_details": [{"type": "reasoning.encrypted", "data": "signed", "index": 0}],
            "tool_calls": [{"id": "A", "type": "function", "function": {"name": "first", "arguments": "{}"}}]
        });
        let payload_identity = serde_json::to_string(&native["tool_calls"]).unwrap();
        let mut prompt = Prompt::new();
        for (index, tagged) in [false, true].into_iter().enumerate() {
            let mut metadata = json!({"openrouter_assistant": native});
            if tagged {
                metadata["inference_turn_id"] = json!(payload_identity);
            }
            prompt.add_function_call(
                json!({"name": "first", "call_id": "A", "arguments": {}, "provider_metadata": metadata}),
                100,
            );
            prompt.add_function_call_response(
                json!({"function_call": {"name": "first", "id": "A"}, "response": format!("actual result {index}")}),
                100,
            );
        }
        let messages = prepare_saved_openrouter_history(prompt);
        assert_eq!(messages.as_array().unwrap().len(), 4);
        assert_eq!(messages[0], native);
        assert_eq!(messages[2], native);
        assert_eq!(messages[1]["content"], "actual result 0");
        assert_eq!(messages[3]["content"], "actual result 1");
    }

    #[test]
    fn missing_openrouter_results_are_inserted_before_the_next_turn() {
        let first = json!({"role":"assistant", "reasoning_details":[{"signature":"signed-first"}],
            "tool_calls":[{"id":"A"},{"id":"B"}]});
        let later = json!({"role":"assistant", "reasoning_details":[{"signature":"signed-later"}],
            "tool_calls":[{"id":"C"}]});
        let valid_a = json!({"role":"tool","tool_call_id":"A","content":"validation failed"});
        let valid_c = json!({"role":"tool","tool_call_id":"C","content":"success"});
        let completed =
            complete_openrouter_tool_results(vec![first.clone(), valid_a.clone(), later.clone(), valid_c.clone()]);
        assert_eq!(completed.len(), 5);
        assert_eq!(completed[0], first);
        assert_eq!(completed[1], valid_a);
        assert_eq!(completed[2]["tool_call_id"], "B");
        assert_eq!(completed[3], later);
        assert_eq!(completed[4], valid_c);
    }

    #[test]
    fn tool_history_restores_provider_reasoning_without_function_metadata() {
        use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
            DeepSeek, OpenAI, OpenRouter, TogetherAI,
        };
        let details = json!([{"type": "reasoning.encrypted", "data": "signed-data", "index": 0}]);
        let metadata = json!({
            "deepseek_reasoning_content": "Earlier reasoning",
            "together_reasoning_content": "Together reasoning",
            "reasoning_details": details,
            "openai_responses": {"output": [{"type": "reasoning", "id": "rs_1"}]},
            "internal": "must not reach the function schema"
        });
        let providers = [
            LLMProviderInterface::TogetherAI(TogetherAI {
                model_type: "moonshotai/Kimi-K3".into(),
            }),
            LLMProviderInterface::DeepSeek(DeepSeek {
                model_type: "deepseek-flash".to_string(),
            }),
            LLMProviderInterface::OpenRouter(OpenRouter {
                model_type: "anthropic/claude-opus-5.5".to_string(),
            }),
            LLMProviderInterface::OpenAI(OpenAI {
                model_type: "gpt-6.1-sol".to_string(),
            }),
        ];
        for model in providers {
            let mut prompt = Prompt::new();
            prompt.add_sub_prompts(vec![SubPrompt::FunctionCall(
                SubPromptType::Assistant,
                json!({"call_id": "call_1", "name": "weather", "arguments": {}, "provider_metadata": metadata}),
                100,
            )]);
            let prepared = openai_prepare_messages(&model, prompt).unwrap();
            let PromptResultEnum::Value(messages) = prepared.messages else {
                panic!("Expected messages")
            };
            let assistant = &messages[0];
            assert_eq!(assistant["tool_calls"][0]["id"], "call_1");
            assert_eq!(
                assistant["tool_calls"][0]["function"],
                json!({"name": "weather", "arguments": "{}"})
            );
            match model {
                LLMProviderInterface::TogetherAI(_) => {
                    assert_eq!(assistant["reasoning_content"], "Together reasoning");
                    assert!(assistant.get("reasoning_details").is_none());
                    assert!(assistant["tool_calls"][0].get("provider_metadata").is_none());
                }
                LLMProviderInterface::DeepSeek(_) => {
                    assert_eq!(assistant["reasoning_content"], "Earlier reasoning");
                    assert!(assistant.get("reasoning_details").is_none());
                }
                LLMProviderInterface::OpenRouter(_) => {
                    assert_eq!(assistant["reasoning_details"], details);
                    assert!(assistant.get("reasoning_content").is_none());
                }
                _ => {
                    assert_eq!(
                        assistant["tool_calls"][0]["provider_metadata"],
                        json!({"openai_responses": metadata["openai_responses"]})
                    );
                    assert!(assistant.get("reasoning_content").is_none());
                    assert!(assistant.get("reasoning_details").is_none());
                }
            }
        }
    }

    #[test]
    fn tool_call_groups_preserve_distinct_reasoning() {
        let messages = json!([
            {"role": "assistant", "tool_calls": [{"id": "a"}], "reasoning_details": [{"data": "first"}]},
            {"role": "assistant", "tool_calls": [{"id": "b"}], "reasoning_details": [{"data": "second"}]}
        ]);
        assert_eq!(
            merge_adjacent_tool_calls(messages.as_array().unwrap().clone()),
            messages.as_array().unwrap().clone()
        );
    }

    #[test]
    fn tool_call_groups_keep_separate_turns_and_text() {
        let messages = json!([
            {"role": "assistant", "content": null, "tool_calls": [{"id": "a"}]},
            {"role": "assistant", "content": null, "tool_calls": [{"id": "b"}]},
            {"role": "tool", "tool_call_id": "a", "content": "first"},
            {"role": "tool", "tool_call_id": "b", "content": "second"},
            {"role": "user", "content": "Next request"},
            {"role": "assistant", "content": null, "tool_calls": [{"id": "c"}]},
            {"role": "tool", "tool_call_id": "c", "content": "third"},
            {"role": "assistant", "content": "Keep this text", "tool_calls": [{"id": "d"}]},
            {"role": "assistant", "content": null, "tool_calls": [{"id": "e"}]}
        ]);
        let original = messages.as_array().unwrap();
        let merged = merge_adjacent_tool_calls(original.clone());
        assert_eq!(merged.len(), original.len() - 1);
        assert_eq!(merged[0]["tool_calls"], json!([{"id": "a"}, {"id": "b"}]));
        assert_eq!(&merged[1..], &original[2..]);
    }

    #[test]
    fn test_openai_from_llm_messages() {
        let sub_prompts = vec![
            SubPrompt::Omni(
                SubPromptType::System,
                "You are Neko the cat respond like one".to_string(),
                vec![],
                98,
            ),
            SubPrompt::Omni(SubPromptType::User, "Hello".to_string(), vec![], 97),
            SubPrompt::Omni(
                SubPromptType::Assistant,
                "Great to meet you. What would you like to know?".to_string(),
                vec![],
                97,
            ),
            SubPrompt::Omni(
                SubPromptType::User,
                "I have two dogs in my house. How many paws are in my house?".to_string(),
                vec![(
                    SubPromptAssetType::Image,
                    "iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAMAAABrrFhUAAAABGdBTUEAALGPC/xhBQAAAAFzUkdCAK7OHOkAAAD5UExURQAAAACl7QCl7ACm7ACl7ACl7ACm7QCm7QCm7QCm7ACm7ACm7QCl7QCl6wCl7ACl7ACl7ACl7QCl6wCl7QCl7ACm7ACm7ACl7QCm7ACl7QCl6wCm7QCm7QCl7ACl7QCm7QCl7QCl7QCm7ACm7QCl6wCl7ACl7QCl7ACm7ACm7ACl7QCl7ACl7QCm7QCm7ACm7ACl7ACl7QCl6wCm7QCm6wCm7QCm7QCm7QCl7QCl7ACm7QCl7ACm7QCl7QCl7ACk6wCl7QCl7ACm7ACl7QCm7ACl7QCl7ACm7QCl7ACm7ACm7ACm7QCl7ACl7ACm7QCl7QCk7ACm7ACm7ahktTwAAABSdFJOUwDoJJubI+v7+vaN9fswD50JzFrLCCbo+esOCcvUWZvM6S9Z+YzP0cyc0VrriQ6MCCX0JIoK7J5Z9p6ZDi8PiCPr6NMl1CTRJSbn+p2cWiSgD4gNsVXUAAACIElEQVR42u3X11KVMRiG0Wx2+femN+kgiL1Ls4MKKhZQc/8X4xmnye+BM3yznjt4VyaTSUqSJEmSJElX/Z7/ObfS5Gtes3Kz+/D9P8y/MTPKYXr167zl/LXxYQ7V8OlBm/1jGzlcZ3v1+98t5YB1pqrPv5NDtj1Wef83ctC+LFYBjOewPa56/4ZxAXYvKwBe58BtlfdPjyIDTKwWAeZz6DaLAPdjA/SKAPdiAywUAQ5jAwyKAE1sgMkiQA4eAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA8D8Amtj7J4sAg9gAj4oAc7EB1osAvdgA3SLARWyAW0WA6VHk/ROrRYA0ExngR3l/ej6Mu3/5tAIg3Y4L8Kdmf5r9FHX/g2dVAKn/Meb+pX6qbOdbxP2dqVRd/068/XdPUosWj5djzf++P5va9fnNizjzR0dfU/uevOyuf7j2v+NmsNDbfJskSZIkSZKu+gtLvn0aIyUzCwAAAABJRU5ErkJggg==".to_string(),
                    "image.png".to_string(),
                )],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the openai_prepare_messages function
        let result = openai_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Define the expected messages and functions
        let expected_messages = json!([
            {
                "role": "system",
                "content": "You are Neko the cat respond like one"
            },
            {
                "role": "user",
                "content": "Hello"
            },
            {
                "role": "assistant",
                "content": "Great to meet you. What would you like to know?"
            },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": "I have two dogs in my house. How many paws are in my house?" },
                    {
                        "type": "image_url",
                        "image_url": { "url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAMAAABrrFhUAAAABGdBTUEAALGPC/xhBQAAAAFzUkdCAK7OHOkAAAD5UExURQAAAACl7QCl7ACm7ACl7ACl7ACm7QCm7QCm7QCm7ACm7ACm7QCl7QCl6wCl7ACl7ACl7ACl7QCl6wCl7QCl7ACm7ACm7ACl7QCm7ACl7QCl6wCm7QCm7QCl7ACl7QCm7QCl7QCl7QCm7ACm7QCl6wCl7ACl7QCl7ACm7ACm7ACl7QCl7ACl7QCm7QCm7ACm7ACl7ACl7QCl6wCm7QCm6wCm7QCm7QCm7QCl7QCl7ACm7QCl7ACm7QCl7QCl7ACk6wCl7QCl7ACm7ACl7QCm7ACl7QCl7ACm7QCl7ACm7ACm7ACm7QCl7ACl7ACm7QCl7QCk7ACm7ACm7ahktTwAAABSdFJOUwDoJJubI+v7+vaN9fswD50JzFrLCCbo+esOCcvUWZvM6S9Z+YzP0cyc0VrriQ6MCCX0JIoK7J5Z9p6ZDi8PiCPr6NMl1CTRJSbn+p2cWiSgD4gNsVXUAAACIElEQVR42u3X11KVMRiG0Wx2+femN+kgiL1Ls4MKKhZQc/8X4xmnye+BM3yznjt4VyaTSUqSJEmSJElX/Z7/ObfS5Gtes3Kz+/D9P8y/MTPKYXr167zl/LXxYQ7V8OlBm/1jGzlcZ3v1+98t5YB1pqrPv5NDtj1Wef83ctC+LFYBjOewPa56/4ZxAXYvKwBe58BtlfdPjyIDTKwWAeZz6DaLAPdjA/SKAPdiAywUAQ5jAwyKAE1sgMkiQA4eAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA8D8Amtj7J4sAg9gAj4oAc7EB1osAvdgA3SLARWyAW0WA6VHk/ROrRYA0ExngR3l/ej6Mu3/5tAIg3Y4L8Kdmf5r9FHX/g2dVAKn/Meb+pX6qbOdbxP2dqVRd/068/XdPUosWj5djzf++P5va9fnNizjzR0dfU/uevOyuf7j2v+NmsNDbfJskSZIkSZKu+gtLvn0aIyUzCwAAAABJRU5ErkJggg==" }
                    }
                ]
            }
        ]);

        // Assert the results
        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);
    }

    #[test]
    fn test_openai_api_message_with_function_call() {
        let json_str = json!({
            "role": "assistant",
            "content": null,
            "function_call":{
                "name": "concat_strings",
                "arguments":  json!({
                    "first_string": "hola",
                    "second_string": " chao"
                }).to_string()
            }
        })
        .to_string();

        // Deserialize the JSON string to OpenAIApiMessage
        let message: OpenAIApiMessage = serde_json::from_str(&json_str).expect("Failed to deserialize");

        // Check the deserialized values
        assert_eq!(message.role, "assistant");
        assert!(message.content.is_none());
        assert!(message.function_call.is_some());

        if let Some(function_call) = message.function_call.clone() {
            assert_eq!(function_call.name, "concat_strings");
            assert_eq!(
                function_call.arguments,
                json!({"first_string": "hola", "second_string": " chao"}).to_string()
            );
        }

        // Serialize the OpenAIApiMessage back to JSON
        let serialized_json = serde_json::to_string(&message).expect("Failed to serialize");

        // Deserialize again to check round-trip consistency
        let deserialized_message: OpenAIApiMessage =
            serde_json::from_str(&serialized_json).expect("Failed to deserialize");

        // Check the deserialized values again
        assert_eq!(deserialized_message.role, "assistant");
        assert!(deserialized_message.content.is_none());
        assert!(deserialized_message.function_call.is_some());

        if let Some(function_call) = deserialized_message.function_call {
            assert_eq!(function_call.name, "concat_strings");
            assert_eq!(
                function_call.arguments,
                json!({"first_string": "hola", "second_string": " chao"}).to_string()
            );
        }
    }

    #[test]
    fn test_openai_response_after_tool_usage_parsing() {
        let response_text = r#"
        {
            "id": "chatcmpl-9cQYyc4ENYwJ5ChU4WHtRv7uPRHbN",
            "object": "chat.completion",
            "created": 1718945600,
            "model": "gpt-4-1106-preview",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "The concatenated result of \"hola\" and \"chao\" is \"hola chao\"."
                    },
                    "logprobs": null,
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 156,
                "completion_tokens": 21,
                "total_tokens": 177
            },
            "system_fingerprint": null
        }
        "#;

        // Deserialize the JSON string to OpenAIResponse
        let response: OpenAIResponse = serde_json::from_str(response_text).expect("Failed to deserialize");

        // Check the deserialized values
        assert_eq!(response.id.clone().unwrap(), "chatcmpl-9cQYyc4ENYwJ5ChU4WHtRv7uPRHbN");
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.created, 1718945600);
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.usage.prompt_tokens, 156);
        assert_eq!(response.usage.completion_tokens, 21);
        assert_eq!(response.usage.total_tokens, 177);

        let choice = &response.choices[0];
        assert_eq!(choice.index, 0);
        assert_eq!(choice.message.role, "assistant");
        if let Some(MessageContent::Text(content)) = &choice.message.content {
            assert_eq!(
                content,
                "The concatenated result of \"hola\" and \"chao\" is \"hola chao\"."
            );
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_groq_response_with_tool_calls() {
        let response_text = r#"{
            "id": "chatcmpl-0cae310a-2b36-470a-9261-0f24d77b01bc",
            "object": "chat.completion",
            "created": 1736736692,
            "model": "llama-3.2-11b-vision-preview",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "tool_calls": [
                            {
                                "id": "call_sa3n",
                                "type": "function",
                                "function": {
                                    "name": "duckduckgo_search",
                                    "arguments": "{\"message\": \"best movie 2024\"}"
                                }
                            }
                        ]
                    },
                    "logprobs": null,
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": {
                "queue_time": 0.018144843999999993,
                "prompt_tokens": 1185,
                "prompt_time": 0.077966956,
                "completion_tokens": 21,
                "completion_time": 0.028,
                "total_tokens": 1206,
                "total_time": 0.105966956
            },
            "system_fingerprint": "fp_9cb648b966",
            "x_groq": {
                "id": "req_01jhes5nvkedsb8hcw0x912fa6"
            }
        }"#;

        let response: OpenAIResponse = serde_json::from_str(response_text).expect("Failed to deserialize");

        // Verify basic response fields
        assert_eq!(
            response.id.clone().unwrap(),
            "chatcmpl-0cae310a-2b36-470a-9261-0f24d77b01bc"
        );
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.created, 1736736692);
        assert_eq!(response.system_fingerprint, Some("fp_9cb648b966".to_string()));

        // Verify choices
        assert_eq!(response.choices.len(), 1);
        let choice = &response.choices[0];
        assert_eq!(choice.index, 0);
        assert_eq!(choice.finish_reason, Some("tool_calls".to_string()));

        // Verify tool calls
        let message = &choice.message;
        assert_eq!(message.role, "assistant");
        assert!(message.content.is_none());

        let tool_calls = message.tool_calls.as_ref().expect("Should have tool_calls");
        assert_eq!(tool_calls.len(), 1);

        let tool_call = &tool_calls[0];
        assert_eq!(tool_call.id, "call_sa3n");
        assert_eq!(tool_call.call_type, "function");
        assert_eq!(tool_call.function.name, "duckduckgo_search");
        assert_eq!(tool_call.function.arguments, "{\"message\": \"best movie 2024\"}");

        // Verify usage
        assert_eq!(response.usage.prompt_tokens, 1185);
        assert_eq!(response.usage.completion_tokens, 21);
        assert_eq!(response.usage.total_tokens, 1206);
        assert!(response.usage.queue_time.is_some());
        assert!(response.usage.prompt_time.is_some());
        assert!(response.usage.completion_time.is_some());
        assert!(response.usage.total_time.is_some());

        // Verify Groq info
        let groq = response.groq.expect("Should have Groq info");
        assert_eq!(groq.id, "req_01jhes5nvkedsb8hcw0x912fa6");
    }

    #[test]
    fn deepseek_reasoning_keeps_system_and_user_messages() {
        use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::DeepSeek;
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![
            SubPrompt::Content(SubPromptType::System, "Keep this system instruction".to_string(), 98),
            SubPrompt::Content(SubPromptType::User, "First user message".to_string(), 100),
            SubPrompt::Omni(
                SubPromptType::UserLastMessage,
                "Last user message".to_string(),
                vec![],
                100,
            ),
        ]);
        let model = LLMProviderInterface::DeepSeek(DeepSeek {
            model_type: "deepseek-flash".to_string(),
        });
        let result = crate::llm_provider::providers::shared::deepseek_api::deepseek_prepare_messages(
            &model,
            prompt,
            uuid::Uuid::new_v4().to_string(),
        )
        .expect("Failed to prepare messages");
        assert_eq!(
            result.messages,
            PromptResultEnum::Value(json!([
                {"role": "system", "content": "Keep this system instruction"},
                {"role": "user", "content": "First user message"},
                {"role": "user", "content": "Last user message"}
            ]))
        );
    }

    #[test]
    fn test_openai_tools_format() {
        let sub_prompts = vec![
            SubPrompt::Content(
                SubPromptType::AvailableTool,
                serde_json::json!({
                    "name": "get_weather",
                    "description": "Get the current weather in a given location",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The city and state, e.g., San Francisco, CA"
                            },
                            "unit": {
                                "type": "string",
                                "description": "The temperature unit to use",
                                "enum": ["celsius", "fahrenheit"]
                            }
                        },
                        "required": ["location"]
                    }
                })
                .to_string(),
                98,
            ),
            SubPrompt::Content(SubPromptType::User, "What's the weather like?".to_string(), 100),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the openai_prepare_messages function
        let result = openai_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Extract messages to verify the tool content is included as a message
        let messages = match &result.messages {
            PromptResultEnum::Value(value) => value.as_array().unwrap(),
            _ => panic!("Expected Value variant"),
        };

        // Check that we have two messages (tool and user)
        assert_eq!(messages.len(), 2);

        // First message should be the tool definition
        let tool_message = &messages[0];
        assert_eq!(tool_message["role"], "tool");

        // Second message should be the user question
        let user_message = &messages[1];
        assert_eq!(user_message["role"], "user");
        assert_eq!(user_message["content"], "What's the weather like?");

        // Add a new test to verify the parsing of the new OpenAI response format with tool_calls
        let response_text = r#"{
            "id": "chatcmpl-BLFYlP20FFM59in39QKADGQc2i48k",
            "object": "chat.completion",
            "created": 1744404399,
            "model": "gpt-4o-2024-08-06",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [
                            {
                                "id": "call_8Q4Ojyqmex2ifIn0Mno6o39e",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"location\":\"San Francisco, CA\"}"
                                }
                            }
                        ],
                        "refusal": null,
                        "annotations": []
                    },
                    "logprobs": null,
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": {
                "prompt_tokens": 92,
                "completion_tokens": 18,
                "total_tokens": 110,
                "prompt_tokens_details": {
                    "cached_tokens": 0,
                    "audio_tokens": 0
                },
                "completion_tokens_details": {
                    "reasoning_tokens": 0,
                    "audio_tokens": 0,
                    "accepted_prediction_tokens": 0,
                    "rejected_prediction_tokens": 0
                }
            },
            "service_tier": "default",
            "system_fingerprint": "fp_22890b9c0a"
        }"#;

        // Deserialize the JSON string to OpenAIResponse
        let response: OpenAIResponse = serde_json::from_str(response_text).expect("Failed to deserialize");

        // Verify basic response fields
        assert_eq!(response.id.clone().unwrap(), "chatcmpl-BLFYlP20FFM59in39QKADGQc2i48k");
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.created, 1744404399);
        assert_eq!(response.system_fingerprint, Some("fp_22890b9c0a".to_string()));

        // Verify choices
        assert_eq!(response.choices.len(), 1);
        let choice = &response.choices[0];
        assert_eq!(choice.index, 0);
        assert_eq!(choice.finish_reason, Some("tool_calls".to_string()));

        // Verify tool calls
        let message = &choice.message;
        assert_eq!(message.role, "assistant");
        assert!(message.content.is_none());

        let tool_calls = message.tool_calls.as_ref().expect("Should have tool_calls");
        assert_eq!(tool_calls.len(), 1);

        let tool_call = &tool_calls[0];
        assert_eq!(tool_call.id, "call_8Q4Ojyqmex2ifIn0Mno6o39e");
        assert_eq!(tool_call.call_type, "function");
        assert_eq!(tool_call.function.name, "get_weather");
        assert_eq!(tool_call.function.arguments, "{\"location\":\"San Francisco, CA\"}");
    }

    #[test]
    fn test_function_to_tool_response_conversion() {
        // Create a prompt with a function call response
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![
            SubPrompt::Content(SubPromptType::System, "You are a helpful assistant".to_string(), 100),
            SubPrompt::Content(SubPromptType::User, "Search for: steam deck".to_string(), 90),
            SubPrompt::FunctionCall(
                SubPromptType::Assistant,
                serde_json::json!({
                    "name": "youtube_search_api",
                    "arguments": "{\"searchQuery\":\"steam deck\"}"
                }),
                80,
            ),
            SubPrompt::FunctionCallResponse(
                SubPromptType::Function,
                serde_json::json!({
                    "function_call": {
                        "name": "youtube_search_api"
                    },
                    "response": "best video about steam deck is https://youtube.com/123123"
                }),
                70,
            ),
        ]);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the openai_prepare_messages function
        let result = openai_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Extract the messages
        let messages = match &result.messages {
            PromptResultEnum::Value(value) => value.as_array().unwrap(),
            _ => panic!("Expected Value variant"),
        };

        // Find the function response message (now converted to tool format)
        let tool_message = messages
            .iter()
            .find(|msg| msg.get("role").and_then(|r| r.as_str()) == Some("tool"))
            .expect("Tool message not found");

        // Verify it has the correct format
        assert_eq!(tool_message.get("role").and_then(|r| r.as_str()), Some("tool"));
        assert_eq!(
            tool_message.get("tool_call_id").and_then(|id| id.as_str()),
            Some("youtube_search_api")
        );
        assert_eq!(
            tool_message.get("content").and_then(|c| c.as_str()),
            Some("best video about steam deck is https://youtube.com/123123")
        );
    }

    #[test]
    fn test_merge_assistant_tool_calls() {
        // Create a prompt with multiple consecutive FunctionCall sub-prompts
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![
            SubPrompt::Content(SubPromptType::System, "You are a helpful assistant".to_string(), 100),
            SubPrompt::Content(
                SubPromptType::User,
                "Search for videos about steam deck".to_string(),
                90,
            ),
            SubPrompt::FunctionCall(
                SubPromptType::Assistant,
                serde_json::json!({
                    "call_id": "call_ZkgL3ICnH7wfpurcoipeKgJe",
                    "name": "youtube_transcript_fetcher",
                    "arguments": "{\"url\":\"https://www.youtube.com/watch?v=CQU7i0Gsffw\"}"
                }),
                80,
            ),
            SubPrompt::FunctionCall(
                SubPromptType::Assistant,
                serde_json::json!({
                    "call_id": "call_b9PeqXFq81UATmjmvw0CAp8v",
                    "name": "youtube_transcript_fetcher",
                    "arguments": "{\"url\":\"https://www.youtube.com/watch?v=96DWMkjR1jo\"}"
                }),
                75,
            ),
            SubPrompt::FunctionCall(
                SubPromptType::Assistant,
                serde_json::json!({
                    "call_id": "call_BJRhdBUuvay1dprkZQiTa3lp",
                    "name": "youtube_transcript_fetcher",
                    "arguments": "{\"url\":\"https://www.youtube.com/watch?v=xRo7XUjqEcA\"}"
                }),
                70,
            ),
        ]);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the openai_prepare_messages function
        let result = openai_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Extract the messages
        let messages = match &result.messages {
            PromptResultEnum::Value(value) => value.as_array().unwrap(),
            _ => panic!("Expected Value variant"),
        };

        // Find all assistant messages
        let assistant_messages: Vec<_> = messages
            .iter()
            .filter(|msg| msg.get("role").and_then(|r| r.as_str()) == Some("assistant"))
            .collect();

        // Verify there's only one assistant message (they were merged)
        assert_eq!(
            assistant_messages.len(),
            1,
            "Expected only 1 assistant message after merging"
        );

        // Verify the assistant message has 3 tool calls
        let tool_calls = assistant_messages[0]
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .unwrap();
        assert_eq!(
            tool_calls.len(),
            3,
            "Expected 3 tool calls in the merged assistant message"
        );

        // Verify the tool calls have the correct function names and arguments
        let function_names: Vec<String> = tool_calls
            .iter()
            .filter_map(|tc| {
                tc.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from)
            })
            .collect();

        assert_eq!(function_names.len(), 3, "Expected 3 function names");
        assert!(
            function_names.iter().all(|name| name == "youtube_transcript_fetcher"),
            "All function names should be 'youtube_transcript_fetcher'"
        );

        // Extract the URLs from the arguments - more safely
        let urls: Vec<String> = tool_calls
            .iter()
            .filter_map(|tc| {
                tc.get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .and_then(|args_str| {
                        // Handle double-encoded JSON: first remove outer quotes if present
                        let cleaned_str = if args_str.starts_with("\"") && args_str.ends_with("\"") {
                            // Remove outer quotes and unescape inner content
                            let inner = &args_str[1..args_str.len() - 1];
                            // Replace escaped quotes and backslashes
                            inner.replace("\\\"", "\"").replace("\\\\", "\\")
                        } else {
                            args_str.to_string()
                        };

                        // Now parse the properly formatted JSON
                        serde_json::from_str::<serde_json::Value>(&cleaned_str).ok()
                    })
                    .and_then(|args_json| {
                        // Extract the URL value
                        args_json.get("url").and_then(|url| url.as_str()).map(String::from)
                    })
            })
            .collect();

        assert_eq!(urls.len(), 3, "Expected 3 URLs");
        assert!(
            urls.contains(&"https://www.youtube.com/watch?v=CQU7i0Gsffw".to_string()),
            "URLs should contain the first video URL"
        );
        assert!(
            urls.contains(&"https://www.youtube.com/watch?v=96DWMkjR1jo".to_string()),
            "URLs should contain the second video URL"
        );
        assert!(
            urls.contains(&"https://www.youtube.com/watch?v=xRo7XUjqEcA".to_string()),
            "URLs should contain the third video URL"
        );
    }
}
