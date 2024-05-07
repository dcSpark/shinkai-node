use crate::{
    agent::{
        error::AgentError,
        execution::prompts::prompts::{Prompt, SubPrompt},
    },
    managers::model_capabilities_manager::{Base64ImageString, ModelCapabilitiesManager, PromptResult, PromptResultEnum},
};
use serde::{Deserialize, Serialize};

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
