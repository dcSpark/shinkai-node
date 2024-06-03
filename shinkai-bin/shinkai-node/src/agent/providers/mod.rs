use super::{
    error::AgentError,
    execution::{chains::inference_chain_trait::LLMInferenceResponse, prompts::prompts::Prompt},
};
use async_trait::async_trait;
use quickxml_to_serde::{xml_string_to_json, Config, NullValue};
use reqwest::Client;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::AgentLLMInterface;

pub mod genericapi;
pub mod groq;
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
        model: AgentLLMInterface,
    ) -> Result<LLMInferenceResponse, AgentError>;

    /// Given an input string, parses the first XML object that it finds.
    fn extract_first_xml_object_into_json_value(s: &str) -> Result<JsonValue, AgentError> {
        let xml_strings = Self::internal_extract_xml_strings(s);
        if let Ok(xml_strings) = xml_strings {
            let first_string = xml_strings[0].clone(); // Should be safe as we already checked for empty when extracting the strings
            let conf = Config::new_with_defaults();
            let json = xml_string_to_json(first_string, &conf);
            json.or(Err(AgentError::FailedSerdeParsingXMLString(
                s.to_string(),
                minidom::Error::InvalidElement,
            )))
        } else {
            xml_strings?;
            Err(AgentError::ContentParseFailed)
        }

        // match xml_strings {
        // Ok(json_str) => match serde_json::from_str(&json_str) {
        //     Ok(json_val) => Ok(json_val),
        //     Err(_e) => {
        //         // If parsing fails, clean up the string and try again
        //         let cleaned_json_string = Self::json_string_cleanup(&json_str);
        //         match internal_extract_json_string(&cleaned_json_string) {
        //             Ok(re_json_str) => match serde_json::from_str(&re_json_str) {
        //                 Ok(obj) => Ok(obj),
        //                 Err(e) => Err(AgentError::FailedSerdeParsingXMLString(re_json_str, _e)),
        //             },
        //             Err(e) => Err(e),
        //         }
        //     }
        // },
        // Err(e) => Err(e),
        // }
    }

    /// Attempts to extract out all top-level XML strings from the input by matching start and end tags and returns them as separate strings.
    fn internal_extract_xml_strings(s: &str) -> Result<Vec<String>, AgentError> {
        let mut xml_strings = Vec::new();
        let mut tag_start = None;
        let mut current_tag = None;

        let mut chars = s.chars().enumerate().peekable();

        while let Some((i, c)) = chars.next() {
            match c {
                '<' if chars.peek().map_or(false, |&(_, next_char)| next_char != '/') => {
                    // Detect the start of a new tag
                    if tag_start.is_none() {
                        tag_start = Some(i);
                        // Capture the tag name
                        let mut tag_name = String::new();
                        while let Some((_, char)) = chars.next() {
                            if char == '>' {
                                break;
                            }
                            if char.is_whitespace() || char == '/' {
                                break;
                            }
                            tag_name.push(char);
                        }
                        current_tag = Some(tag_name);
                    }
                }
                '>' if current_tag.is_some() => {
                    // Look for the closing tag
                    let tag = current_tag.clone().unwrap();
                    let end_tag = format!("</{}>", tag);
                    if let Some(end) = s[i..].find(&end_tag) {
                        let end_index = i + end + end_tag.len();
                        if let Some(start_index) = tag_start {
                            xml_strings.push(s[start_index..end_index].to_string());
                        }
                        // Reset for next potential tag
                        tag_start = None;
                        current_tag = None;
                        // Move iterator to end of current tag
                        for _ in 0..end + end_tag.len() - 1 {
                            chars.next();
                        }
                    }
                }
                _ => {}
            }
        }

        if xml_strings.is_empty() {
            Err(AgentError::FailedSerdeParsingXMLString(
                s.to_string(),
                minidom::Error::InvalidElement,
            ))
        } else {
            Ok(xml_strings)
        }
    }

    /// Given an input string, parses the largest JSON object that it finds. Largest allows us to skip over
    /// cases where LLMs repeat your schema or post other broken/smaller json objects for whatever reason.
    fn extract_largest_json_object(s: &str) -> Result<JsonValue, AgentError> {
        match internal_extract_json_string(s) {
            Ok(json_str) => match serde_json::from_str(&json_str) {
                Ok(json_val) => Ok(json_val),
                Err(_e) => {
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
                        return Err(AgentError::FailedExtractingJSONObjectFromResponse(s.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    // Return the longest JSON string
    match json_strings.into_iter().max_by_key(|jstr| jstr.len()) {
        Some(longest_json_string) => Ok(longest_json_string),
        None => Err(AgentError::FailedExtractingJSONObjectFromResponse(
            s.to_string() + " - No JSON strings found",
        )),
    }
}
