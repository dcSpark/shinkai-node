use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shinkai_graphrag::llm::llm::{BaseLLM, BaseLLMCallback, GlobalSearchPhase, LLMParams, MessageType};

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaMessage,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

pub struct Ollama {
    base_url: String,
    model_type: String,
}

impl Ollama {
    pub fn new(base_url: String, model_type: String) -> Self {
        Ollama { base_url, model_type }
    }
}

#[async_trait]
impl BaseLLM for Ollama {
    async fn agenerate(
        &self,
        messages: MessageType,
        _streaming: bool,
        _callbacks: Option<Vec<BaseLLMCallback>>,
        _llm_params: LLMParams,
        search_phase: Option<GlobalSearchPhase>,
    ) -> anyhow::Result<String> {
        let client = Client::new();
        let chat_url = format!("{}{}", &self.base_url, "/api/chat");

        let messages_json = match messages {
            MessageType::String(message) => json![message],
            MessageType::Strings(messages) => json!(messages),
            MessageType::Dictionary(messages) => {
                let messages = match search_phase {
                    Some(GlobalSearchPhase::Map) => {
                        // Filter out system messages and convert them to user messages
                        messages
                            .into_iter()
                            .filter(|map| map.get_key_value("role").is_some_and(|(_, v)| v == "system"))
                            .map(|map| {
                                map.into_iter()
                                    .map(|(key, value)| {
                                        if key == "role" {
                                            return (key, "user".to_string());
                                        }
                                        (key, value)
                                    })
                                    .collect()
                            })
                            .collect()
                    }
                    Some(GlobalSearchPhase::Reduce) => {
                        // Convert roles to user
                        messages
                            .into_iter()
                            .map(|map| {
                                map.into_iter()
                                    .map(|(key, value)| {
                                        if key == "role" {
                                            return (key, "user".to_string());
                                        }
                                        (key, value)
                                    })
                                    .collect()
                            })
                            .collect()
                    }
                    _ => messages,
                };

                json!(messages)
            }
        };

        let payload = json!({
            "model": self.model_type,
            "messages": messages_json,
            "stream": false,
        });

        let response = client.post(chat_url).json(&payload).send().await?;
        let response = response.json::<OllamaResponse>().await?;

        Ok(response.message.content)
    }
}
