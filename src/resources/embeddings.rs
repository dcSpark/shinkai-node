#[macro_use]
use lazy_static::lazy_static;
use llm::load_progress_callback_stdout as load_callback;
use llm::Model;

lazy_static! {
    static ref DEFAULT_MODEL_PATH: &'static str = "pythia-160m-q4_0.bin";
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoredEmbedding {
    pub score: f32,
    pub embedding: Embedding,
}

impl ScoredEmbedding {
    /// Print scored embedding id + score.
    ///
    /// # Parameters
    /// - `embedding`: The embedding to print.
    pub fn print(&self) {
        println!("  {}: {}", self.embedding.id, self.score);
    }
}

//#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
}

impl Embedding {
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    pub fn set_id_with_integer(&mut self, id: u64) {
        self.id = id.to_string();
    }

    /// Print embedding.
    ///
    /// # Parameters
    /// - `embedding`: The embedding to print.
    pub fn print(&self) {
        println!("Embedding ID: {}", self.id);
        println!("  Embeddings length: {}", self.vector.len());
        println!("  Embeddings first 10: {:.02?}", &self.vector[0..10]);
    }

    /// Calculate the cosine similarity between two embedding vectors
    /// (self + another Embedding).
    ///
    /// # Parameters
    /// - `self`: The first embedding.
    /// - `embedding2`: The embedding to compare with self.
    ///
    /// # Returns
    /// The cosine similarity between the two embedding vectors as an f32.
    pub fn cosine_similarity(&self, embedding2: &Embedding) -> f32 {
        let dot_product = self.dot(&self.vector, &embedding2.vector);
        let magnitude1 = self.magnitude(&self.vector);
        let magnitude2 = self.magnitude(&embedding2.vector);

        dot_product / (magnitude1 * magnitude2)
    }

    /// Calculate the dot product between two vectors.
    ///
    /// # Parameters
    /// - `v1`: The first vector.
    /// - `v2`: The second vector.
    ///
    /// # Returns
    /// The dot product between the two vectors.
    fn dot(&self, v1: &[f32], v2: &[f32]) -> f32 {
        v1.iter().zip(v2.iter()).map(|(&x, &y)| x * y).sum()
    }

    /// Calculate the magnitude of a vector.
    ///
    /// # Parameters
    /// - `v`: The vector.
    ///
    /// # Returns
    /// The magnitude of the vector.
    fn magnitude(&self, v: &[f32]) -> f32 {
        v.iter().map(|&x| x * x).sum::<f32>().sqrt()
    }

    /// Calculate and score the cosine similarity between the query embedding
    /// (self) and a list of embeddings.
    ///
    /// The function calculates the cosine similarity between the query
    /// embedding and each embedding in the provided list. It returns a
    /// sorted vector of `ScoredEmbedding` objects, where each object
    /// contains the cosine similarity score and the corresponding embedding.
    ///
    /// # Parameters
    /// - `embeddings`: A vector of `Embedding` objects representing the
    ///   embeddings to be scored.
    ///
    /// # Returns
    /// A sorted vector of `ScoredEmbedding` objects, sorted in descending order
    /// by the cosine similarity score.
    pub fn score_similarity(&self, embeddings: Vec<Embedding>) -> Vec<ScoredEmbedding> {
        // Calculate the cosine similarity between the query and each embedding, and
        // sort by similarity
        let mut similarities: Vec<ScoredEmbedding> = embeddings
            .iter()
            .map(|embedding| ScoredEmbedding {
                score: self.cosine_similarity(&embedding),
                embedding: embedding.clone(),
            })
            .collect();
        similarities.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        similarities
    }
}

pub struct EmbeddingGenerator {
    model: Box<dyn Model>,
}

impl EmbeddingGenerator {
    /// Create a new EmbeddingGenerator with a specified model.
    ///
    /// # Parameters
    /// - `model`: The model to be used for generating embeddings.
    ///
    /// # Returns
    /// A new `EmbeddingGenerator` that uses the specified model.
    pub fn new(model: Box<dyn Model>) -> Self {
        Self { model }
    }

    /// Create a new EmbeddingGenerator that uses the default model.
    ///
    /// # Returns
    /// A new `EmbeddingGenerator` that uses the default model.
    ///
    /// # Panics
    /// This function will panic if it fails to load the default model.
    pub fn new_default() -> Self {
        let default = llm::load_dynamic(
            Some(llm::ModelArchitecture::GptNeoX),
            std::path::Path::new(&*DEFAULT_MODEL_PATH),
            llm::TokenizerSource::Embedded,
            Default::default(),
            load_callback,
        )
        .unwrap_or_else(|err| panic!("Failed to load model: {}", err));
        Self { model: default }
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
    pub fn generate_embedding(&self, input_string: &str, id: &str) -> Result<Embedding, String> {
        let mut session = self.model.start_session(Default::default());
        let mut output_request = llm::OutputRequest {
            all_logits: None,
            embeddings: Some(Vec::new()),
        };
        let vocab = self.model.tokenizer();
        let beginning_of_sentence = true;

        let tokens = vocab
            .tokenize(input_string, beginning_of_sentence)
            .map_err(|err| format!("Failed to tokenize input string: {}", err))?;

        let query_token_ids = tokens.iter().map(|(_, tok)| *tok).collect::<Vec<_>>();

        self.model.evaluate(&mut session, &query_token_ids, &mut output_request);

        let vector = output_request
            .embeddings
            .ok_or_else(|| "Failed to generate embeddings".to_string())?;

        Ok(Embedding {
            id: String::from(id),
            vector,
        })
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_embeddings_generation() {
        let generator = EmbeddingGenerator::new_default();

        let dog_embeddings = generator.generate_embedding("dog", "1").unwrap();
        let cat_embeddings = generator.generate_embedding("cat", "2").unwrap();

        assert_eq!(dog_embeddings, dog_embeddings);
        assert_eq!(cat_embeddings, cat_embeddings);
        assert_ne!(dog_embeddings, cat_embeddings);
    }

    #[test]
    fn test_embedding_vector_similarity() {
        let generator = EmbeddingGenerator::new_default();

        let query = "What can fly in the sky?";
        let comparands = vec![
            "A golden retriever dog".to_string(),
            "A four legged frog".to_string(),
            "A plane in the sky".to_string(),
        ];

        // Generate embeddings for query and comparands
        let query_embedding = generator.generate_embedding(query, query).unwrap();
        let comparand_embeddings: Vec<Embedding> = comparands
            .iter()
            .map(|text| generator.generate_embedding(text, text).unwrap())
            .collect();

        // Print the embeddings
        query_embedding.print();
        println!("---");
        for embedding in &comparand_embeddings {
            embedding.print();
        }

        // Calculate the cosine similarity between the query and each comparand, and
        // sort by similarity
        let mut similarities: Vec<(Embedding, f32)> = comparand_embeddings
            .iter()
            .map(|embedding| (embedding.clone(), query_embedding.cosine_similarity(&embedding)))
            .collect();
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let similarities = query_embedding.score_similarity(comparand_embeddings);

        // Print similarities
        println!("---");
        println!("Similarities:");
        for scored_embedding in &similarities {
            scored_embedding.print();
        }

        assert!(similarities[0].embedding.id == "A plane in the sky");
    }
}
