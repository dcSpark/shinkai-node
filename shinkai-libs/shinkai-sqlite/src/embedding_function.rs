use reqwest::Client;
use rusqlite::Result;
use serde::{Deserialize, Serialize};
use shinkai_vector_resources::model_type::EmbeddingModelType;

#[derive(Serialize, Deserialize)]
struct OllamaResponse {
    embedding: Vec<f32>,
}

pub struct EmbeddingFunction {
    client: Client,
    api_url: String,
    model_type: EmbeddingModelType,
}

impl EmbeddingFunction {
    pub fn new(api_url: &str, model_type: EmbeddingModelType) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.to_string(),
            model_type,
        }
    }

    pub async fn request_embeddings(&self, prompt: &str) -> Result<Vec<f32>> {
        let model_str = match &self.model_type {
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => model.to_string(),
            _ => {
                return Err(rusqlite::Error::InvalidQuery);
            }
        };

        let max_tokens = self.model_type.max_input_token_count();
        let truncated_prompt = if prompt.len() > max_tokens {
            &prompt[..max_tokens]
        } else {
            prompt
        };

        let request_body = serde_json::json!({
            "model": model_str,
            "prompt": truncated_prompt
        });

        let full_url = if self.api_url.ends_with('/') {
            format!("{}api/embeddings", self.api_url)
        } else {
            format!("{}/api/embeddings", self.api_url)
        };

        let response = self
            .client
            .post(&full_url)
            .json(&request_body)
            .send()
            .await
            .map_err(|_e| rusqlite::Error::InvalidQuery)?
            .json::<OllamaResponse>()
            .await
            .map_err(|_e| rusqlite::Error::InvalidQuery)?;

        Ok(response.embedding)
    }
} 