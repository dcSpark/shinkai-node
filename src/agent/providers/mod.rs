use super::{error::AgentError, execution::job_prompts::Prompt};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value as JsonValue;

pub mod openai;
pub mod genericapi;
pub mod ollama;
pub mod shinkai_backend;
pub mod shared;

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

    /// Given a JSON string, it cleans it up and returns a more likely readable JSON string
    fn json_string_cleanup(s: &str) -> String {
        let mut response_string = s.to_string();

        // Code to clean up the response string
        response_string = if response_string.starts_with("- \n\n") {
            response_string[4..].to_string()
        } else {
            response_string
        };
        response_string = response_string.replace("\\\"", "\"");
        response_string = response_string.trim_end_matches(" ```").to_string();

        // Replace single quotes with double quotes in specific parts of the string
        response_string = response_string.replace("{ 'answer'", "{ \"answer\"");
        response_string = response_string.replace(": '", ": \"");
        response_string = response_string.replace("' }", "\" }");

        // it cuts off everything after a triple single quotes by the end
        let pattern1 = "}\n ```";
        let pattern2 = "\n```";
        let mut json_part = response_string.clone();

        if let Some(end_index) = response_string
            .find(pattern1)
            .or_else(|| response_string.find(pattern2))
        {
            json_part = response_string[..end_index + 1].to_string();
            // +1 to include the closing brace of the JSON object
        }

        json_part = json_part.replace("\"\n}\n``` ", "\"}");

        json_part
    }

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

    fn get_max_tokens(s: &str) -> usize;
    fn get_max_output_tokens(s: &str) -> usize;
    fn normalize_model(s: &str) -> String;
}
