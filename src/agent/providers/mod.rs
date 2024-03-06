use super::{error::AgentError, execution::job_prompts::Prompt};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value as JsonValue;

pub mod genericapi;
pub mod ollama;
pub mod openai;
pub mod shared;
pub mod shinkai_backend;

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

    /// Given an input string, parses the largest JSON object that it finds. Largest allows us to skip over
    /// cases where LLMs repeat your schema or post other broken/smaller json objects for whatever reason.
    fn extract_largest_json_object(s: &str) -> Result<JsonValue, AgentError> {
        match internal_extract_json_string(s) {
            Ok(json_str) => match serde_json::from_str(&json_str) {
                Ok(json_val) => Ok(json_val),
                Err(e) => {
                    // If parsing fails, clean up the string and try again
                    let cleaned_json_string = Self::json_string_cleanup(&json_str);
                    match internal_extract_json_string(&cleaned_json_string) {
                        Ok(re_json_str) => match serde_json::from_str(&re_json_str) {
                            Ok(obj) => Ok(obj),
                            Err(e) => Err(AgentError::FailedSerdeParsingJSONString(re_json_str, e)),
                        },
                        Err(e) => Err(e),
                    }
                }
            },
            Err(e) => Err(e),
        }
    }

    /// Given a JSON string, it cleans it up and returns a more likely readable JSON string
    fn json_string_cleanup(s: &str) -> String {
        let mut response_string = replace_single_quotes(s);

        // Quote & underscore fixes
        response_string = response_string.replace("\\'", "\'");
        response_string = response_string.replace("\\\'", "\'");
        response_string = response_string.replace("\\\"", "\"");
        response_string = response_string.replace("\\_", "_");
        response_string = response_string.replace("\\\\_", "_");

        // Remove system messages from LLM models
        response_string = response_string.replace("<</SYS>>", "");
        response_string = response_string.replace("<<SYS>>", "");

        // Further more manual fixes
        response_string = if response_string.starts_with("- \n\n") {
            response_string[4..].to_string()
        } else {
            response_string
        };
        response_string = response_string.trim_end_matches(" ```").to_string();

        // Cuts off everything after a triple ` by the end
        let pattern1 = "}\n ```";
        let pattern2 = "\n```";

        if let Some(end_index) = response_string
            .find(pattern1)
            .or_else(|| response_string.find(pattern2))
        {
            response_string = response_string[..end_index + 1].to_string();
        }

        // Extra linebreak replaces
        response_string = response_string.replace("\"\n}\n``` ", "\"}");
        response_string = response_string.replace("\\n", " ");
        response_string = response_string.replace("{\" \"", "{ \"");
        response_string = response_string.replace("\" \"}", "\" }");

        // Check for and remove an extra set of curly braces after everything else
        let trimmed_string = response_string.trim();
        if trimmed_string.starts_with("{{") && trimmed_string.ends_with("}}") {
            response_string = trimmed_string[1..trimmed_string.len() - 1].to_string();
        }

        response_string
    }
}

fn replace_single_quotes(s: &str) -> String {
    let replaced_string = s.to_string().replace("''", "'");
    let mut chars = replaced_string.chars().peekable();
    let mut cleaned_string = String::new();
    let mut in_quotes = false; // Tracks whether we are inside quotes

    while let Some(c) = chars.next() {
        match c {
            // If we encounter a double quote, we flip the in_quotes flag
            '"' => {
                cleaned_string.push(c);
                in_quotes = !in_quotes;
            }
            // If we encounter a single quote and we are not inside quotes, replace it with a double quote
            '\'' if !in_quotes => {
                cleaned_string.push('"');
            }
            // If we are inside quotes or it's any other character, just append it
            _ => cleaned_string.push(c),
        }
    }

    cleaned_string
}

/// Attempts to extract out all json strings from the input by matching braces and returns the longest one.
fn internal_extract_json_string(s: &str) -> Result<String, AgentError> {
    let mut depth = 0;
    let mut start = None;
    let mut json_strings = Vec::new();

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
                    if let Some(start_index) = start {
                        // Add the found JSON string to the vector
                        json_strings.push(s[start_index..=i].to_string());
                        start = None; // Reset start for the next JSON string
                    } else {
                        println!("\n\n1 - Failed string to parse as json: {}\n\n", s);
                        return Err(AgentError::FailedExtractingJSONObjectFromResponse(s.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    // Return the longest JSON string
    match json_strings.into_iter().max_by_key(|s| s.len()) {
        Some(longest_json_string) => Ok(longest_json_string),
        None => Err(AgentError::FailedExtractingJSONObjectFromResponse(
            s.to_string() + " - No JSON strings found",
        )),
    }
}
