use crate::agent::job_prompts::Prompt;

use super::AgentError;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::Value as JsonValue;
use serde_json::json;
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

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Serialize)]
struct ApiPayload {
    model: String,
    messages: String, // Maybe it'd be better to have Vec<Message> here?
    temperature: f64,
    max_tokens: usize,
}

#[async_trait]
impl LLMProvider for OpenAI {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/v1/chat/completions");
                let messages_json_string = prompt.generate_openai_messages()?;

                let payload = json!({
                    "model": self.model_type,
                    "messages": serde_json::from_str::<JsonValue>(&messages_json_string)?,
                    "temperature": 0,
                    "max_tokens": 1024
                });

                let body = serde_json::to_string(&payload)?;


                eprintln!("body api chagpt: {}", body);

                let res = client
                    .post(url)
                    .bearer_auth(key)
                    .header("Content-Type", "application/json")
                    .body(body)
                    .send()
                    .await?;

                eprintln!("Status: {}", res.status());
                eprintln!("Response: {:?}", res.text().await?);
                // let data: Response = res.json().await.map_err(AgentError::ReqwestError)?;
                // let response_string: String = data
                //     .choices
                //     .iter()
                //     .map(|choice| choice.message.content.clone())
                //     .collect::<Vec<String>>()
                //     .join(" ");
                // Self::extract_first_json_object(&response_string)
                Self::extract_first_json_object("")
            } else {
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
