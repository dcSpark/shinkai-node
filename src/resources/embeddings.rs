pub use llm::ModelArchitecture;

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
    /// Creates a new `Embedding`.
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the `Embedding`.
    /// * `vector` - The vector of the `Embedding`.
    ///
    /// # Returns
    ///
    /// * `Self` - A new instance of `Embedding`.
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

    /// Calculate the cosine similarity between the query embedding
    /// (self) and a list of embeddings.
    ///
    /// The function calculates the cosine similarity between the query
    /// embedding and each embedding in the provided list. It returns a naively
    /// sorted vector of `ScoredEmbedding` objects, where each object
    /// contains the cosine similarity score and the corresponding embedding.
    ///
    /// Of note, the sorting implementation is slow, so do not use this
    /// for large lists of embeddings. For large lists, we recommend adding
    /// them into a resource and using similarity_search().
    ///
    /// # Parameters
    /// - `embeddings`: A vector of `Embedding` objects representing the
    ///   embeddings to be scored.
    ///
    /// # Returns
    /// A sorted vector of `ScoredEmbedding` objects, sorted in descending order
    /// by the cosine similarity score.
    pub fn score_similarities(&self, embeddings: Vec<Embedding>) -> Vec<ScoredEmbedding> {
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
