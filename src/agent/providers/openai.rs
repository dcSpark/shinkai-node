use crate::agent::job_prompts::Prompt;

use super::AgentError;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;
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
    message: OpenAIApiMessage,
    finish_reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenAIApiMessage {
    pub role: String,
    pub content: String,
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

                let messages = prompt.generate_openai_messages(None)?;
                let messages_json = serde_json::to_value(&messages)?;

                let payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "temperature": 0.7,
                    "max_tokens": 3000, // TODO: need to set up this correctly depending on the length of messages
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
                let response_text = res.text().await?;
                eprintln!("Response: {:?}", response_text);

                let data_resp: Result<JsonValue, _> = serde_json::from_str(&response_text);
                eprintln!("data_resp: {:?}", data_resp);

                // let data_resp = res.json::<serde_json::Value>().await;
                match data_resp {
                    Ok(value) => {
                        let data: Response = serde_json::from_value(value).map_err(AgentError::SerdeError)?;
                        let response_string: String = data
                            .choices
                            .iter()
                            .map(|choice| choice.message.content.clone())
                            .collect::<Vec<String>>()
                            .join(" ");
                        Self::extract_first_json_object(&response_string)
                    }
                    Err(e) => Err(AgentError::SerdeError(e)),
                }
            } else {
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
