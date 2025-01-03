use crate::model_type::EmbeddingModelType;
use crate::shinkai_embedding_errors::ShinkaiEmbeddingError;
use async_trait::async_trait;
use crate::embedding_generator::EmbeddingGenerator;

#[derive(Clone)]
pub struct MockGenerator {
    model_type: EmbeddingModelType,
    num_embeddings: usize,
}

impl MockGenerator {
    pub fn new(model_type: EmbeddingModelType, num_embeddings: usize) -> Self {
        MockGenerator {
            model_type,
            num_embeddings,
        }
    }
}

#[async_trait]
impl EmbeddingGenerator for MockGenerator {
    fn model_type(&self) -> EmbeddingModelType {
        self.model_type.clone()
    }

    fn set_model_type(&mut self, model_type: EmbeddingModelType) {
        self.model_type = model_type;
    }

    fn box_clone(&self) -> Box<dyn EmbeddingGenerator> {
        Box::new((*self).clone())
    }

    fn generate_embedding_blocking(&self, _input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
        Ok(vec![0.0; self.num_embeddings])
    }

    fn generate_embeddings_blocking(&self, input_strings: &Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
        Ok(input_strings.iter().map(|_| vec![0.0; self.num_embeddings]).collect())
    }

    async fn generate_embedding(&self, _input_string: &str) -> Result<Vec<f32>, ShinkaiEmbeddingError> {
        Ok(vec![0.0; self.num_embeddings])
    }

    async fn generate_embeddings(&self, input_strings: &Vec<String>) -> Result<Vec<Vec<f32>>, ShinkaiEmbeddingError> {
        Ok(input_strings.iter().map(|_| vec![0.0; self.num_embeddings]).collect())
    }
}
