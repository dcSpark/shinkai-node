use super::AgentError;
use super::LLMProvider;
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
        _: &str,
    ) -> Result<JsonValue, AgentError> {
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(JsonValue::Bool(true))
    }
}
