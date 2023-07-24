use crate::resources::embeddings::*;
use crate::resources::local_ai::DEFAULT_LOCAL_AI_PORT;
use crate::resources::model_type::*;
use crate::resources::resource_errors::*;
use lazy_static::lazy_static;
use llm::load_progress_callback_stdout as load_callback;
use llm::Model;
use llm::ModelArchitecture;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref DEFAULT_LOCAL_MODEL_PATH: &'static str = "models/pythia-160m-q4_0.bin";
}

/// A trait for types that can generate embeddings from text.
pub trait EmbeddingGenerator {
    // Returns the embedding model type
    fn model_type(&self) -> EmbeddingModelType;

    /// Generates an embedding from the given input string, and assigns the
    /// provided id.
    fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, ResourceError>;

    /// Generate an Embedding for an input string, sets id to a default value
    /// of empty string.
    ///
    /// # Parameters
    /// - `input_string`: The input string for which embeddings are generated.
    fn generate_embedding_default(&self, input_string: &str) -> Result<Embedding, ResourceError> {
        self.generate_embedding(input_string, "")
    }

    /// Generates embeddings from the given list of input strings,  and assigns
    /// the provided ids.
    fn generate_embeddings(&self, input_strings: &[&str], ids: &[&str]) -> Result<Vec<Embedding>, ResourceError> {
        input_strings
            .iter()
            .zip(ids)
            .map(|(input, id)| self.generate_embedding(input, id))
            .collect()
    }

    /// Generate Embeddings for a list of input strings, sets ids to a default
    fn generate_embeddings_default(&self, input_strings: &[&str]) -> Result<Vec<Embedding>, ResourceError> {
        input_strings
            .iter()
            .map(|input| self.generate_embedding_default(input))
            .collect()
    }
}

#[derive(Serialize)]
struct EmbeddingRequestBody {
    input: String,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponseData {
    embedding: Vec<f32>,
    index: usize,
    object: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    object: String,
    model: String,
    data: Vec<EmbeddingResponseData>,
    usage: serde_json::Value, // or define a separate struct for this if you need to use these values
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteEmbeddingGenerator {
    model_type: EmbeddingModelType,
    api_url: String,
    api_key: Option<String>,
}

impl EmbeddingGenerator for RemoteEmbeddingGenerator {
    /// Generate an Embedding for an input string by using the external API.
    ///
    /// # Parameters
    /// - `input_string`: The input string for which embeddings are generated.
    /// - `id`: The id to be associated with the embeddings.
    ///
    /// # Returns
    /// An `Embedding` for the input string or an error.
    fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, ResourceError> {
        // Prepare the request body
        let request_body = EmbeddingRequestBody {
            input: String::from(input_string),
            model: self.model_type().to_string(),
        };

        // Create the HTTP client
        let client = Client::new();

        // Build the request
        let request = client
            .post(&format!("{}/v1/embeddings", self.api_url))
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Send the request and check for errors
        let response = request.send().map_err(|err| {
            // Handle any HTTP client errors here (e.g., request creation failure)
            ResourceError::RequestFailed(format!("HTTP request failed: {}", err))
        })?;

        println!("Received response");

        // Check if the response is successful
        if response.status().is_success() {
            println!("Is successful");
            // Deserialize the response JSON into a struct (assuming you have an
            // EmbeddingResponse struct)
            let embedding_response: EmbeddingResponse = response
                .json()
                .map_err(|err| ResourceError::RequestFailed(format!("Failed to deserialize response JSON: {}", err)))?;

            // Use the response to create an Embedding instance
            Ok(Embedding {
                id: String::from(id),
                vector: embedding_response.data[0].embedding.clone(),
            })
        } else {
            // Handle non-successful HTTP responses (e.g., server error)
            Err(ResourceError::RequestFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )))
        }
    }

    fn model_type(&self) -> EmbeddingModelType {
        self.model_type.clone()
    }
}

impl RemoteEmbeddingGenerator {
    /// Create a RemoteEmbeddingGenerator
    pub fn new(model_type: EmbeddingModelType, api_url: &str, api_key: Option<&str>) -> RemoteEmbeddingGenerator {
        RemoteEmbeddingGenerator {
            model_type,
            api_url: api_url.to_string(),
            api_key: api_key.map(|a| a.to_string()),
        }
    }

    /// Create a RemoteEmbeddingGenerator that automatically attempts to connect
    /// to the webserver of a local running instance of LocalAI using the
    /// default set port.
    ///
    /// Expected to have downloaded & be using the AllMiniLML12v2 model.
    pub fn new_default() -> RemoteEmbeddingGenerator {
        let model_architecture = EmbeddingModelType::RemoteModel(RemoteModel::AllMiniLML12v2);
        let url = format!("http://0.0.0.0:{}", DEFAULT_LOCAL_AI_PORT.to_string());
        RemoteEmbeddingGenerator {
            model_type: model_architecture,
            api_url: url,
            api_key: None,
        }
    }
}

/// An Embedding Generator for Local LLMs, such as LLama, Bloom, Pythia, etc.
pub struct LocalEmbeddingGenerator {
    model: Box<dyn Model>,
    model_type: EmbeddingModelType,
}

impl EmbeddingGenerator for LocalEmbeddingGenerator {
    /// Generate an Embedding for an input string.
    ///
    /// # Parameters
    /// - `input_string`: The input string for which embeddings are generated.
    /// - `id`: The id to be associated with the embeddings.
    ///
    /// # Returns
    /// An `Embedding` for the input string or an error.
    fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, ResourceError> {
        let mut session = self.model.start_session(Default::default());
        let mut output_request = llm::OutputRequest {
            all_logits: None,
            embeddings: Some(Vec::new()),
        };
        let vocab = self.model.tokenizer();
        let beginning_of_sentence = true;

        let tokens = vocab
            .tokenize(input_string, beginning_of_sentence)
            .map_err(|_| ResourceError::FailedEmbeddingGeneration)?;

        let query_token_ids = tokens.iter().map(|(_, tok)| *tok).collect::<Vec<_>>();

        self.model.evaluate(&mut session, &query_token_ids, &mut output_request);

        let vector = output_request
            .embeddings
            .ok_or_else(|| ResourceError::FailedEmbeddingGeneration)?;

        Ok(Embedding {
            id: String::from(id),
            vector,
        })
    }

    fn model_type(&self) -> EmbeddingModelType {
        self.model_type.clone()
    }
}

impl LocalEmbeddingGenerator {
    /// Create a new LocalEmbeddingGenerator with a specified model.
    ///
    /// # Parameters
    /// - `model`: The model to be used for generating embeddings.
    ///
    /// # Returns
    /// A new `LocalEmbeddingGenerator` that uses the specified model.
    pub fn new(model: Box<dyn Model>, model_architecture: ModelArchitecture) -> Self {
        Self {
            model,
            model_type: EmbeddingModelType::LocalModel(LocalModel::from_model_architecture(model_architecture)),
        }
    }

    /// Create a new LocalEmbeddingGenerator that uses the default model.
    /// Intended to be used just for testing.
    ///
    /// # Returns
    /// A new `LocalEmbeddingGenerator` that uses the default model.
    ///
    /// # Panics
    /// This function will panic if it fails to load the default model.
    pub fn new_default() -> Self {
        let model_architecture = llm::ModelArchitecture::GptNeoX;
        let model = llm::load_dynamic(
            Some(model_architecture),
            std::path::Path::new(&*DEFAULT_LOCAL_MODEL_PATH),
            llm::TokenizerSource::Embedded,
            Default::default(),
            load_callback,
        )
        .unwrap_or_else(|err| panic!("Failed to load model: {}", err));
        LocalEmbeddingGenerator::new(model, model_architecture)
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_embeddings_generation() {
        let generator = LocalEmbeddingGenerator::new_default();

        let dog_embeddings = generator.generate_embedding("dog", "1").unwrap();
        let cat_embeddings = generator.generate_embedding("cat", "2").unwrap();

        assert_eq!(dog_embeddings, dog_embeddings);
        assert_eq!(cat_embeddings, cat_embeddings);
        assert_ne!(dog_embeddings, cat_embeddings);
    }

    //
    // Commented out because embedding generation is slow
    // and resources tests cover this functionality anyways
    //
    // #[test]
    // fn test_embedding_vector_similarity() {
    //     let generator = LocalEmbeddingGenerator::new_default();

    //     let query = "What can fly in the sky?";
    //     let comparands = vec![
    //         "A golden retriever dog".to_string(),
    //         "A four legged frog".to_string(),
    //         "A plane in the sky".to_string(),
    //     ];

    //     // Generate embeddings for query and comparands
    //     let query_embedding = generator.generate_embedding_default(query
    // ).unwrap();     let comparand_embeddings: Vec<Embedding> =
    // comparands         .iter()
    //         .map(|text| generator.generate_embedding(text).unwrap())
    //         .collect();

    //     // Print the embeddings
    //     query_embedding.print();
    //     println!("---");
    //     for embedding in &comparand_embeddings {
    //         embedding.print();
    //     }

    //     // Calculate the cosine similarity between the query and each
    // comparand, and     // sort by similarity
    //     let mut similarities: Vec<(Embedding, f32)> = comparand_embeddings
    //         .iter()
    //         .map(|embedding| (embedding.clone(),
    // query_embedding.cosine_similarity(&embedding)))         .collect();
    //     similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    //     let similarities =
    // query_embedding.score_similarities(comparand_embeddings);

    //     // Print similarities
    //     println!("---");
    //     println!("Similarities:");
    //     for scored_embedding in &similarities {
    //         scored_embedding.print();
    //     }

    //     assert!(similarities[0].embedding.id == "A plane in the sky");
    // }
}
