use std::sync::Arc;

use crate::network::ws_manager::WSUpdateHandler;

use super::{
    error::LLMProviderError,
    execution::{chains::inference_chain_trait::LLMInferenceResponse, prompts::prompts::Prompt},
};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::{inbox_name::InboxName, llm_providers::serialized_llm_provider::LLMProviderInterface};
use tokio::sync::Mutex;

pub mod genericapi;
pub mod groq;
pub mod ollama;
pub mod openai;
pub mod shared;
pub mod shinkai_backend;
pub mod gemini;
pub mod exo;

#[async_trait]
pub trait LLMService {
    // type Response;
    // fn parse_response(response_body: &str) -> Result<Self::Response, LLMProviderError>;
    // fn extract_content(response: &Self::Response) -> Result<JsonValue, LLMProviderError>;
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<LLMInferenceResponse, LLMProviderError>;
}
