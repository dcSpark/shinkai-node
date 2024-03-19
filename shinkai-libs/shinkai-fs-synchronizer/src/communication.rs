use reqwest::{Client, Error};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostDataResponse {
    pub status: String,
    pub data: serde_json::Value,
}

pub async fn request_post(input: String, path: &str) -> Result<PostDataResponse, String> {
    let client = Client::new();
    let shinkai_node_url = env::var("SHINKAI_NODE_URL").expect("SHINKAI_NODE_URL must be set");
    let url = format!("{}{}", shinkai_node_url, path);

    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(input)
        .send()
        .await
    {
        Ok(response) => match response.json::<PostDataResponse>().await {
            Ok(data) => {
                dbg!(data.clone());
                Ok(data)
            }
            Err(e) => {
                eprintln!("Error parsing response: {:?}", e);
                Err(format!("Error parsing response: {:?}", e))
            }
        },
        Err(e) => {
            eprintln!("Error when interacting with {}. Error: {:?}", path, e);
            Err(format!("Error when interacting with {}. Error: {:?}", path, e))
        }
    }
}
