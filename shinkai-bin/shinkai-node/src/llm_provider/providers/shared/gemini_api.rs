use super::shared_model_logic::get_image_type;
use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

pub fn gemini_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_llm_messages(
        Some(max_input_tokens),
        None,
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

    // Get a more accurate estimate of the number of used tokens
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&chat_completion_messages);
    // Calculate the remaining output tokens available
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    // Separate messages into those with a user / assistant / system role and those without
    let (mut messages_with_role, _tools): (Vec<_>, Vec<_>) = chat_completion_messages
        .into_iter()
        .partition(|message| message.role.is_some());

    // Update the role from "assistant" to "model"
    for message in &mut messages_with_role {
        if let Some(role) = &message.role {
            if role == "assistant" {
                message.role = Some("model".to_string());
            }
        }
    }

    // Convert both sets of messages to serde Value
    let messages_json = serde_json::to_value(messages_with_role)?;

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
                            if let Some(image_type) = get_image_type(&image_str) {
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

    // Separate system instruction from other messages
    let system_instruction = messages_vec
        .clone()
        .iter()
        .find(|msg| msg.get("role") == Some(&serde_json::Value::String("system".to_string())))
        .cloned();

    let contents: Vec<_> = messages_vec
        .iter()
        .filter(|msg| msg.get("role") != Some(&serde_json::Value::String("system".to_string())))
        .cloned()
        .collect();

    let default_content = "".to_string();
    let result_json = serde_json::json!({
        "system_instruction": {
            "parts": { "text": system_instruction.map_or(default_content.clone(), |msg| {
                msg.get("content")
                    .and_then(|v| {
                        if let serde_json::Value::Array(arr) = v {
                            arr.iter().find_map(|item| {
                                if let serde_json::Value::Object(obj) = item {
                                    obj.get("text").and_then(|text| text.as_str().map(|s| s.to_string()))
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        }
                    })
                    .unwrap_or(default_content.clone())
            })}
        },
        "contents": contents.into_iter().map(|msg| {
            let role = msg.get("role").cloned().unwrap_or(serde_json::Value::String("".to_string()));
            let content = msg.get("content").cloned().unwrap_or(serde_json::Value::String("".to_string()));
            let text_content = if let serde_json::Value::Array(content_array) = content {
                content_array.into_iter().find_map(|item| {
                    if let serde_json::Value::Object(mut obj) = item {
                        if let Some(serde_json::Value::String(text)) = obj.remove("text") {
                            return Some(text);
                        }
                    }
                    None
                }).unwrap_or_default()
            } else {
                "".to_string()
            };
            serde_json::json!({
                "role": role,
                "parts": [{ "text": text_content }]
            })
        }).collect::<Vec<_>>()
    });

    Ok(PromptResult {
        messages: PromptResultEnum::Value(result_json),
        functions: Some(vec![]),
        remaining_tokens: remaining_output_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};

    #[test]
    fn test_gemini_from_llm_messages() {
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
                vec![],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the gemini_prepare_messages function
        let result = gemini_prepare_messages(&model, prompt).expect("Failed to prepare messages");
        eprintln!("result: {:?}", result);

        // Define the expected messages and functions
        let expected_messages = json!({
            "system_instruction": {
                "parts": { "text": "You are Neko the cat respond like one" }
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "Hello" }]
                },
                {
                    "role": "model",
                    "parts": [{ "text": "Great to meet you. What would you like to know?" }]
                },
                {
                    "role": "user",
                    "parts": [{ "text": "I have two dogs in my house. How many paws are in my house?" }]
                }
            ]
        });

        // Assert the results
        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_tokens > 0);
    }
}