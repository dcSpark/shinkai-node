use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::resource_errors::VRError;
use std::result::Result::Ok;

/// Scores job task embedding against a set of embeddings and returns the highest score.
pub async fn top_score_embeddings(embeddings: Vec<(String, Embedding)>, user_message_embedding: &Embedding) -> f32 {
    let mut top_score = 0.0;
    for (string, embedding) in embeddings {
        let score = embedding.score_similarity(user_message_embedding);
        println!("{} Score: {:.2}", string, score);
        if score > top_score {
            top_score = score;
        }
    }
    top_score
}
