use reqwest::Client;
use rusqlite::Result;
use serde::{Deserialize, Serialize};
use shinkai_embedding::model_type::EmbeddingModelType;

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

    pub async fn request_embeddings(&self, prompt: &str) -> Result<Vec<f32>, rusqlite::Error> {
        let model_str = match &self.model_type {
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => model.to_string(),
            _ => {
                println!("Unsupported embedding model type: {:?}", self.model_type);
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

        let response = self.client.post(&full_url).json(&request_body).send().await;

        match response {
            Ok(response) => {
                if !response.status().is_success() {
                    println!("Failed to send request to embedding API: {}", response.status());
                    return Err(rusqlite::Error::InvalidQuery);
                }
                let ollama_response = response.json::<OllamaResponse>().await.map_err(|e| {
                    println!("Failed to convert response to OllamaResponse: {}", e);
                    rusqlite::Error::InvalidQuery
                })?;

                Ok(ollama_response.embedding)
            }
            Err(e) => {
                println!("Failed to send request to embedding API: {}", e);
                return Err(rusqlite::Error::InvalidParameterName(e.to_string()));
            }
        }
    }
}
