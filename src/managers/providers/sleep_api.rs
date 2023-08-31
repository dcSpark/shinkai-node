use crate::{managers::agent::AgentError};

use super::Provider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shinkai_message_wasm::{shinkai_message::shinkai_message_schemas::{JobPreMessage, JobRecipient}, schemas::agents::serialized_agent::SleepAPI};
use tokio::time::Duration;

#[async_trait]
impl Provider for SleepAPI {
    type Response = (); // Empty tuple as a stand-in for no data

    fn parse_response(_: &str) -> Result<Self::Response, Box<dyn std::error::Error>> {
        Ok(())
    }

    fn extract_content(_: &Self::Response) -> Vec<JobPreMessage> {
        vec![
            JobPreMessage {
                tool_calls: Vec::new(), // TODO: You might want to replace this with actual values
                content: "OK".to_string(),
                recipient: JobRecipient::SelfNode, // TODO: This is a placeholder. You should replace this with the actual recipient.
            }
        ]
    }

    async fn call_api(&self, _: &Client, _: Option<&String>, _: Option<&String>, _: &str, _: Vec<String>) -> Result<Vec<JobPreMessage>, AgentError> {
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(Self::extract_content(&()))
    }
}
