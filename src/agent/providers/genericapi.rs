use crate::agent::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::agent::job_manager::JobManager;
use crate::agent::providers::shared::shared_model_logic::parse_markdown_to_json;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;

use super::super::{error::AgentError, execution::prompts::prompts::Prompt};
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
    ) -> Result<LLMInferenceResponse, AgentError> {
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
                        "<|eot_id|>",
                        "[/INST]",
                        "</s>",
                        "Sys:"
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

                match data_resp {
                    Ok(data) => {
                        // Comment(Nico): maybe we could go over all the choices and check for the ones that can convert to json with our format
                        // and from those the longest one. I haven't see multiple choices so far though.
                        let response_string: String = data
                            .output
                            .choices
                            .first()
                            .map(|choice| choice.text.clone())
                            .unwrap_or_else(String::new);

                        match parse_markdown_to_json(&response_string) {
                            Ok(json) => {
                                shinkai_log(
                                    ShinkaiLogOption::JobExecution,
                                    ShinkaiLogLevel::Debug,
                                    format!("Parsed JSON from Markdown: {:?}", json).as_str(),
                                );
                                Ok(LLMInferenceResponse::new(response_string, json))
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
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        Err(AgentError::SerdeError(e))
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
