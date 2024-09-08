use std::sync::Arc;

use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::job::JobConfig;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::network::ws_manager::WSUpdateHandler;

use super::super::{error::LLMProviderError, execution::prompts::prompts::Prompt};
use super::shared::togetherai::TogetherAPIResponse;
use super::LLMService;
use async_trait::async_trait;
use reqwest::Client;

use serde_json;
use serde_json::json;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{GenericAPI, LLMProviderInterface};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tokio::sync::Mutex;

#[async_trait]
impl LLMService for GenericAPI {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        _ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
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

                        return Ok(LLMInferenceResponse::new(response_string, json!({}), None));
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        Err(LLMProviderError::SerdeError(e))
                    }
                }
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}
