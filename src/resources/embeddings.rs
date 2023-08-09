pub use llm::ModelArchitecture;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
}

impl Embedding {
    /// Creates a new `Embedding`.
    pub fn new(id: &str, vector: Vec<f32>) -> Self {
        Embedding {
            id: String::from(id),
            vector,
        }
    }

    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    pub fn set_id_with_integer(&mut self, id: u64) {
        self.id = id.to_string();
    }

    /// Print embedding.
    pub fn print(&self) {
        println!("Embedding ID: {}", self.id);
        println!("  Embeddings length: {}", self.vector.len());
        println!("  Embeddings first 10: {:.02?}", &self.vector[0..10]);
    }

    /// Calculate the cosine similarity between two embedding vectors
    pub fn cosine_similarity(&self, embedding2: &Embedding) -> f32 {
        let dot_product = self.dot(&self.vector, &embedding2.vector);
        let magnitude1 = self.magnitude(&self.vector);
        let magnitude2 = self.magnitude(&embedding2.vector);

        dot_product / (magnitude1 * magnitude2)
    }

    /// Calculate the dot product between two vectors.
    fn dot(&self, v1: &[f32], v2: &[f32]) -> f32 {
        v1.iter().zip(v2.iter()).map(|(&x, &y)| x * y).sum()
    }

    /// Calculate the magnitude of a vector.
    fn magnitude(&self, v: &[f32]) -> f32 {
        v.iter().map(|&x| x * x).sum::<f32>().sqrt()
    }

    /// Calculate the cosine similarity between the query embedding
    /// (self) and a list of embeddings.
    pub fn score_similarities(&self, embeddings: Vec<Embedding>) -> Vec<(f32, Embedding)> {
        // Calculate the cosine similarity between the query and each embedding, and
        // sort by similarity
        let mut similarities: Vec<(f32, Embedding)> = embeddings
            .iter()
            .map(|embedding| (self.cosine_similarity(&embedding), embedding.clone()))
            .collect();
        similarities.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        similarities
    }
}
