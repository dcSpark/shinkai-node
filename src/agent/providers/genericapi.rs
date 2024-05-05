use crate::agent::job_manager::JobManager;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;

use super::super::{error::AgentError, execution::prompts::prompts::Prompt};
use super::ollama::parse_markdown_to_json;
use super::shared::togetherai::TogetherAPIResponse;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;

use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, GenericAPI};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};

#[async_trait]
impl LLMProvider for GenericAPI {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: AgentLLMInterface,
    ) -> Result<JsonValue, AgentError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/inference");

                let max_tokens = ModelCapabilitiesManager::get_max_tokens(&model);
                let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(&model);
                let max_output_tokens = ModelCapabilitiesManager::get_max_output_tokens(&model);
                let messages_string = prompt.generate_genericapi_messages(Some(max_input_tokens))?;

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Info,
                    format!("Messages JSON: {:?}", messages_string).as_str(),
                );

                let payload = json!({
                    "model": self.model_type,
                    "max_tokens": max_output_tokens,
                    "prompt": messages_string,
                    "request_type": "language-model-inference",
                    "temperature": 0.7,
                    "top_p": 0.7,
                    "top_k": 50,
                    "repetition_penalty": 1,
                    "stream_tokens": false,
                    "stop": [
                        "[/INST]",
                        "</s>"
                    ],
                    "negative_prompt": "",
                    "safety_model": "",
                    "repetitive_penalty": 1,
                });

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload).as_str(),
                );

                let res = client
                    .post(url)
                    .bearer_auth(key)
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await?;

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Status: {:?}", res.status()).as_str(),
                );

                let response_text = res.text().await?;
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Info,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );
                let data_resp: Result<TogetherAPIResponse, _> = serde_json::from_str(&response_text);

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
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
