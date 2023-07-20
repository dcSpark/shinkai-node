use crate::resources::embeddings::*;
use crate::resources::resource_errors::*;
use lazy_static::lazy_static;
use llm::load_progress_callback_stdout as load_callback;
use llm::Model;
pub use llm::ModelArchitecture;

lazy_static! {
    static ref DEFAULT_MODEL_PATH: &'static str = "pythia-160m-q4_0.bin";
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EmbeddingModelType {
    LocalModel(LocalModel),
    ExternalModel(ExternalModel),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LocalModel {
    Bloom,
    Gpt2,
    GptJ,
    GptNeoX,
    Llama,
    Mpt,
    Falcon,
}

impl LocalModel {
    pub fn from_model_architecture(arch: ModelArchitecture) -> LocalModel {
        match arch {
            ModelArchitecture::Bloom => LocalModel::Bloom,
            ModelArchitecture::Gpt2 => LocalModel::Gpt2,
            ModelArchitecture::GptJ => LocalModel::GptJ,
            ModelArchitecture::GptNeoX => LocalModel::GptNeoX,
            ModelArchitecture::Llama => LocalModel::Llama,
            ModelArchitecture::Mpt => LocalModel::Mpt,
            //ModelArchitecture::Falcon => LocalModel::Falcon, // Falcon not implemented yet in llm crate
            _ => LocalModel::Llama,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExternalModel {
    OpenAITextEmbeddingAda002,
}

pub struct RemoteEmbeddingGenerator {
    model_type: EmbeddingModelType,
    // ...
}

impl RemoteEmbeddingGenerator {
    pub fn model_type(&self) -> EmbeddingModelType {
        self.model_type.clone()
    }
}

pub struct LocalEmbeddingGenerator {
    model: Box<dyn Model>,
    model_type: EmbeddingModelType,
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
            std::path::Path::new(&*DEFAULT_MODEL_PATH),
            llm::TokenizerSource::Embedded,
            Default::default(),
            load_callback,
        )
        .unwrap_or_else(|err| panic!("Failed to load model: {}", err));
        Self {
            model,
            model_type: EmbeddingModelType::LocalModel(LocalModel::from_model_architecture(model_architecture)),
        }
    }

    /// Generate an Embedding for an input string.
    ///
    /// # Parameters
    /// - `id`: The id to be associated with the embeddings.
    /// - `input_string`: The input string for which embeddings are generated.
    /// - `metadata`: The metadata to be associated with the embeddings.
    ///
    /// # Returns
    /// An `Embedding` for the input string or an error.
    pub fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, ResourceError> {
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

    pub fn model_type(&self) -> EmbeddingModelType {
        self.model_type.clone()
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
    //     let query_embedding = generator.generate_embedding(query,
    // query).unwrap();     let comparand_embeddings: Vec<Embedding> =
    // comparands         .iter()
    //         .map(|text| generator.generate_embedding(text, text).unwrap())
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
