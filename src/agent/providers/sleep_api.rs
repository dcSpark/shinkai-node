use super::AgentError;
use super::LLMProvider;
use crate::agent::job_prompts::Prompt;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SleepAPI;
use tokio::time::Duration;

#[async_trait]
impl LLMProvider for SleepAPI {
    async fn call_api(
        &self,
        _: &Client,
        _: Option<&String>,
        _: Option<&String>,
        _: Prompt,
    ) -> Result<JsonValue, AgentError> {
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(JsonValue::Bool(true))
    }
}
