use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shinkai_graphrag::llm::base::{BaseLLM, BaseLLMCallback, BaseTextEmbedding, LLMParams, MessageType};

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaChatMessage,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OllamaChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaEmbeddingResponse {
    pub model: String,
    pub embeddings: Vec<Vec<f32>>,
}

pub struct OllamaChat {
    base_url: String,
    model: String,
}

impl OllamaChat {
    pub fn new(base_url: &str, model: &str) -> Self {
        OllamaChat {
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl BaseLLM for OllamaChat {
    async fn agenerate(
        &self,
        messages: MessageType,
        _streaming: bool,
        _callbacks: Option<Vec<BaseLLMCallback>>,
        llm_params: LLMParams,
    ) -> anyhow::Result<String> {
        let client = Client::new();
        let chat_url = format!("{}{}", &self.base_url, "/api/chat");

        let messages_json = match messages {
            MessageType::String(message) => json![message],
            MessageType::Strings(messages) => json!(messages),
            MessageType::Dictionary(messages) => json!(messages),
        };

        let payload = json!({
            "model": self.model,
            "messages": messages_json,
            "options": {
                "num_ctx": llm_params.max_tokens,
                "temperature": llm_params.temperature,
            },
            "stream": false,
        });

        let response = client.post(chat_url).json(&payload).send().await?;
        let response = response.json::<OllamaChatResponse>().await?;

        Ok(response.message.content)
    }
}

pub struct OllamaEmbedding {
    base_url: String,
    model: String,
}

impl OllamaEmbedding {
    pub fn new(base_url: &str, model: &str) -> Self {
        OllamaEmbedding {
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl BaseTextEmbedding for OllamaEmbedding {
    async fn aembed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let client = Client::new();
        let embedding_url = format!("{}{}", &self.base_url, "/api/embedding");

        let payload = json!({
            "model": self.model,
            "input": text,
        });

        let response = client.post(embedding_url).json(&payload).send().await?;
        let response = response.json::<OllamaEmbeddingResponse>().await?;

        Ok(response.embeddings.first().cloned().unwrap_or_default())
    }
}
