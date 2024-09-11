use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::job::JobConfig;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::shared::ollama::{
    ollama_conversation_prepare_messages,
};
use crate::managers::model_capabilities_manager::PromptResultEnum;
use crate::network::ws_manager::{WSMessageType, WSMetadata, WSUpdateHandler};

use super::super::{error::LLMProviderError, execution::prompts::prompts::Prompt};
use super::ollama::truncate_image_content_in_payload;
use super::LLMService;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Exo, LLMProviderInterface, Ollama};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct ExoAPIStreamingResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub system_fingerprint: String,
    pub choices: Vec<ExoChoice>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExoChoice {
    pub index: i32,
    pub message: ExoMessage,
    pub logprobs: Option<JsonValue>,
    pub finish_reason: Option<String>,
    pub delta: ExoDelta,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExoMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExoDelta {
    pub role: String,
    pub content: String,
}

#[async_trait]
impl LLMService for Exo {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        _api_key: Option<&String>, // Note: not required
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            let url = format!("{}{}", base_url, "/v1/chat/completions");

            let messages_result = ollama_conversation_prepare_messages(&model, prompt)?;
            let messages_json = match messages_result.messages {
                PromptResultEnum::Value(v) => v,
                _ => {
                    return Err(LLMProviderError::UnexpectedPromptResultVariant(
                        "Expected Value variant in PromptResultEnum".to_string(),
                    ))
                }
            };

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                format!("Messages JSON: {:?}", messages_json).as_str(),
            );
            // Print messages_json as a pretty JSON string
            match serde_json::to_string_pretty(&messages_json) {
                Ok(pretty_json) => eprintln!("Messages JSON: {}", pretty_json),
                Err(e) => eprintln!("Failed to serialize messages_json: {:?}", e),
            };

            let mut payload = json!({
                "model": self.model_type,
                "messages": messages_json,
                "stream": true, // Yeah let's go wild and stream the response
                // Include any other optional parameters as needed
                // https://github.com/jmorganca/ollama/blob/main/docs/api.md#request-json-mode
            });

            // Modify payload to add options if needed
            add_options_to_payload(&mut payload);

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
            let mut previous_json_chunk: String = String::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(chunk) => {
                        let mut chunk_str = String::from_utf8_lossy(&chunk).to_string();
                        if !previous_json_chunk.is_empty() {
                            chunk_str = previous_json_chunk.clone() + chunk_str.as_str();
                        }
                        if chunk_str.starts_with("data: ") {
                            chunk_str = chunk_str.trim_start_matches("data: ").to_string();
                        }
                        let data_resp: Result<ExoAPIStreamingResponse, _> = serde_json::from_str(&chunk_str);
                        match data_resp {
                            Ok(data) => {
                                previous_json_chunk = "".to_string();
                                if let Some(choice) = data.choices.get(0) {
                                    response_text.push_str(&choice.delta.content);

                                    // Note: this is the code for enabling WS
                                    if let Some(ref manager) = ws_manager_trait {
                                        if let Some(ref inbox_name) = inbox_name {
                                            let m = manager.lock().await;
                                            let inbox_name_string = inbox_name.to_string();

                                            let metadata = WSMetadata {
                                                id: Some(session_id.clone()),
                                                is_done: choice.finish_reason.is_some(),
                                                done_reason: choice.finish_reason.clone(),
                                                total_duration: None, // Not available in the new format
                                                eval_count: None,     // Not available in the new format
                                            };

                                            let ws_message_type = WSMessageType::Metadata(metadata);

                                            let _ = m
                                                .queue_message(
                                                    WSTopic::Inbox,
                                                    inbox_name_string,
                                                    choice.delta.content.clone(),
                                                    ws_message_type,
                                                    true,
                                                )
                                                .await;
                                        }
                                    }
                                }
                            }
                            Err(_e) => {
                                previous_json_chunk += chunk_str.as_str();
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
                        return Err(LLMProviderError::NetworkError(e.to_string()));
                    }
                }
            }

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Cleaned Response Text: {:?}", response_text).as_str(),
            );

            // Directly return response_text with an empty JSON object
            Ok(LLMInferenceResponse::new(response_text, json!({}), None, None))
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

fn add_options_to_payload(payload: &mut serde_json::Value) {
    let mut options = serde_json::Map::new();

    // Helper function to read and parse environment variables
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }

    // Read and add options from environment variables
    if let Some(seed) = read_env_var::<u64>("LLM_SEED") {
        options.insert("seed".to_string(), serde_json::json!(seed));
    }
    if let Some(temp) = read_env_var::<f64>("LLM_TEMPERATURE") {
        options.insert("temperature".to_string(), serde_json::json!(temp));
    }
    if let Some(num_keep) = read_env_var::<u64>("LLM_NUM_KEEP") {
        options.insert("num_keep".to_string(), serde_json::json!(num_keep));
    }
    if let Some(num_predict) = read_env_var::<u64>("LLM_NUM_PREDICT") {
        options.insert("num_predict".to_string(), serde_json::json!(num_predict));
    }
    if let Some(top_k) = read_env_var::<u64>("LLM_TOP_K") {
        options.insert("top_k".to_string(), serde_json::json!(top_k));
    }
    if let Some(top_p) = read_env_var::<f64>("LLM_TOP_P") {
        options.insert("top_p".to_string(), serde_json::json!(top_p));
    }
    if let Some(tfs_z) = read_env_var::<f64>("LLM_TFS_Z") {
        options.insert("tfs_z".to_string(), serde_json::json!(tfs_z));
    }
    if let Some(typical_p) = read_env_var::<f64>("LLM_TYPICAL_P") {
        options.insert("typical_p".to_string(), serde_json::json!(typical_p));
    }
    if let Some(repeat_last_n) = read_env_var::<u64>("LLM_REPEAT_LAST_N") {
        options.insert("repeat_last_n".to_string(), serde_json::json!(repeat_last_n));
    }
    if let Some(repeat_penalty) = read_env_var::<f64>("LLM_REPEAT_PENALTY") {
        options.insert("repeat_penalty".to_string(), serde_json::json!(repeat_penalty));
    }
    if let Some(presence_penalty) = read_env_var::<f64>("LLM_PRESENCE_PENALTY") {
        options.insert("presence_penalty".to_string(), serde_json::json!(presence_penalty));
    }
    if let Some(frequency_penalty) = read_env_var::<f64>("LLM_FREQUENCY_PENALTY") {
        options.insert("frequency_penalty".to_string(), serde_json::json!(frequency_penalty));
    }
    if let Some(mirostat) = read_env_var::<u64>("LLM_MIROSTAT") {
        options.insert("mirostat".to_string(), serde_json::json!(mirostat));
    }
    if let Some(mirostat_tau) = read_env_var::<f64>("LLM_MIROSTAT_TAU") {
        options.insert("mirostat_tau".to_string(), serde_json::json!(mirostat_tau));
    }
    if let Some(mirostat_eta) = read_env_var::<f64>("LLM_MIROSTAT_ETA") {
        options.insert("mirostat_eta".to_string(), serde_json::json!(mirostat_eta));
    }
    if let Some(penalize_newline) = read_env_var::<bool>("LLM_PENALIZE_NEWLINE") {
        options.insert("penalize_newline".to_string(), serde_json::json!(penalize_newline));
    }
    if let Some(stop) = read_env_var::<String>("LLM_STOP") {
        options.insert(
            "stop".to_string(),
            serde_json::json!(stop.split(',').collect::<Vec<&str>>()),
        );
    }
    if let Some(numa) = read_env_var::<bool>("LLM_NUMA") {
        options.insert("numa".to_string(), serde_json::json!(numa));
    }
    if let Some(num_ctx) = read_env_var::<u64>("LLM_NUM_CTX") {
        options.insert("num_ctx".to_string(), serde_json::json!(num_ctx));
    }
    if let Some(num_batch) = read_env_var::<u64>("LLM_NUM_BATCH") {
        options.insert("num_batch".to_string(), serde_json::json!(num_batch));
    }
    if let Some(num_gpu) = read_env_var::<u64>("LLM_NUM_GPU") {
        options.insert("num_gpu".to_string(), serde_json::json!(num_gpu));
    }
    if let Some(main_gpu) = read_env_var::<u64>("LLM_MAIN_GPU") {
        options.insert("main_gpu".to_string(), serde_json::json!(main_gpu));
    }
    if let Some(low_vram) = read_env_var::<bool>("LLM_LOW_VRAM") {
        options.insert("low_vram".to_string(), serde_json::json!(low_vram));
    }
    if let Some(f16_kv) = read_env_var::<bool>("LLM_F16_KV") {
        options.insert("f16_kv".to_string(), serde_json::json!(f16_kv));
    }
    if let Some(vocab_only) = read_env_var::<bool>("LLM_VOCAB_ONLY") {
        options.insert("vocab_only".to_string(), serde_json::json!(vocab_only));
    }
    if let Some(use_mmap) = read_env_var::<bool>("LLM_USE_MMAP") {
        options.insert("use_mmap".to_string(), serde_json::json!(use_mmap));
    }
    if let Some(use_mlock) = read_env_var::<bool>("LLM_USE_MLOCK") {
        options.insert("use_mlock".to_string(), serde_json::json!(use_mlock));
    }
    if let Some(num_thread) = read_env_var::<u64>("LLM_NUM_THREAD") {
        options.insert("num_thread".to_string(), serde_json::json!(num_thread));
    }

    // Add options to payload if not empty
    if !options.is_empty() {
        payload["options"] = serde_json::Value::Object(options);
    }
}
