use crate::agent::job_manager::JobManager;
use crate::agent::providers::shared::ollama::OllamaAPIStreamingResponse;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};

use super::super::{error::AgentError, execution::prompts::prompts::Prompt};
use super::LLMProvider;
use async_trait::async_trait;
use futures::StreamExt;
use regex::Regex;
use reqwest::Client;
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, Ollama};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::error::Error;

fn truncate_image_content_in_payload(payload: &mut JsonValue) {
    if let Some(images) = payload.get_mut("images") {
        if let Some(array) = images.as_array_mut() {
            for image in array {
                if let Some(str_image) = image.as_str() {
                    let truncated_image = format!("{}...", &str_image[0..20.min(str_image.len())]);
                    *image = JsonValue::String(truncated_image);
                }
            }
        }
    }
}

pub fn parse_markdown_to_json(markdown: &str) -> Result<JsonValue, AgentError> {
    // Find the index of the first '#' and slice the string from there
    let start_index = markdown.find('#').unwrap_or(0);
    let trimmed_markdown = &markdown[start_index..];

    let mut sections = serde_json::Map::new();
    let re = Regex::new(r"(?m)^# (\w+)$").unwrap();
    let mut current_section = None;
    let mut content = String::new();

    for line in trimmed_markdown.lines() {
        if let Some(caps) = re.captures(line) {
            if let Some(section) = current_section {
                sections.insert(section, JsonValue::String(content.trim().to_string()));
                content.clear();
            }
            current_section = Some(caps[1].to_string());
        } else if current_section.is_some() {
            content.push_str(line);
            content.push('\n');
        } else {
            current_section = Some("".to_string());
            content.push_str(line);
            content.push('\n');
        }
    }

    if let Some(section) = current_section {
        sections.insert(section, JsonValue::String(content.trim().to_string()));
    }

    Ok(JsonValue::Object(sections))
}

#[async_trait]
impl LLMProvider for Ollama {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        _api_key: Option<&String>, // Note: not required
        prompt: Prompt,
        model: AgentLLMInterface,
    ) -> Result<JsonValue, AgentError> {
        if let Some(base_url) = url {
            let url = format!("{}{}", base_url, "/api/generate");
            let messages_result = ModelCapabilitiesManager::route_prompt_with_model(prompt, &model).await?;
            let (messages_string, asset_content) = match messages_result.value {
                PromptResultEnum::Text(v) => (v, None),
                PromptResultEnum::ImageAnalysis(v, i) => (v, Some(i)),
                _ => {
                    return Err(AgentError::UnexpectedPromptResultVariant(
                        "Expected Value variant in PromptResultEnum".to_string(),
                    ))
                }
            };

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                format!("Messages JSON: {:?}", messages_string).as_str(),
            );

            let mut payload = json!({
                "model": self.model_type,
                "prompt": messages_string,
                "stream": true, // Yeah let's go wild and stream the response
                // Include any other optional parameters as needed
                // https://github.com/jmorganca/ollama/blob/main/docs/api.md#request-json-mode
            });

            if let Some(asset_content) = asset_content {
                let asset_content_str = asset_content.to_string();
                payload["images"] = json!([asset_content_str]);
            }

            let mut payload_log = payload.clone();
            truncate_image_content_in_payload(&mut payload_log);

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Call API Body: {:?}", payload_log).as_str(),
            );

            let res = client.post(url).json(&payload).send().await?;

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Call API Status: {:?}", res.status()).as_str(),
            );

            let mut stream = res.bytes_stream();
            let mut response_text = String::new();

            while let Some(item) = stream.next().await {
                match item {
                    Ok(chunk) => {
                        let chunk_str = String::from_utf8_lossy(&chunk).to_string();
                        let data_resp: Result<OllamaAPIStreamingResponse, _> = serde_json::from_str(&chunk_str);
                        match data_resp {
                            Ok(data) => {
                                if let Some(response) = data.response.as_str() {
                                    response_text.push_str(response);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse line: {:?}", e);
                                // Handle JSON parsing error here...
                            }
                        }
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source()).as_str(),
                        );
                        return Err(AgentError::NetworkError(e.to_string()));
                    }
                }
            }

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Cleaned Response Text: {:?}", response_text).as_str(),
            );

            match parse_markdown_to_json(&response_text) {
                Ok(json) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Debug,
                        format!("Parsed JSON from Markdown: {:?}", json).as_str(),
                    );
                    Ok(json)
                }
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        format!("Failed to parse Markdown to JSON: {:?}", e).as_str(),
                    );
                    Err(e)
                }
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
