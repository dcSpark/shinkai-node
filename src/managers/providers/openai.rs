use serde::Deserialize;
use serde_json;
use std::error::Error;

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

impl Provider for Response {
    type Response = Response;

    fn parse_response(response_body: &str) -> Result<Self::Response, Box<dyn Error>> {
        let res: Result<Response, serde_json::Error> = serde_json::from_str(response_body);
        match res {
            Ok(response) => Ok(response),
            Err(e) => Err(Box::new(e)),
        }
    }
    
    fn extract_content(response: &Self::Response) -> String {
        response.choices.get(0).map_or(String::new(), |choice| choice.message.content.clone())
    }
}