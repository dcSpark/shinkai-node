use super::super::{error::AgentError, execution::job_prompts::Prompt};
use super::openai::OpenAIApiMessage;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::GenericAPI;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tiktoken_rs::get_chat_completion_max_tokens;
use tiktoken_rs::num_tokens_from_messages;

#[derive(Serialize, Deserialize)]
struct APIResponse {
    status: String,
    prompt: Vec<String>,
    model: String,
    model_owner: String,
    tags: serde_json::Map<String, serde_json::Value>,
    num_returns: i32,
    args: Args,
    subjobs: Vec<String>,
    output: Output,
}

#[derive(Serialize, Deserialize)]
struct Args {
    model: String,
    prompt: String,
    temperature: f32,
    top_p: f32,
    top_k: i32,
    max_tokens: i32,
}

#[derive(Serialize, Deserialize)]
struct Output {
    choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize)]
struct Choice {
    finish_reason: Option<String>,
    index: Option<i32>,
    text: String,
}

#[async_trait]
impl LLMProvider for GenericAPI {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/inference");
                let messages_string = prompt.generate_genericapi_messages(None)?;
                eprintln!("Tiktoken messages: {:?}", messages_string);

                let messages_string = messages_string
                    .split("\n\n")
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .join("\n\n");
                // let messages_json = serde_json::to_value(&messages_string)?.to_string();
                let messages_json = format!("{}", serde_json::to_value(&messages_string)?.to_string());
                let messages_json = messages_json.strip_prefix('\"').unwrap_or(&messages_json);
                let messages_json = messages_json.strip_suffix("\\\"\\n\\n ```").unwrap_or(&messages_json);

                eprintln!("###");
                eprintln!("Messages JSON: {:?}", messages_json);
                eprintln!("###");
                // panic!();
                // let max_tokens = std::cmp::max(5, 4097 - used_characters);

                let payload = json!({
                    "model": self.model_type,
                    "max_tokens": 2048,
                    "prompt": messages_json,
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

                let body = serde_json::to_string(&payload)?;

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", body).as_str(),
                );

                let res = client
                    .post(url)
                    .bearer_auth(key)
                    .header("Content-Type", "application/json")
                    .body(body)
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
                    ShinkaiLogLevel::Debug,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );

                let data_resp: Result<APIResponse, _> = serde_json::from_str(&response_text);

                match data_resp {
                    Ok(data) => {
                        let response_string: String = data
                            .output
                            .choices
                            .iter()
                            .map(|choice| choice.text.clone())
                            .collect::<Vec<String>>()
                            .join(" ");
                        eprintln!("######");
                        eprintln!("Response string: {:?}", response_string);
                        Self::extract_first_json_object(&response_string)
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
