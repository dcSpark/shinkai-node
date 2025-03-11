use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde_json::{self, Value as JsonValue};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::subprompts::SubPrompt;

use super::openai_api::openai_prepare_messages;

pub fn sambanova_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    let mut prompt_copy = prompt.clone();
    
    // Collect the last omni assets
    let last_omni_assets = prompt_copy
        .sub_prompts
        .iter_mut()
        .rev()
        .filter_map(|sub_prompt| {
            if let SubPrompt::Omni(_, _, assets, _) = sub_prompt {
                Some(assets.clone())
            } else {
                None
            }
        })
        .next();

    if let Some(last_omni) = last_omni_assets {
        for sub_prompt in prompt_copy.sub_prompts.iter_mut() {
            if let SubPrompt::Omni(_, _, assets, _) = sub_prompt {
                if *assets != last_omni {
                    assets.clear();
                }
            }
        }
    }

    let result = openai_prepare_messages(model, prompt_copy)?;

    // SambaNova uses OpenAI-compatible format, so we can use the result directly
    // We don't need to modify the messages format like Groq does
    
    Ok(PromptResult {
        messages: result.messages,
        functions: result.functions,
        remaining_output_tokens: result.remaining_output_tokens,
        tokens_used: result.tokens_used,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptAssetType, SubPromptType};

    #[test]
    fn test_sambanova_with_system_message() {
        let sub_prompts = vec![
            SubPrompt::Omni(
                SubPromptType::System,
                "You are a helpful assistant".to_string(),
                vec![],
                98,
            ),
            SubPrompt::Omni(SubPromptType::User, "Hello".to_string(), vec![], 97),
            SubPrompt::Omni(
                SubPromptType::Assistant,
                "How can I help you today?".to_string(),
                vec![],
                97,
            ),
            SubPrompt::Omni(
                SubPromptType::User,
                "Tell me about SambaNova".to_string(),
                vec![],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the sambanova_prepare_messages function
        let result = sambanova_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Define the expected messages and functions
        let expected_messages = json!([
            {
                "role": "system",
                "content": "You are a helpful assistant"
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Hello"
                    }
                ]
            },
            {
                "role": "assistant",
                "content": "How can I help you today?"
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Tell me about SambaNova"
                    }
                ]
            }
        ]);

        // Assert the results
        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);
    }

    #[test]
    fn test_sambanova_with_image() {
        let sub_prompts = vec![
            SubPrompt::Omni(SubPromptType::User, "Hello".to_string(), vec![], 97),
            SubPrompt::Omni(
                SubPromptType::User,
                "What's in this image?".to_string(),
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

        // Call the sambanova_prepare_messages function
        let result = sambanova_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Define the expected messages and functions
        let expected_messages = json!([
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Hello"
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "What's in this image?"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAMAAABrrFhUAAAABGdBTUEAALGPC/xhBQAAAAFzUkdCAK7OHOkAAAD5UExURQAAAACl7QCl7ACm7ACl7ACl7ACm7QCm7QCm7QCm7ACm7ACm7QCl7QCl6wCl7ACl7ACl7ACl7QCl6wCl7QCl7ACm7ACm7ACl7QCm7ACl7QCl6wCm7QCm7QCl7ACl7QCm7QCl7QCl7QCm7ACm7QCl6wCl7ACl7QCl7ACm7ACm7ACl7QCl7ACl7QCm7QCm7ACm7ACl7ACl7QCl6wCm7QCm6wCm7QCm7QCm7QCl7QCl7ACm7QCl7ACm7QCl7QCl7ACk6wCl7QCl7ACm7ACl7QCm7ACl7QCl7ACm7QCl7ACm7ACm7ACm7QCl7ACl7ACm7QCl7QCk7ACm7ACm7ahktTwAAABSdFJOUwDoJJubI+v7+vaN9fswD50JzFrLCCbo+esOCcvUWZvM6S9Z+YzP0cyc0VrriQ6MCCX0JIoK7J5Z9p6ZDi8PiCPr6NMl1CTRJSbn+p2cWiSgD4gNsVXUAAACIElEQVR42u3X11KVMRiG0Wx2+femN+kgiL1Ls4MKKhZQc/8X4xmnye+BM3yznjt4VyaTSUqSJEmSJElX/Z7/ObfS5Gtes3Kz+/D9P8y/MTPKYXr167zl/LXxYQ7V8OlBm/1jGzlcZ3v1+98t5YB1pqrPv5NDtj1Wef83ctC+LFYBjOewPa56/4ZxAXYvKwBe58BtlfdPjyIDTKwWAeZz6DaLAPdjA/SKAPdiAywUAQ5jAwyKAE1sgMkiQA4eAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA8D8Amtj7J4sAg9gAj4oAc7EB1osAvdgA3SLARWyAW0WA6VHk/ROrRYA0ExngR3l/ej6Mu3/5tAIg3Y4L8Kdmf5r9FHX/g2dVAKn/Meb+pX6qbOdbxP2dqVRd/068/XdPUosWj5djzf++P5va9fnNizjzR0dfU/uevOyuf7j2v+NmsNDbfJskSZIkSZKu+gtLvn0aIyUzCwAAAABJRU5ErkJggg=="
                        }
                    }
                ]
            }
        ]);

        // Assert the results
        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);
    }
}
