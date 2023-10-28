use super::super::{error::AgentError, execution::job_prompts::Prompt};
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::OpenAI;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tiktoken_rs::get_chat_completion_max_tokens;
use tiktoken_rs::num_tokens_from_messages;

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

                let tiktoken_messages = prompt.generate_openai_messages(None)?;
                let used_tokens = num_tokens_from_messages("gpt-4", &tiktoken_messages).unwrap();

                let mut messages: Vec<OpenAIApiMessage> = tiktoken_messages
                    .into_iter()
                    .filter_map(|message| {
                        if let Some(content) = message.content {
                            Some(OpenAIApiMessage {
                                role: message.role,
                                content,
                            })
                        } else {
                            eprintln!(
                                "Warning: Message with role '{}' has no content. Ignoring.",
                                message.role
                            );
                            None
                        }
                    })
                    .collect();

                if let Some(last_message) = messages.last_mut() {
                    if !last_message.content.ends_with(" ```") {
                        last_message.content.push_str(" ```json");
                    }
                }
                let messages_json = serde_json::to_value(&messages)?;

                let max_tokens = std::cmp::max(5, 4089 - used_tokens);

                let payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "temperature": 0.7,
                    "max_tokens": max_tokens,
                });

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload).as_str(),
                );

                let res = client
                    .post(url)
                    .bearer_auth(key)
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await?;

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Status: {:?}", res.status()).as_str(),
                );

                let response_text = res.text().await?;
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );

                let data_resp: Result<JsonValue, _> = serde_json::from_str(&response_text);

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
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        Err(AgentError::SerdeError(e))
                    }
                }
            } else {
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
