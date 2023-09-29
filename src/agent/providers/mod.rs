use super::{error::AgentError, job_prompts::Prompt};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value as JsonValue;

pub mod openai;
pub mod sleep_api;

#[async_trait]
pub trait LLMProvider {
    // type Response;
    // fn parse_response(response_body: &str) -> Result<Self::Response, AgentError>;
    // fn extract_content(response: &Self::Response) -> Result<JsonValue, AgentError>;
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
    ) -> Result<JsonValue, AgentError>;

    /// Given an input string, parses the first JSON object that it finds
    fn extract_first_json_object(s: &str) -> Result<JsonValue, AgentError> {
        let mut depth = 0;
        let mut start = None;

        for (i, c) in s.char_indices() {
            match c {
                '{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let json_str = &s[start.unwrap()..=i];
                        let json_val: JsonValue = serde_json::from_str(json_str)
                            .map_err(|_| AgentError::FailedExtractingJSONObjectFromResponse(s.to_string()))?;
                        return Ok(json_val);
                    }
                }
                _ => {}
            }
        }

        Err(AgentError::FailedExtractingJSONObjectFromResponse(s.to_string()))
    }
}
