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

pub async fn request_post(input: String, path: &str) -> Result<PostDataResponse, String> {
    let client = Client::new();
    let shinkai_node_url = env::var("SHINKAI_NODE_URL").expect("SHINKAI_NODE_URL must be set");
    let url = format!("{}{}", shinkai_node_url, path);

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

pub async fn node_init() -> anyhow::Result<ShinkaiManager> {
    loop {
        let check_health_result = ShinkaiManager::check_node_health().await;
        let check_health = check_health_result.map_err(anyhow::Error::msg)?;

        if check_health.status == "ok" {
            match ShinkaiManager::initialize_node_connection(check_health).await {
                Ok(manager) => {
                    return Ok(manager);
                }
                Err(e) => {
                    eprintln!("Failed to initialize node connection: {}", e);
                }
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}
