use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::shinkai_manager::ShinkaiManager;
use std::env;

#[derive(Debug)]
pub enum PostRequestError {
    RequestFailed(String),
    InvalidResponse(String),
    SerializationError(String),
    FSFolderNotFound(String),
    Unknown(String),
}

impl From<PostRequestError> for String {
    fn from(error: PostRequestError) -> Self {
        match error {
            PostRequestError::RequestFailed(msg) => msg,
            PostRequestError::InvalidResponse(msg) => msg,
            PostRequestError::SerializationError(msg) => msg,
            PostRequestError::Unknown(msg) => msg,
            PostRequestError::FSFolderNotFound(msg) => msg,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostDataResponse {
    pub status: String,
    pub data: serde_json::Value,
}

use reqwest::header::HeaderMap;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DefaultResponse {
    pub url: UrlDetails,
    pub status: u16,
    pub headers: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UrlDetails {
    pub scheme: String,
    pub cannot_be_a_base: bool,
    pub username: String,
    pub password: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

pub async fn request_post(api_url: String, input: String, path: &str) -> Result<PostDataResponse, PostRequestError> {
    let client = Client::new();
    let url = format!("{}{}", api_url, path);

    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(input.clone())
        .send()
        .await
    {
        Ok(response) => {
            let status_code = response.status();
            let response_path = response.url().path().to_string();

            let response_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to get response text".to_string());

            if status_code == 500 {
                if response_path == "/v1/vec_fs/retrieve_path_simplified_json" {
                    return Err(PostRequestError::FSFolderNotFound("FS folder not found.".to_string()));
                }
            }

            // TODO: handle 400 specifically
            if status_code == 400 {}

            match serde_json::from_str::<PostDataResponse>(&response_text) {
                Ok(data) => Ok(data),
                Err(e) => Err(PostRequestError::SerializationError(e.to_string())),
            }
        }
        Err(e) => Err(PostRequestError::InvalidResponse(format!(
            "Error when interacting with {}. Error: {:?}",
            path, e
        ))),
    }
}
