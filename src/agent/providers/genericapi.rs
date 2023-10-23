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
                let mut messages_string = prompt.generate_genericapi_messages(None)?;
                if !messages_string.ends_with(" ```") {
                    messages_string.push_str(" ```json");
                }

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Info,
                    format!("Messages JSON: {:?}", messages_string).as_str(),
                );

                // panic!();
                // let max_tokens = std::cmp::max(5, 4097 - used_characters);

                let payload = json!({
                    "model": self.model_type,
                    "max_tokens": 2800,// 4096 is max amount of tokens
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

                // let body = serde_json::to_string(&payload)?;

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

                let data_resp: Result<APIResponse, _> = serde_json::from_str(&response_text);

                match data_resp {
                    Ok(data) => {
                        // Comment(Nico): maybe we could go over all the choices and check for the ones that can convert to json with our format
                        // and from those the longest one. I haven't see multiple choices so far though.
                        let mut response_string: String = data
                            .output
                            .choices
                            .first()
                            .map(|choice| choice.text.clone())
                            .unwrap_or_else(|| String::new());

                        eprintln!("#####################################################################################");
                        eprintln!("Response string: {:?}", response_string);

                        // Code to clean up the response string
                        response_string = if response_string.starts_with("- \n\n") {
                            response_string[4..].to_string()
                        } else {
                            response_string
                        };
                        response_string = response_string.trim_end_matches(" ```").to_string();

                        // Replace single quotes with double quotes in specific parts of the string
                        response_string = response_string.replace("{ 'answer'", "{ \"answer\"");
                        response_string = response_string.replace(": '", ": \"");
                        response_string = response_string.replace("' }", "\" }");
                        // End cleaning up code

                        eprintln!("(Cleaned up) Response string: {:?}", response_string);

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
