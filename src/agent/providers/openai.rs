use super::AgentError;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::OpenAI;

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

#[async_trait]
impl LLMProvider for OpenAI {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        content: &str,
    ) -> Result<JsonValue, AgentError> {
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

                eprintln!("Status: {}", res.status());
                let data: Response = res.json().await.map_err(AgentError::ReqwestError)?;
                let response_string: String = data
                    .choices
                    .iter()
                    .map(|choice| choice.message.content.clone())
                    .collect::<Vec<String>>()
                    .join(" ");
                Self::extract_first_json_object(&response_string)
            } else {
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
