pub use llm::ModelArchitecture;
use ordered_float::NotNan;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

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

    /// Calculate the cosine similarity score between the query embedding
    /// (self) and a list of embeddings, returning the num_of_results
    /// most similar embeddings as a tuple of (score, embedding_id)
    pub fn score_similarities(&self, embeddings: &Vec<Embedding>, num_of_results: u64) -> Vec<(f32, String)> {
        let num_of_results = num_of_results as usize;

        // Calculate the similarity scores for all chunk embeddings and skip any that
        // are NaN
        let scores: Vec<(NotNan<f32>, String)> = embeddings
            .iter()
            .filter_map(|embedding| {
                let similarity = self.cosine_similarity(embedding);
                match NotNan::new(similarity) {
                    Ok(not_nan_similarity) => Some((not_nan_similarity, embedding.id.clone())),
                    Err(_) => None, // Skip this embedding if similarity is NaN
                }
            })
            .collect();

        // Use a binary heap to more efficiently order the scores to get most similar
        let results = Self::bin_heap_order_scores(scores, num_of_results);

        results
    }

    /// Order scores using a binary heap and return the most similar scores
    pub fn bin_heap_order_scores<G>(scores: Vec<(NotNan<f32>, G)>, num_of_results: usize) -> Vec<(f32, G)>
    where
        G: Clone + Ord,
    {
        let mut heap = BinaryHeap::with_capacity(num_of_results);
        for score in scores {
            if heap.len() < num_of_results {
                heap.push(Reverse(score));
            } else if let Some(least_similar_score) = heap.peek() {
                if least_similar_score.0 .0 < score.0 {
                    heap.pop();
                    heap.push(Reverse(score));
                }
            }
        }

        // Create a Vec to hold the reversed results
        let mut results: Vec<(f32, G)> = Vec::new();

        while let Some(Reverse((similarity, id))) = heap.pop() {
            results.push((similarity.into_inner(), id));
        }

        // Reverse the order of the scores so that the highest score is first
        results.reverse();

        results
    }
}
