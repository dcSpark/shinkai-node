use reqwest::Client;
use crate::schemas::message_schemas::JobPreMessage;

use super::agent::AgentError;
use async_trait::async_trait;

#[async_trait]
pub trait Provider {
    type Response;
    fn parse_response(response_body: &str) -> Result<Self::Response, Box<dyn std::error::Error>>;
    fn extract_content(response: &Self::Response) -> Vec<JobPreMessage>;
    async fn call_api(&self, client: &Client, url: Option<&String>, api_key: Option<&String>, content: &str, context: Vec<String>) -> Result<Vec<JobPreMessage>, AgentError>;
}

pub mod openai;
pub mod sleep_api;

