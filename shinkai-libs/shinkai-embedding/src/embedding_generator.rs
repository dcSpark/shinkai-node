use crate::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use crate::shinkai_embedding_errors::ShinkaiEmbeddingError;
use async_trait::async_trait;

use lazy_static::lazy_static;

use reqwest::blocking::Client;

use reqwest::Client as AsyncClient;
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// TODO: remove duplicate methods
// TODO: remove blocking / non-blocking methods

lazy_static! {
    pub static ref DEFAULT_EMBEDDINGS_SERVER_URL: &'static str = "https://internal.shinkai.com/x-embed-api/";
    pub static ref DEFAULT_EMBEDDINGS_LOCAL_URL: &'static str = "http://localhost:11434/";
}

/// A trait for types that can generate embeddings from text.
#[async_trait]
pub trait EmbeddingGenerator: Sync + Send {
    fn model_type(&self) -> EmbeddingModelType;
    fn set_model_type(&mut self, model_type: EmbeddingModelType);
    fn box_clone(&self) -> Box<dyn EmbeddingGenerator>;

    /// Generates an embedding from the given input string, and assigns the
    /// provided id.
    fn generate_embedding_blocking(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError>;

    /// Generate an Embedding for an input string, sets id to a default value
    /// of empty string.
    fn generate_embedding_default_blocking(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
        self.generate_embedding_blocking(input_string)
    }

    /// Generates embeddings from the given list of input strings and ids.
    fn generate_embeddings_blocking(&self, input_strings: &Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError>;

    /// Generate Embeddings for a list of input strings, sets ids to default.
    fn generate_embeddings_blocking_default(
        &self,
        input_strings: &Vec<String>,
    ) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
        self.generate_embeddings_blocking(input_strings)
    }

    /// Generates an embedding from the given input string, and assigns the
    /// provided id.
    async fn generate_embedding(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError>;

    /// Generate an Embedding for an input string, sets id to a default value
    /// of empty string.
    async fn generate_embedding_default(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
        self.generate_embedding(input_string).await
    }
    // ### TODO: remove all these duplicate methods

    /// Generates embeddings from the given list of input strings and ids.
    async fn generate_embeddings(&self, input_strings: &Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError>;

    /// Generate Embeddings for a list of input strings, sets ids to default
    async fn generate_embeddings_default(&self, input_strings: &Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
        self.generate_embeddings(input_strings).await
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]

pub struct RemoteEmbeddingGenerator {
    pub model_type: EmbeddingModelType,
    pub api_url: String,
    pub api_key: Option<String>,
}

#[async_trait]
impl EmbeddingGenerator for RemoteEmbeddingGenerator {
    /// Clones self and wraps it in a Box
    fn box_clone(&self) -> Box<dyn EmbeddingGenerator> {
        Box::new(self.clone())
    }

    /// Generate Embeddings for an input list of strings by using the external API.
    /// This method batch generates whenever possible to increase speed.
    /// Note this method is blocking.
    fn generate_embeddings_blocking(&self, input_strings: &Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();

        match self.model_type {
            EmbeddingModelType::OllamaTextEmbeddingsInference(_) => {
                let mut embeddings = Vec::new();
                for input_string in input_strings.iter() {
                    let embedding = self.generate_embedding_ollama_blocking(input_string)?;
                    embeddings.push(embedding);
                }
                Ok(embeddings)
            }
        }
    }

    /// Generate an Embedding for an input string by using the external API.
    /// Note this method is blocking.
    fn generate_embedding_blocking(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
        let input_strings = [input_string.to_string()];
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();

        let results = self.generate_embeddings_blocking(&input_strings)?;
        if results.is_empty() {
            Err(ShinkaiEmbeddingError::FailedEmbeddingGeneration(
                "No results returned from the embedding generation".to_string(),
            ))
        } else {
            Ok(results[0].clone())
        }
    }

    /// Generate an Embedding for an input string by using the external API.
    /// This method batch generates whenever possible to increase speed.
    async fn generate_embeddings(&self, input_strings: &Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();

        match self.model_type.clone() {
            EmbeddingModelType::OllamaTextEmbeddingsInference(model) => {
                let mut embeddings = Vec::new();
                for input_string in input_strings.iter() {
                    let embedding = self
                        .generate_embedding_ollama(input_string.clone(), model.to_string())
                        .await?;
                    embeddings.push(embedding);
                }
                Ok(embeddings)
            }
        }
    }

    /// Generate an Embedding for an input string by using the external API.
    async fn generate_embedding(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
        let input_strings = [input_string.to_string()];
        let input_strings: Vec<String> = input_strings
            .iter()
            .map(|s| s.chars().take(self.model_type.max_input_token_count()).collect())
            .collect();

        let results = self.generate_embeddings(&input_strings).await?;
        if results.is_empty() {
            Err(ShinkaiEmbeddingError::FailedEmbeddingGeneration(
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
    /// Create a RemoteEmbeddingGenerator that uses the default model and server
    pub fn new_default_local() -> RemoteEmbeddingGenerator {
        let model_architecture =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);
        RemoteEmbeddingGenerator {
            model_type: model_architecture,
            api_url: DEFAULT_EMBEDDINGS_LOCAL_URL.to_string(),
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

    /// Generates embeddings using Hugging Face's Text Embedding Interface server
    /// pub async fn generate_embedding_open_ai(&self, input_string: &str, id: &str) -> Result<Embedding, VRError> {
    pub async fn generate_embedding_ollama(
        &self,
        input_string: String,
        model: String,
    ) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
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
                            return Ok(embedding_response.embedding);
                        }
                        Err(err) => {
                            return Err(ShinkaiEmbeddingError::RequestFailed(format!(
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
                        return Err(ShinkaiEmbeddingError::RequestFailed(format!(
                            "HTTP request failed after multiple recursive iterations shortening input. Status: {}",
                            response.status()
                        )));
                    }
                    continue;
                }
                Ok(response) => {
                    return Err(ShinkaiEmbeddingError::RequestFailed(format!(
                        "HTTP request failed with status: {}",
                        response.status()
                    )));
                }
                Err(err) => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        continue;
                    } else {
                        return Err(ShinkaiEmbeddingError::RequestFailed(format!(
                            "HTTP request failed after {} retries: {}",
                            max_retries, err
                        )));
                    }
                }
            }
        }
    }

    /// Generate an Embedding for an input string by using the external Ollama API.
    fn generate_embedding_ollama_blocking(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
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
            ShinkaiEmbeddingError::RequestFailed(format!("HTTP request failed: {}", err))
        })?;

        // Check if the response is successful
        if response.status().is_success() {
            let embedding_response: OllamaEmbeddingsResponse = response.json().map_err(|err| {
                ShinkaiEmbeddingError::RequestFailed(format!("Failed to deserialize response JSON: {}", err))
            })?;
            Ok(embedding_response.embedding)
        } else {
            Err(ShinkaiEmbeddingError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }

    /// Generates embeddings using Hugging Face's Text Embedding Interface server
    pub async fn generate_embedding_tei(&self, input_strings: Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
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
                            return Ok(embedding_response);
                        }
                        Err(err) => {
                            return Err(ShinkaiEmbeddingError::RequestFailed(format!(
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
                        return Err(ShinkaiEmbeddingError::RequestFailed(format!(
                            "HTTP request failed after multiple recursive iterations shortening input. Status: {}",
                            response.status()
                        )));
                    }
                    continue;
                }
                Ok(response) => {
                    return Err(ShinkaiEmbeddingError::RequestFailed(format!(
                        "HTTP request failed with status: {}",
                        response.status()
                    )));
                }
                Err(err) => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        continue;
                    } else {
                        return Err(ShinkaiEmbeddingError::RequestFailed(format!(
                            "HTTP request failed after {} retries: {}",
                            max_retries, err
                        )));
                    }
                }
            }
        }
    }

    /// Generates embeddings using a Hugging Face Text Embeddings Inference server
    fn generate_embedding_tei_blocking(&self, input_strings: Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
        // Prepare the request body
        let request_body = EmbeddingArrayRequestBody {
            inputs: input_strings.iter().map(|s| s.to_string()).collect(),
        };

        // Create the HTTP client with a custom timeout
        let timeout = Duration::from_secs(60); // Set the desired timeout duration
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| ShinkaiEmbeddingError::RequestFailed(format!("Failed to create HTTP client: {}", err)))?;

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
                None => {
                    return Err(ShinkaiEmbeddingError::RequestFailed(
                        "Failed to clone request for retry".into(),
                    ))
                }
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
                        return Err(ShinkaiEmbeddingError::RequestFailed(format!(
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
                Ok(embedding_response) => Ok(embedding_response),
                Err(err) => Err(ShinkaiEmbeddingError::RequestFailed(format!(
                    "Failed to deserialize response JSON: {}",
                    err
                ))),
            }
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(ShinkaiEmbeddingError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }

    /// Generate an Embedding for an input string by using the external OpenAI-matching API.
    pub async fn generate_embedding_open_ai(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
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
            ShinkaiEmbeddingError::RequestFailed(format!("HTTP request failed: {}", err))
        })?;

        // Check if the response is successful
        if response.status().is_success() {
            // Deserialize the response JSON into a struct (assuming you have an
            // EmbeddingResponse struct)
            let embedding_response: EmbeddingResponse = response.json().await.map_err(|err| {
                ShinkaiEmbeddingError::RequestFailed(format!("Failed to deserialize response JSON: {}", err))
            })?;

            // Use the response to create an Embedding instance
            Ok(embedding_response.data[0].embedding.clone())
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(ShinkaiEmbeddingError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }

    /// Generate an Embedding for an input string by using the external OpenAI-matching API.
    fn generate_embedding_open_ai_blocking(&self, input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
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
            ShinkaiEmbeddingError::RequestFailed(format!("HTTP request failed: {}", err))
        })?;

        // Check if the response is successful
        if response.status().is_success() {
            // Deserialize the response JSON into a struct (assuming you have an
            // EmbeddingResponse struct)
            let embedding_response: EmbeddingResponse = response.json().map_err(|err| {
                ShinkaiEmbeddingError::RequestFailed(format!("Failed to deserialize response JSON: {}", err))
            })?;

            // Use the response to create an Embedding instance
            Ok(embedding_response.data[0].embedding.clone())
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(ShinkaiEmbeddingError::RequestFailed(format!(
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
