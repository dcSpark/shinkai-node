use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_message::LlmMessage;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

pub fn claude_prepare_messages(
    model: &LLMProviderInterface,
    prompt: Prompt,
) -> Result<(PromptResult, Vec<LlmMessage>), LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    let chat_completion_messages = prompt.generate_llm_messages(
        Some(max_input_tokens),
        Some("function".to_string()),
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

    process_llm_messages(chat_completion_messages, model)
}

pub fn process_llm_messages(
    chat_completion_messages: Vec<LlmMessage>,
    model: &LLMProviderInterface,
) -> Result<(PromptResult, Vec<LlmMessage>), LLMProviderError> {
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&chat_completion_messages);
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    let (mut messages_with_role, tools): (Vec<_>, Vec<_>) = chat_completion_messages
        .into_iter()
        .partition(|message| message.role.is_some());

    let mut system_messages = Vec::new();

    for message in &mut messages_with_role {
        if message.role == Some("system".to_string()) {
            system_messages.push(message.clone());
        }
    }

    messages_with_role.retain(|message| {
        ((message.role == Some("user".to_string())
            || message.role == Some("function".to_string())
            || message.role == Some("assistant".to_string()))
            && message.content.is_some()
            && message.content.as_ref().unwrap().len() > 0)
            || (message.role == Some("assistant".to_string()) && message.function_call.is_some())
    });

    let messages_json = serde_json::to_value(messages_with_role)?;
    let tools_json = serde_json::to_value(tools)?;

    let messages_vec = match messages_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|mut message| {
                if message.get("role") == Some(&serde_json::Value::String("user".to_string())) {
                    if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
                        message["content"] = serde_json::Value::String(content.to_string());
                    }
                    message.as_object_mut().unwrap().remove("images");
                }

                if message.get("role") == Some(&serde_json::Value::String("assistant".to_string())) {
                    if let Some(function_call) = message.get("function_call").cloned() {
                        if let Some(arguments) = function_call.get("arguments").and_then(|v| v.as_str()) {
                            let input: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();

                            message["content"] = serde_json::json!([
                                {
                                    "type": "tool_use",
                                    "id": "toolu_abc123",
                                    "name": function_call.get("name").cloned().unwrap_or_default(),
                                    "input": input
                                }
                            ]);
                        }
                        message.as_object_mut().unwrap().remove("function_call");
                    } else if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
                        message["content"] = serde_json::Value::String(content.to_string());
                        message.as_object_mut().unwrap().remove("images");
                    }
                }

                if message.get("role") == Some(&serde_json::Value::String("function".to_string())) {
                    if let Some(content) = message.get("content").cloned() {
                        message["content"] = serde_json::json!([{
                            "type": "tool_result",
                            "tool_use_id": "toolu_abc123",
                            "content": content
                        }]);
                    }
                    message["role"] = serde_json::Value::String("user".to_string());
                    message.as_object_mut().unwrap().remove("name");
                }

                message
            })
            .collect(),
        _ => vec![],
    };

    let tools_vec = match tools_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .flat_map(|tool| {
                if let serde_json::Value::Object(mut map) = tool {
                    map.remove("functions")
                        .and_then(|functions| {
                            if let serde_json::Value::Array(funcs) = functions {
                                Some(funcs)
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

    Ok((
        PromptResult {
            messages: PromptResultEnum::Value(serde_json::Value::Array(messages_vec)),
            functions: Some(tools_vec),
            remaining_output_tokens,
            tokens_used: used_tokens,
        },
        system_messages,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_message_primitives::schemas::{
        llm_message::DetailedFunctionCall, llm_providers::serialized_llm_provider::Claude,
    };

    #[test]
    fn test_claude_prepare_messages() {
        let claude_model = Claude {
            model_type: "claude-3-5-sonnet-20241022".to_string(),
        };

        let model = LLMProviderInterface::Claude(claude_model);

        let llm_messages = vec![
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("You are a very helpful assistant. You may be provided with documents or content to analyze and answer questions about them, in that case refer to the content provided in the user message for your responses.".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("tell me what's the response when using shinkai echo tool with: say hello".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: Some(vec![]),
            },
            LlmMessage {
                role: Some("assistant".to_string()),
                content: None,
                name: None,
                function_call: Some(DetailedFunctionCall {
                    name: "shinkai__echo".to_string(),
                    arguments: "{\"message\":\"hello\"}".to_string(),
                }),
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("function".to_string()),
                content: Some("{\"data\":{\"message\":\"echoing: hello\"}}".to_string()),
                name: Some("shinkai__echo".to_string()),
                function_call: None,
                functions: None,
                images: None,
            },
        ];

        let expected_json = json!([
          {
            "role": "user",
            "content": "tell me what's the response when using shinkai echo tool with: say hello"
          },
          {
            "role": "assistant",
            "content": [
          {
            "type": "tool_use",
            "id": "toolu_abc123",
            "name": "shinkai__echo",
            "input": {
              "message": "hello"
            }
          }
            ]
          },
          {
            "role": "user",
            "content": [
              {
                "type": "tool_result",
                "tool_use_id": "toolu_abc123",
                "content": "{\"data\":{\"message\":\"echoing: hello\"}}"
              }
            ]
          }
        ]);

        let (messages_result, _system_messages) = process_llm_messages(llm_messages, &model).unwrap();
        let messages_json = match messages_result.messages {
            PromptResultEnum::Value(v) => v,
            _ => {
                panic!("Expected Value variant in PromptResultEnum");
            }
        };
        eprintln!("\n\nresult: {:?}", messages_json);
        eprintln!("\n\nexpected: {:?}", expected_json);
        assert_eq!(messages_json, expected_json);
    }
}
