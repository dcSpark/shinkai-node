use arrow::array::ArrayData;
use arrow_array::{Array, FixedSizeListArray, Float32Array, StringArray};
use arrow_schema::{DataType, Field};
use lancedb::embeddings::EmbeddingFunction;
use lancedb::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shinkai_vector_resources::model_type::EmbeddingModelType;
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct OllamaEmbeddingFunction {
    client: Client,
    api_url: String,
    model_type: EmbeddingModelType,
}

impl OllamaEmbeddingFunction {
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
                return Err(lancedb::Error::Other {
                    message: "Invalid model type for Ollama".to_string(),
                    source: None,
                })
            }
        };

        let request_body = serde_json::json!({
            "model": model_str,
            "prompt": prompt
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
            .map_err(|e| lancedb::Error::Http { message: e.to_string() })?
            .json::<OllamaResponse>()
            .await
            .map_err(|e| lancedb::Error::Http { message: e.to_string() })?;

        Ok(response.embedding)
    }
}

#[derive(Serialize, Deserialize)]
struct OllamaResponse {
    embedding: Vec<f32>,
}

impl EmbeddingFunction for OllamaEmbeddingFunction {
    fn name(&self) -> &str {
        "ollama"
    }

    fn source_type(&self) -> Result<Cow<DataType>> {
        Ok(Cow::Owned(DataType::Utf8))
    }

    fn dest_type(&self) -> Result<Cow<DataType>> {
        let n_dims = 384;
        Ok(Cow::Owned(DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, false)),
            n_dims,
        )))
    }

    fn compute_source_embeddings(&self, source: Arc<dyn Array>) -> Result<Arc<dyn Array>> {
        let source_str = source.as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
        let prompt = source_str.value(0); // Assuming single value for simplicity

        let embedding = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.request_embeddings(prompt))?;

        let array = Float32Array::from(embedding);
        Ok(Arc::new(array) as Arc<dyn Array>)
    }

    fn compute_query_embeddings(&self, input: Arc<dyn Array>) -> Result<Arc<dyn Array>> {
        let input_str = input.as_any().downcast_ref::<StringArray>().unwrap();
        let mut embeddings = Vec::new();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        for i in 0..input_str.len() {
            let prompt = input_str.value(i);
            let embedding = runtime.block_on(self.request_embeddings(prompt))?;
            embeddings.push(embedding);
        }

        // Flatten the embeddings and create a FixedSizeListArray
        let flattened_embeddings: Vec<f32> = embeddings.into_iter().flatten().collect();
        let array_data = ArrayData::builder(DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, false)),
            768,
        ))
        .len(input_str.len())
        .add_child_data(Float32Array::from(flattened_embeddings).into_data())
        .build()?;

        Ok(Arc::new(FixedSizeListArray::from(array_data)) as Arc<dyn Array>)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tokio::runtime::Runtime;

//     #[test]
//     fn test_request_embeddings() {
//         let api_url = "http://localhost:11434/api/embeddings";
//         let embedding_function = OllamaEmbeddingFunction::new("test", api_url);

//         let rt = Runtime::new().unwrap();
//         let result = rt.block_on(embedding_function.request_embeddings("Velociraptor"));

//         assert!(result.is_ok());
//         let embedding = result.unwrap();
//         assert_eq!(embedding.len(), 384); // Assuming the embedding size is 384
//     }
// }
