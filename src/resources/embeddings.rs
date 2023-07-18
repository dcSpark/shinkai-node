#[macro_use]
use lazy_static::lazy_static;
use llm::load_progress_callback_stdout as load_callback;
use llm::Model;
use std::sync::Arc;

lazy_static! {
    static ref DEFAULT_MODEL_PATH: &'static str = "pythia-160m-q4_0.bin";
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

    /// Generate embeddings for an input string.
    ///
    /// # Parameters
    /// - `input_string`: The input string for which embeddings are generated.
    ///
    /// # Returns
    /// A vector of `f32` representing the embeddings for the input string or an
    /// error.
    pub fn generate_embeddings(&self, input_string: &str) -> Result<Vec<f32>, String> {
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

        output_request
            .embeddings
            .ok_or_else(|| "Failed to generate embeddings".to_string())
    }

    // Print embeddings
    pub fn print_embeddings(&self, text: &str, embeddings: &[f32]) {
        println!("{text}");
        println!("  Embeddings length: {}", embeddings.len());
        println!("  Embeddings first 10: {:.02?}", embeddings.get(0..10));
    }

    pub fn cosine_similarity(&self, v1: &[f32], v2: &[f32]) -> f32 {
        let dot_product = self.dot(v1, v2);
        let magnitude1 = self.magnitude(v1);
        let magnitude2 = self.magnitude(v2);

        dot_product / (magnitude1 * magnitude2)
    }

    fn dot(&self, v1: &[f32], v2: &[f32]) -> f32 {
        v1.iter().zip(v2.iter()).map(|(&x, &y)| x * y).sum()
    }

    fn magnitude(&self, v: &[f32]) -> f32 {
        v.iter().map(|&x| x * x).sum::<f32>().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embeddings_generation() {
        let generator = EmbeddingGenerator::new_default();

        let dog_embeddings = generator.generate_embeddings("dog").unwrap();
        let cat_embeddings = generator.generate_embeddings("cat").unwrap();

        assert_eq!(dog_embeddings, dog_embeddings);
        assert_ne!(dog_embeddings, cat_embeddings);
    }
}
