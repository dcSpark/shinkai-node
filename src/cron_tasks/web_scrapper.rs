use std::fs;

use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use uuid::Uuid;

use crate::db::db_cron_task::CronTask;

#[derive(Debug, Clone)]
pub struct WebScraper {
    pub task: CronTask,
    pub api_url: String,
}

impl WebScraper {
    pub async fn download_and_parse(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        // Download the content
        eprintln!("Downloading: {}", &self.task.url);
        let response = reqwest::get(&self.task.url).await?;
        eprintln!("Response: {:?}", response);
        let content = response.bytes().await?;
    
        // Get the current directory
        let mut dir_path = std::env::current_dir()?;
        dir_path.push("tmp_cron_downloads");
    
        // Create the directory if it doesn't exist
        std::fs::create_dir_all(&dir_path)?;
    
        // Parse the URL and get the path
        let url = Url::parse(&self.task.url)?;
        let url_path = url.path_segments().and_then(std::iter::Iterator::last);
    
        // Generate a random ID
        let id = Uuid::new_v4();
    
        // Create the file path
        let mut file_path = dir_path;
        file_path.push(format!("{}_{}.html", url_path.unwrap_or("file"), id));
    
        // Write the content to a file
        std::fs::write(&file_path, &content)?;
    
        // Create a multipart form with the file
        let part = reqwest::multipart::Part::bytes(fs::read(&file_path)?).file_name(file_path.to_string_lossy().to_string());
        let form = Form::new().part("files", part);
    
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
    
        let client = reqwest::Client::new();
        let response = client.post(&self.api_url)
            .headers(headers)
            .multipart(form)
            .send()
            .await?;
    
        // Delete the file
        let _ = std::fs::remove_file(&file_path);
    
        if response.status().is_success() {
            let response_body: Vec<Value> = response.json().await?;
            if let Some(first_obj) = response_body.get(0) {
                match first_obj.get("text") {
                    Some(text) => Ok(text.as_str().unwrap_or("").to_string()),
                    None => Err("Field 'text' not found in the response".into()),
                }
            } else {
                Err("Response array is empty".into())
            }
        } else {
            let status = response.status();
            let response_body: Value = response.json().await?;
            Err(format!("File upload failed with status code: {}. Response: {:?}", status, response_body).into())
        }
    }

    pub fn extract_links(content: &str) -> Vec<String> {
        let url_re = regex::Regex::new(r"(http|https)://([^\s]+)").unwrap();
        url_re
            .find_iter(content)
            .map(|mat| mat.as_str().to_string())
            .collect()
    }
}