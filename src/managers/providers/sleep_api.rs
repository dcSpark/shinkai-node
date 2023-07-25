use crate::managers::agent::AgentError;

use super::Provider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SleepAPI {}

#[async_trait]
impl Provider for SleepAPI {
    type Response = (); // Empty tuple as a stand-in for no data

    fn parse_response(_: &str) -> Result<Self::Response, Box<dyn std::error::Error>> {
        Ok(())
    }

    fn extract_content(_: &Self::Response) -> String {
        "OK".to_string()
    }

    async fn call_api(&self, _: &Client, _: Option<&String>, _: Option<&String>, _: &str) -> Result<String, AgentError> {
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(Self::extract_content(&()))
    }
}
