use crate::embeddings::Embedding;
use crate::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference, TextEmbeddingsInference};
use crate::resource_errors::VRError;
use async_trait::async_trait;

use lazy_static::lazy_static;
#[cfg(feature = "desktop-only")]
use reqwest::blocking::Client;
#[cfg(feature = "desktop-only")]
use reqwest::Client as AsyncClient;
use serde::{Deserialize, Serialize};
use reqwest::ClientBuilder;
use std::time::Duration;

lazy_static! {
    pub static ref DEFAULT_EMBEDDINGS_SERVER_URL: &'static str = "https://internal.shinkai.com/x-embed-api/";
}

/// A trait for types that can generate embeddings from text.
#[async_trait]
pub trait EmbeddingGenerator: Sync + Send {
    fn model_type(&self) -> EmbeddingModelType;
    fn set_model_type(&mut self, model_type: EmbeddingModelType);
    fn box_clone(&self) -> Box<dyn EmbeddingGenerator>;

    /// Generates an embedding from the given input string, and assigns the
    /// provided id.
    fn generate_embedding_blocking(&self, input_string: &str, id: &str) -> Result<Embedding, VRError>;

    /// Generate an Embedding for an input string, sets id to a default value
    /// of empty string.
    fn generate_embedding_default_blocking(&self, input_string: &str) -> Result<Embedding, VRError> {
        self.generate_embedding_blocking(input_string, "")
    }

    /// Generates embeddings from the given list of input strings and ids.
    fn generate_embeddings_blocking(
        &self,
        input_strings: &Vec<String>,
        ids: &Vec<String>,
    ) -> Result<Vec<Embedding>, VRError>;

    /// Generate Embeddings for a list of input strings, sets ids to default.
    fn generate_embeddings_blocking_default(&self, input_strings: &Vec<String>) -> Result<Vec<Embedding>, VRError> {
        let ids: Vec<String> = vec!["".to_string(); input_strings.len()];
        self.generate_embeddings_blocking(input_strings, &ids)
    }

    /// Generates an embedding from the given input string, and assigns the
    /// provided id.
    async fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, VRError>;

    /// Generate an Embedding for an input string, sets id to a default value
    /// of empty string.
    async fn generate_embedding_default(&self, input_string: &str) -> Result<Embedding, VRError> {
        self.generate_embedding(input_string, "").await
    }

    /// Generates embeddings from the given list of input strings and ids.
    async fn generate_embeddings(
        &self,
        input_strings: &Vec<String>,
        ids: &Vec<String>,
    ) -> Result<Vec<Embedding>, VRError>;

    /// Generate Embeddings for a list of input strings, sets ids to default
    async fn generate_embeddings_default(&self, input_strings: &Vec<String>) -> Result<Vec<Embedding>, VRError> {
        let ids: Vec<String> = vec!["".to_string(); input_strings.len()];
        self.generate_embeddings(input_strings, &ids).await
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(feature = "desktop-only")]
pub struct RemoteEmbeddingGenerator {
    pub model_type: EmbeddingModelType,
    pub api_url: String,
    pub api_key: Option<String>,
}

#[cfg(feature = "desktop-only")]
#[async_trait]
impl EmbeddingGenerator for RemoteEmbeddingGenerator {
    /// Clones self and wraps it in a Box
    fn box_clone(&self) -> Box<dyn EmbeddingGenerator> {
        Box::new(self.clone())
    }

    #[cfg(feature = "desktop-only")]
    /// Generate Embeddings for an input list of strings by using the external API.
    /// This method batch generates whenever possible to increase speed.
    /// Note this method is blocking.
    fn generate_embeddings_blocking(
        &self,
        input_strings: &Vec<String>,
        ids: &Vec<String>,
    ) -> Result<Vec<Embedding>, VRError> {
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();

        match self.model_type {
            EmbeddingModelType::TextEmbeddingsInference(_) => {
                self.generate_embedding_tei_blocking(input_strings.clone(), ids.clone())
            }
            EmbeddingModelType::OllamaTextEmbeddingsInference(_) => {
                let mut embeddings = Vec::new();
                for (input_string, id) in input_strings.iter().zip(ids) {
                    let embedding = self.generate_embedding_ollama_blocking(input_string, id)?;
                    embeddings.push(embedding);
                }
                Ok(embeddings)
            }
            _ => {
                let mut embeddings = Vec::new();
                for (input_string, id) in input_strings.iter().zip(ids) {
                    let embedding = self.generate_embedding_open_ai_blocking(input_string, id)?;
                    embeddings.push(embedding);
                }
                Ok(embeddings)
            }
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generate an Embedding for an input string by using the external API.
    /// Note this method is blocking.
    fn generate_embedding_blocking(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
        let input_strings = vec![input_string.to_string()];
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();
        let ids = vec![id.to_string()];

        let results = self.generate_embeddings_blocking(&input_strings, &ids)?;
        if results.is_empty() {
            Err(VRError::FailedEmbeddingGeneration(
                "No results returned from the embedding generation".to_string(),
            ))
        } else {
            Ok(results[0].clone())
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generate an Embedding for an input string by using the external API.
    /// This method batch generates whenever possible to increase speed.
    async fn generate_embeddings(
        &self,
        input_strings: &Vec<String>,
        ids: &Vec<String>,
    ) -> Result<Vec<Embedding>, VRError> {
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();

        match self.model_type.clone() {
            EmbeddingModelType::TextEmbeddingsInference(_) => {
                self.generate_embedding_tei(input_strings.clone(), ids.clone()).await
            }
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => {
                let mut embeddings = Vec::new();
                for (input_string, id) in input_strings.iter().zip(ids) {
                    let embedding = self
                        .generate_embedding_ollama(input_string.clone(), id.clone(), model.to_string())
                        .await?;
                    embeddings.push(embedding);
                }
                Ok(embeddings)
            }
            _ => {
                let mut embeddings = Vec::new();
                for (input_string, id) in input_strings.iter().zip(ids) {
                    let embedding = self.generate_embedding_open_ai(input_string, id).await?;
                    embeddings.push(embedding);
                }
                Ok(embeddings)
            }
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generate an Embedding for an input string by using the external API.
    async fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
        let input_strings = [input_string.to_string()];
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();
        let ids = vec![id.to_string()];

        let results = self.generate_embeddings(&input_strings, &ids).await?;
        if results.is_empty() {
            Err(VRError::FailedEmbeddingGeneration(
                "No results returned from the embedding generation".to_string(),
            ))
        } else {
            Ok(results[0].clone())
        }
    }

    /// Returns the EmbeddingModelType
    fn model_type(&self) -> EmbeddingModelType {
        self.model_type.clone()
    }

    /// Sets the EmbeddingModelType
    fn set_model_type(&mut self, model_type: EmbeddingModelType) {
        self.model_type = model_type
    }
}

#[cfg(feature = "desktop-only")]
impl RemoteEmbeddingGenerator {
    /// Create a RemoteEmbeddingGenerator
    pub fn new(model_type: EmbeddingModelType, api_url: &str, api_key: Option<String>) -> RemoteEmbeddingGenerator {
        RemoteEmbeddingGenerator {
            model_type,
            api_url: api_url.to_string(),
            api_key,
        }
    }

    /// Create a RemoteEmbeddingGenerator that uses the default model and server
    pub fn new_default() -> RemoteEmbeddingGenerator {
        let model_architecture =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);
        RemoteEmbeddingGenerator {
            model_type: model_architecture,
            api_url: DEFAULT_EMBEDDINGS_SERVER_URL.to_string(),
            api_key: None,
        }
    }

    /// String of the main endpoint url for generating embeddings via
    /// Hugging face's Text Embedding Interface server
    fn tei_endpoint_url(&self) -> String {
        if self.api_url.ends_with('/') {
            format!("{}embed", self.api_url)
        } else {
            format!("{}/embed", self.api_url)
        }
    }

    /// String of the main endpoint url for generating embeddings via
    /// Ollama Text Embedding Interface server
    fn ollama_endpoint_url(&self) -> String {
        if self.api_url.ends_with('/') {
            format!("{}api/embeddings", self.api_url)
        } else {
            format!("{}/api/embeddings", self.api_url)
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generates embeddings using Hugging Face's Text Embedding Interface server
    /// pub async fn generate_embedding_open_ai(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
    pub async fn generate_embedding_ollama(
        &self,
        input_string: String,
        id: String,
        model: String,
    ) -> Result<Embedding, VRError> {
        let max_retries = 3;
        let mut retry_count = 0;
        let mut shortening_retry = 0;
        let mut input_string = input_string.clone();

        loop {
            // Prepare the request body
            let request_body = OllamaEmbeddingsRequestBody {
                model: model.clone(),
                prompt: input_string.clone(),
            };

            // Create the HTTP client with a custom timeout
            let timeout = Duration::from_secs(60);
            let client = ClientBuilder::new().timeout(timeout).build()?;

            // Build the request
            let mut request = client
                .post(self.ollama_endpoint_url().to_string())
                .header("Content-Type", "application/json")
                .json(&request_body);

            // Add the API key to the header if it's available
            if let Some(api_key) = &self.api_key {
                request = request.header("Authorization", format!("Bearer {}", api_key));
            }

            // Send the request
            let response = request.send().await;

            match response {
                Ok(response) if response.status().is_success() => {
                    let embedding_response: Result<OllamaEmbeddingsResponse, _> =
                        response.json::<OllamaEmbeddingsResponse>().await;
                    match embedding_response {
                        Ok(embedding_response) => {
                            let embedding = Embedding {
                                id: String::from(id),
                                vector: embedding_response.embedding,
                            };
                            return Ok(embedding);
                        }
                        Err(err) => {
                            return Err(VRError::RequestFailed(format!(
                                "Failed to deserialize response JSON: {}",
                                err
                            )));
                        }
                    }
                }
                Ok(response) if response.status() == reqwest::StatusCode::PAYLOAD_TOO_LARGE => {
                    // Calculate the maximum size allowed based on the number of retries
                    let reduction_step = if shortening_retry > 1 {
                        100 * shortening_retry
                    } else {
                        50
                    };
                    let shortened_max_size = input_string.len().saturating_sub(reduction_step).max(5);
                    input_string = input_string.chars().take(shortened_max_size).collect();

                    retry_count = 0;
                    shortening_retry += 1;
                    if shortening_retry > 10 {
                        return Err(VRError::RequestFailed(format!(
                            "HTTP request failed after multiple recursive iterations shortening input. Status: {}",
                            response.status()
                        )));
                    }
                    continue;
                }
                Ok(response) => {
                    return Err(VRError::RequestFailed(format!(
                        "HTTP request failed with status: {}",
                        response.status()
                    )));
                }
                Err(err) => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        continue;
                    } else {
                        return Err(VRError::RequestFailed(format!(
                            "HTTP request failed after {} retries: {}",
                            max_retries, err
                        )));
                    }
                }
            }
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generate an Embedding for an input string by using the external Ollama API.
    fn generate_embedding_ollama_blocking(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
        // Prepare the request body
        let request_body = OllamaEmbeddingsRequestBody {
            model: self.model_type.to_string(),
            prompt: String::from(input_string),
        };

        // Create the HTTP client
        let client = Client::new();

        // Build the request
        let mut request = client
            .post(&format!("{}", self.ollama_endpoint_url()))
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Add the API key to the header if it's available
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        // Send the request and check for errors
        let response = request.send().map_err(|err| {
            // Handle any HTTP client errors here (e.g., request creation failure)
            VRError::RequestFailed(format!("HTTP request failed: {}", err))
        })?;

        // Check if the response is successful
        if response.status().is_success() {
            // Deserialize the response JSON into a struct
            let embedding_response: OllamaEmbeddingsResponse = response
                .json()
                .map_err(|err| VRError::RequestFailed(format!("Failed to deserialize response JSON: {}", err)))?;

            // Use the response to create an Embedding instance
            Ok(Embedding {
                id: String::from(id),
                vector: embedding_response.embedding,
            })
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(VRError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generates embeddings using Hugging Face's Text Embedding Interface server
    pub async fn generate_embedding_tei(
        &self,
        input_strings: Vec<String>,
        ids: Vec<String>,
    ) -> Result<Vec<Embedding>, VRError> {
        let max_retries = 3;
        let mut retry_count = 0;
        let mut shortening_retry = 0;
        let mut current_input_strings = input_strings.clone();

        loop {
            // Prepare the request body
            let request_body = EmbeddingArrayRequestBody {
                inputs: current_input_strings.iter().map(|s| s.to_string()).collect(),
            };

            // Create the HTTP client with a custom timeout
            let timeout = Duration::from_secs(60);
            let client = ClientBuilder::new().timeout(timeout).build()?;

            // Build the request
            let mut request = client
                .post(self.tei_endpoint_url().to_string())
                .header("Content-Type", "application/json")
                .json(&request_body);

            // Add the API key to the header if it's available
            if let Some(api_key) = &self.api_key {
                request = request.header("Authorization", format!("Bearer {}", api_key));
            }

            // Send the request
            let response = request.send().await;

            match response {
                Ok(response) if response.status().is_success() => {
                    let embedding_response: Result<Vec<Vec<f32>>, _> = response.json::<Vec<Vec<f32>>>().await;
                    match embedding_response {
                        Ok(embedding_response) => {
                            // Create a Vec<Embedding> by iterating over ids and embeddings
                            let embeddings: Result<Vec<Embedding>, _> = ids
                                .iter()
                                .zip(embedding_response.into_iter())
                                .map(|(id, embedding)| {
                                    Ok(Embedding {
                                        id: id.clone(),
                                        vector: embedding,
                                    })
                                })
                                .collect();
                            return embeddings;
                        }
                        Err(err) => {
                            return Err(VRError::RequestFailed(format!(
                                "Failed to deserialize response JSON: {}",
                                err
                            )));
                        }
                    }
                }
                Ok(response) if response.status() == reqwest::StatusCode::PAYLOAD_TOO_LARGE => {
                    let max_size = current_input_strings.iter().map(|s| s.len()).max().unwrap_or(0);
                    // Increase the number of characters removed based on the number of retries
                    let reduction_step = if shortening_retry > 1 {
                        100 * shortening_retry
                    } else {
                        50
                    };
                    let shortened_max_size = max_size.saturating_sub(reduction_step).max(5);
                    current_input_strings = current_input_strings
                        .iter()
                        .map(|s| {
                            if s.len() > shortened_max_size {
                                s.chars().take(shortened_max_size).collect()
                            } else {
                                s.clone()
                            }
                        })
                        .collect();
                    retry_count = 0;
                    shortening_retry += 1;
                    if shortening_retry > 10 {
                        return Err(VRError::RequestFailed(format!(
                            "HTTP request failed after multiple recursive iterations shortening input. Status: {}",
                            response.status()
                        )));
                    }
                    continue;
                }
                Ok(response) => {
                    return Err(VRError::RequestFailed(format!(
                        "HTTP request failed with status: {}",
                        response.status()
                    )));
                }
                Err(err) => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        continue;
                    } else {
                        return Err(VRError::RequestFailed(format!(
                            "HTTP request failed after {} retries: {}",
                            max_retries, err
                        )));
                    }
                }
            }
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generates embeddings using a Hugging Face Text Embeddings Inference server
    fn generate_embedding_tei_blocking(
        &self,
        input_strings: Vec<String>,
        ids: Vec<String>,
    ) -> Result<Vec<Embedding>, VRError> {
        // Prepare the request body
        let request_body = EmbeddingArrayRequestBody {
            inputs: input_strings.iter().map(|s| s.to_string()).collect(),
        };

        // Create the HTTP client with a custom timeout
        let timeout = Duration::from_secs(60); // Set the desired timeout duration
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| VRError::RequestFailed(format!("Failed to create HTTP client: {}", err)))?;

        // Build the request
        let mut request = client
            .post(&format!("{}", self.tei_endpoint_url()))
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Add the API key to the header if it's available
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        // Send the request with retries
        let max_retries = 3;
        let mut retry_count = 0;
        let response = loop {
            let cloned_request = match request.try_clone() {
                Some(req) => req,
                None => return Err(VRError::RequestFailed("Failed to clone request for retry".into())),
            };
            match cloned_request.send() {
                Ok(response) => break response,
                Err(err) => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        eprintln!(
                            "Request failed with error: {}. Retrying ({}/{})...",
                            err, retry_count, max_retries
                        );
                        std::thread::sleep(Duration::from_secs(1)); // Optional: Add a delay between retries
                    } else {
                        return Err(VRError::RequestFailed(format!(
                            "HTTP request failed after {} retries: {}",
                            max_retries, err
                        )));
                    }
                }
            }
        };

        // Check if the response is successful
        if response.status().is_success() {
            let embedding_response: Result<Vec<Vec<f32>>, _> = response.json::<Vec<Vec<f32>>>();

            match embedding_response {
                Ok(embedding_response) => {
                    // Create a Vec<Embedding> by iterating over ids and embeddings
                    let embeddings: Result<Vec<Embedding>, _> = ids
                        .iter()
                        .zip(embedding_response.into_iter())
                        .map(|(id, embedding)| {
                            Ok(Embedding {
                                id: id.clone(),
                                vector: embedding,
                            })
                        })
                        .collect();

                    // Return the embeddings
                    embeddings
                }
                Err(err) => Err(VRError::RequestFailed(format!(
                    "Failed to deserialize response JSON: {}",
                    err
                ))),
            }
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(VRError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generate an Embedding for an input string by using the external OpenAI-matching API.
    pub async fn generate_embedding_open_ai(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
        // Prepare the request body
        let request_body = EmbeddingRequestBody {
            input: String::from(input_string),
            model: self.model_type().to_string(),
        };

        // Create the HTTP client
        let client = AsyncClient::new();

        // Build the request
        let mut request = client
            .post(self.api_url.to_string())
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Add the API key to the header if it's available
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        // Send the request and check for errors
        let response = request.send().await.map_err(|err| {
            // Handle any HTTP client errors here (e.g., request creation failure)
            VRError::RequestFailed(format!("HTTP request failed: {}", err))
        })?;

        // Check if the response is successful
        if response.status().is_success() {
            // Deserialize the response JSON into a struct (assuming you have an
            // EmbeddingResponse struct)
            let embedding_response: EmbeddingResponse = response
                .json()
                .await
                .map_err(|err| VRError::RequestFailed(format!("Failed to deserialize response JSON: {}", err)))?;

            // Use the response to create an Embedding instance
            Ok(Embedding {
                id: String::from(id),
                vector: embedding_response.data[0].embedding.clone(),
            })
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(VRError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }

    #[cfg(feature = "desktop-only")]
    /// Generate an Embedding for an input string by using the external OpenAI-matching API.
    fn generate_embedding_open_ai_blocking(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
        // Prepare the request body
        let request_body = EmbeddingRequestBody {
            input: String::from(input_string),
            model: self.model_type().to_string(),
        };

        // Create the HTTP client
        let client = Client::new();

        // Build the request
        let mut request = client
            .post(&format!("{}", self.api_url))
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Add the API key to the header if it's available
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        // Send the request and check for errors
        let response = request.send().map_err(|err| {
            // Handle any HTTP client errors here (e.g., request creation failure)
            VRError::RequestFailed(format!("HTTP request failed: {}", err))
        })?;

        // Check if the response is successful
        if response.status().is_success() {
            // Deserialize the response JSON into a struct (assuming you have an
            // EmbeddingResponse struct)
            let embedding_response: EmbeddingResponse = response
                .json()
                .map_err(|err| VRError::RequestFailed(format!("Failed to deserialize response JSON: {}", err)))?;

            // Use the response to create an Embedding instance
            Ok(Embedding {
                id: String::from(id),
                vector: embedding_response.data[0].embedding.clone(),
            })
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(VRError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }
}

#[derive(Serialize)]
#[allow(dead_code)]
struct EmbeddingRequestBody {
    input: String,
    model: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EmbeddingResponseData {
    embedding: Vec<f32>,
    index: usize,
    object: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EmbeddingResponse {
    object: String,
    model: String,
    data: Vec<EmbeddingResponseData>,
    usage: serde_json::Value, // or define a separate struct for this if you need to use these values
}

#[derive(Serialize)]
#[allow(dead_code)]
struct EmbeddingArrayRequestBody {
    inputs: Vec<String>,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct OllamaEmbeddingsRequestBody {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaEmbeddingsResponse {
    embedding: Vec<f32>,
}

// /// An Embedding Generator for Local LLMs, such as LLama, Bloom, Pythia, etc.
// pub struct LocalEmbeddingGenerator {
//     model: Box<dyn Model>,
//     model_type: EmbeddingModelType,
// }

// impl EmbeddingGenerator for LocalEmbeddingGenerator {
//     /// Generate an Embedding for an input string.
//     /// - `id`: The id to be associated with the embeddings.
//     fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
//         let mut session = self.model.start_session(Default::default());
//         let mut output_request = llm::OutputRequest {
//             all_logits: None,
//             embeddings: Some(Vec::new()),
//         };
//         let vocab = self.model.tokenizer();
//         let beginning_of_sentence = true;

//         let tokens = vocab
//             .tokenize(input_string, beginning_of_sentence)
//             .map_err(|_| VRError::FailedEmbeddingGeneration)?;

//         let query_token_ids = tokens.iter().map(|(_, tok)| *tok).collect::<Vec<_>>();

//         self.model.evaluate(&mut session, &query_token_ids, &mut output_request);

//         let vector = output_request
//             .embeddings
//             .ok_or_else(|| VRError::FailedEmbeddingGeneration)?;

//         Ok(Embedding {
//             id: String::from(id),
//             vector,
//         })
//     }

//     fn model_type(&self) -> EmbeddingModelType {
//         self.model_type.clone()
//     }
// }

// impl LocalEmbeddingGenerator {
//     /// Create a new LocalEmbeddingGenerator with a specified model.
//     pub fn new(model: Box<dyn Model>, model_architecture: ModelArchitecture) -> Self {
//         Self {
//             model,
//             model_type: EmbeddingModelType::LocalModel(LocalModel::from_model_architecture(model_architecture)),
//         }
//     }

//     /// Create a new LocalEmbeddingGenerator that uses the default model.
//     /// Intended to be used just for testing.
//     pub fn new_default() -> Self {

//         let DEFAULT_LOCAL_MODEL_PATH: &'static str = "models/pythia-160m-q4_0.bin";
//         let model_architecture = llm::ModelArchitecture::GptNeoX;
//         let model = llm::load_dynamic(
//             Some(model_architecture),
//             std::path::Path::new(&*DEFAULT_LOCAL_MODEL_PATH),
//             llm::TokenizerSource::Embedded,
//             Default::default(),
//             load_callback,
//         )
//         .unwrap_or_else(|err| panic!("Failed to load model: {}", err));
//         LocalEmbeddingGenerator::new(model, model_architecture)
//     }
// }
