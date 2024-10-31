use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_message::LlmMessage;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

use super::shared_model_logic;

pub fn claude_prepare_messages(
    model: &LLMProviderInterface,
    prompt: Prompt,
) -> Result<(PromptResult, Vec<LlmMessage>), LLMProviderError> {
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
    let (mut messages_with_role, tools): (Vec<_>, Vec<_>) = chat_completion_messages
        .into_iter()
        .partition(|message| message.role.is_some());

    let mut system_messages = Vec::new();

    // Collect system messages
    for message in &mut messages_with_role {
        if message.role == Some("system".to_string()) {
            system_messages.push(message.clone());
        }
    }

    // Filter out empty content and keep only user and assistant messages
    messages_with_role.retain(|message| {
        message.content.is_some()
            && message.content.as_ref().unwrap().len() > 0
            && (message.role == Some("user".to_string()) || message.role == Some("assistant".to_string()))
    });

    // Convert both sets of messages to serde Value
    let messages_json = serde_json::to_value(messages_with_role)?;
    let tools_json = serde_json::to_value(tools)?;

    // Convert messages_json and tools_json to Vec<serde_json::Value>
    let messages_vec = match messages_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|mut message| {
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
                message
            })
            .collect(),
        _ => vec![],
    };

    // Flatten the tools array to extract functions directly
    // TODO: this is to support the old functions format. We need to update it to tools
    let tools_vec = match tools_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .flat_map(|tool| {
                if let serde_json::Value::Object(mut map) = tool {
                    // TODO: functions is deprecated in favor of tools. Update it
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
