use std::sync::Arc;

use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;

use super::{
    error::LLMProviderError, execution::chains::inference_chain_trait::LLMInferenceResponse, llm_stopper::LLMStopper
};
use async_trait::async_trait;
use reqwest::Client;
use shinkai_message_primitives::schemas::{
    inbox_name::InboxName, job_config::JobConfig, llm_providers::serialized_llm_provider::LLMProviderInterface, prompts::Prompt
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;

pub mod claude;
pub mod deepseek;
pub mod exo;
pub mod gemini;
pub mod groq;
pub mod llm_cancellable_request;
pub mod local_regex;
pub mod ollama;
pub mod openai;
pub mod openai_tests;
pub mod openrouter;
pub mod shared;
pub mod shinkai_backend;
pub mod togetherai;

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
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
        db: Arc<SqliteManager>,
        tracing_message_id: Option<String>,
    ) -> Result<LLMInferenceResponse, LLMProviderError>;
}
