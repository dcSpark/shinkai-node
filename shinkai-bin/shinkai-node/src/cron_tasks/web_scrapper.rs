use std::collections::HashSet;
use std::{fmt, fs};

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::multipart::Form;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_vector_resources::file_parser::unstructured_api::UnstructuredAPI;
use uuid::Uuid;

use crate::db::db_cron_task::CronTask;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CronTaskRequest {
    pub crawl_links: bool,
    pub cron_description: String,
    pub task_description: String,
    pub object_description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CronTaskRequestResponse {
    pub cron_task_request: CronTaskRequest,
    pub cron_description: String,
    pub pddl_plan_problem: String,
    pub pddl_plan_domain: Option<String>,
}

impl fmt::Display for CronTaskRequestResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Cron Task Request: {:?}\nCron Description: {}\nPDDL Plan Problem: {}\nPDDL Plan Domain: {}",
            self.cron_task_request,
            self.cron_description,
            self.pddl_plan_problem,
            self.pddl_plan_domain.as_deref().unwrap_or("None")
        )
    }
}

#[derive(Debug, Clone)]
pub struct WebScraper {
    pub task: CronTask,
    pub unstructured_api: UnstructuredAPI,
}

#[derive(Debug, Clone)]
pub struct WebScraperResult {
    pub structured: String,
    pub unfiltered: String,
}

impl WebScraper {
    pub async fn download_and_parse(
        &self,
    ) -> Result<WebScraperResult, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut url = if self.task.url.starts_with("http://") || self.task.url.starts_with("https://") {
            self.task.url.clone()
        } else {
            format!("http://{}", &self.task.url)
        };

        // Remove trailing slash if it exists
        url = url.trim_end_matches('/').to_string();

        // Download the content
        // eprintln!("Downloading: {}", &url);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(20))
            .build()?;
        let response = client.get(&url).send().await?;
        let content = response.bytes().await?;

        // Get the current directory
        let mut dir_path = std::env::current_dir()?;
        dir_path.push("tmp_cron_downloads");

        // Create the directory if it doesn't exist
        std::fs::create_dir_all(&dir_path)?;

        // Parse the URL and get the path
        let url = Url::parse(&url)?;
        let url_path = url.path_segments().and_then(std::iter::Iterator::last);

        // Generate a random ID
        let id = Uuid::new_v4();

        // Create the file path
        let mut file_path = dir_path;
        file_path.push(format!("{}_{}.html", url_path.unwrap_or("file"), id));

        // Write the content to a file
        std::fs::write(&file_path, &content)?;

        // Print the content to the console
        let unfiltered = String::from_utf8_lossy(&content).to_string();

        // Create a multipart form with the file
        let part =
            reqwest::multipart::Part::bytes(fs::read(&file_path)?).file_name(file_path.to_string_lossy().to_string());
        let form = Form::new().part("files", part);

        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));

        let client = reqwest::Client::new();
        let response = client
            .post(&self.unstructured_api.endpoint_url())
            .headers(headers)
            .multipart(form)
            .send()
            .await?;

        // Delete the file
        let _ = std::fs::remove_file(&file_path);

        if response.status().is_success() {
            let response_body: Vec<Value> = response.json().await?;
            // eprintln!("\n\n\n Response: {:?}", response_body);
            if let Some(first_obj) = response_body.first() {
                match first_obj.get("text") {
                    Some(text) => Ok(WebScraperResult {
                        structured: text.as_str().unwrap_or("").to_string(),
                        unfiltered,
                    }),
                    None => Err("Field 'text' not found in the response".into()),
                }
            } else {
                Err("Response array is empty".into())
            }
        } else {
            let status = response.status();
            let response_body: Value = response.json().await?;
            Err(format!(
                "File upload failed with status code: {}. Response: {:?}",
                status, response_body
            )
            .into())
        }
    }

    pub fn extract_links(content: &str) -> Vec<String> {
        let mut links = HashSet::new();

        let url_re1 = regex::Regex::new(r"(http|https)://([^\s\)]+)").unwrap();
        for mat in url_re1.find_iter(content) {
            links.insert(mat.as_str().to_string());
        }

        let url_re2 = regex::Regex::new(r"\(\s*([^\s()]+\.[^\s()]+)\s*\)").unwrap();
        for cap in url_re2.captures_iter(content) {
            if let Some(mat) = cap.get(1) {
                links.insert(mat.as_str().to_string());
            }
        }

        links.into_iter().collect()
    }
}
