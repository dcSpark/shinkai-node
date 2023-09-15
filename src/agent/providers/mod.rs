use reqwest::Client;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobPreMessage;

use super::error::AgentError;
use async_trait::async_trait;

#[async_trait]
pub trait Provider {
    type Response;
    fn parse_response(response_body: &str) -> Result<Self::Response, Box<dyn std::error::Error>>;
    fn extract_content(response: &Self::Response) -> Vec<JobPreMessage>;
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        content: &str,
        step_history: Vec<String>,
    ) -> Result<Vec<JobPreMessage>, AgentError>;
}

pub mod openai;
pub mod sleep_api;
