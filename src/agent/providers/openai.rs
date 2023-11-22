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
use tiktoken_rs::model::get_context_size;
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

                let total_tokens = Self::get_max_tokens(self.model_type.as_str());
                let tiktoken_messages = prompt.generate_openai_messages(Some(total_tokens / 2))?;
                let used_tokens = num_tokens_from_messages(
                    Self::normalize_model(&self.model_type.clone()).as_str(),
                    &tiktoken_messages,
                )?;
                let mut max_tokens = std::cmp::max(5, total_tokens - used_tokens);
                max_tokens = std::cmp::min(max_tokens, Self::get_max_output_tokens(self.model_type.as_str()));

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

                let payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "temperature": 0.7,
                    "max_tokens": max_tokens,
                    "response_format": { "type": "json_object" }
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

    /// Returns the maximum number of tokens supported based on
    /// the provided model string
    fn get_max_tokens(s: &str) -> usize {
        // Custom added, since not supported by Tiktoken atm
        if s == "gpt-4-1106-preview" {
            128_000
        } else {
            let normalized_model = Self::normalize_model(s);
            get_context_size(normalized_model.as_str())
        }
    }

    /// Returns a maximum number of output tokens
    fn get_max_output_tokens(s: &str) -> usize {
        4096
    }

    /// Normalizes the model string to one that is supported by Tiktoken crate
    fn normalize_model(s: &str) -> String {
        if s.to_string().starts_with("gpt-4") {
            "gpt-4-32k".to_string()
        } else if s.to_string().starts_with("gpt-3.5") {
            "gpt-3.5-turbo-16k".to_string()
        } else {
            "gpt-4".to_string()
        }
    }
}
