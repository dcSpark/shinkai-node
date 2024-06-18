use serde_json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::{
    schemas::llm_providers::serialized_llm_provider::LLMProviderInterface,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};

use crate::{
    llm_provider::{error::LLMProviderError, execution::prompts::{prompts::Prompt, subprompts::SubPrompt}},
    managers::model_capabilities_manager::{
        Base64ImageString, ModelCapabilitiesManager, PromptResult, PromptResultEnum,
    },
};

pub fn llama_prepare_messages(
    _model: &LLMProviderInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, LLMProviderError> {
    let messages_string = prompt.generate_genericapi_messages(Some(total_tokens))?;

    let used_tokens = ModelCapabilitiesManager::count_tokens_from_message_llama3(&messages_string);

    Ok(PromptResult {
        value: PromptResultEnum::Text(messages_string.clone()),
        remaining_tokens: total_tokens - used_tokens,
    })
}

pub fn llava_prepare_messages(
    _model: &LLMProviderInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, LLMProviderError> {
    let messages_string = prompt.generate_genericapi_messages(Some(total_tokens))?;

    if let Some((_, _, asset_content, _, _)) = prompt.sub_prompts.iter().rev().find_map(|sub_prompt| {
        if let SubPrompt::Asset(prompt_type, asset_type, asset_content, asset_detail, priority) = sub_prompt {
            Some((prompt_type, asset_type, asset_content, asset_detail, priority))
        } else {
            None
        }
    }) {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            format!("Messages JSON (image analysis): {:?}", messages_string).as_str(),
        );

        Ok(PromptResult {
            value: PromptResultEnum::ImageAnalysis(messages_string.clone(), Base64ImageString(asset_content.clone())),
            remaining_tokens: total_tokens - messages_string.len(),
        })
    } else {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Error,
            format!("Image content not found: {:?}", messages_string).as_str(),
        );
        Err(LLMProviderError::ImageContentNotFound(
            "Image content not found".to_string(),
        ))
    }
}
