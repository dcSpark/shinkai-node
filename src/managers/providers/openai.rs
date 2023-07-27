use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use std::error::Error;
use async_trait::async_trait;

use crate::{managers::agent::AgentError, schemas::message_schemas::{JobPreMessage, JobRecipient}};

use super::Provider;

#[derive(Debug, Deserialize)]
pub struct Response {
    id: String,
    object: String,
    created: u64,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    index: i32,
    message: Message,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OpenAI {
    pub model_type: String,
}

#[async_trait]
impl Provider for OpenAI {
    type Response = Response;

    fn parse_response(response_body: &str) -> Result<Self::Response, Box<dyn Error>> {
        let res: Result<Response, serde_json::Error> = serde_json::from_str(response_body);
        match res {
            Ok(response) => Ok(response),
            Err(e) => Err(Box::new(e)),
        }
    }

    fn extract_content(response: &Self::Response) -> Vec<JobPreMessage> {
        response.choices.iter().map(|choice| {
            JobPreMessage {
                tool_calls: Vec::new(), // TODO: You might want to replace this with actual values
                content: choice.message.content.clone(),
                recipient: JobRecipient::SelfNode, // TODO: This is a placeholder. You should replace this with the actual recipient.
            }
        }).collect()
    }
    
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        content: &str,
        step_history: Vec<String>,
    ) -> Result<Vec<JobPreMessage>, AgentError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/v1/chat/completions");
                let body = format!(
                    r#"{{
                            "model": "{}",
                            "messages": [
                                {{"role": "system", "content": "You are a helpful assistant."}},
                                {{"role": "user", "content": "{}"}}
                            ],
                            "temperature": 0,
                            "max_tokens": 1024
                        }}"#,
                    self.model_type, content
                );

                let res = client
                    .post(url)
                    .bearer_auth(key)
                    .header("Content-Type", "application/json")
                    .body(body)
                    .send()
                    .await?;

                println!("Status: {}", res.status());
                let data: Response = res.json().await.map_err(AgentError::ReqwestError)?;
                Ok(Self::extract_content(&data))
            } else {
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
