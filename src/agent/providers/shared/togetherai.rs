use crate::{
    agent::{
        error::AgentError,
        execution::job_prompts::{Prompt, SubPrompt},
    },
    managers::model_capabilities_manager::{Base64ImageString, PromptResult, PromptResultEnum},
};
use serde::{Deserialize, Serialize};
use serde_json;
use shinkai_message_primitives::{
    schemas::agents::serialized_agent::AgentLLMInterface,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};

#[derive(Serialize, Deserialize)]
pub struct TogetherAPIResponse {
    pub status: String,
    pub prompt: Vec<String>,
    pub model: String,
    pub model_owner: String,
    pub num_returns: i32,
    pub args: Args,
    pub subjobs: Vec<String>,
    pub output: Output,
}

#[derive(Serialize, Deserialize)]
pub struct Args {
    pub model: String,
    pub prompt: String,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub max_tokens: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Output {
    pub choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize)]
pub struct Choice {
    pub finish_reason: Option<String>,
    pub index: Option<i32>,
    pub text: String,
}

pub fn llama_prepare_messages(
    model: &AgentLLMInterface,
    model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, AgentError> {
    let mut messages_string = prompt.generate_genericapi_messages(None)?;
    if !messages_string.ends_with(" ```") {
        messages_string.push_str(" ```json");
    }

    shinkai_log(
        ShinkaiLogOption::JobExecution,
        ShinkaiLogLevel::Info,
        format!("Messages JSON (generic): {:?}", messages_string).as_str(),
    );

    Ok(PromptResult {
        value: PromptResultEnum::Text(messages_string.clone()),
        remaining_tokens: total_tokens - messages_string.len(),
    })
}

pub fn llava_prepare_messages(
    model: &AgentLLMInterface,
    model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, AgentError> {
    let mut messages_string = prompt.generate_genericapi_messages(None)?;
    if !messages_string.ends_with(" ```") {
        messages_string.push_str(" ```json");
    }

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
        Err(AgentError::ImageContentNotFound("Image content not found".to_string()))
    }
}
