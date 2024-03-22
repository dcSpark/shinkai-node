use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::shinkai_manager::ShinkaiManager;
use std::env;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostDataResponse {
    pub status: String,
    pub data: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub scheme: String,
    pub cannot_be_a_base: bool,
    pub username: String,
    pub password: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
    pub status: String,
}

pub async fn request_post(api_url: String, input: String, path: &str) -> Result<PostDataResponse, String> {
    let client = Client::new();
    let url = format!("{}{}", api_url, path);

    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(input.clone()) // Clone input for debugging
        .send()
        .await
    {
        Ok(response) => {
            // Print the payload before attempting to map it
            println!("response: {:?}", response);
            let response_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to get response text".to_string());
            match serde_json::from_str::<PostDataResponse>(&response_text) {
                Ok(data) => Ok(data),
                Err(e) => {
                    eprintln!("Error parsing response: {:?}", e);
                    eprintln!("Response text: {}", response_text);
                    Err(format!("Error parsing response: {:?}", e))
                }
            }
        }
        Err(e) => {
            eprintln!("Error when interacting with {}. Error: {:?}", path, e);
            Err(format!("Error when interacting with {}. Error: {:?}", path, e))
        }
    }
}
