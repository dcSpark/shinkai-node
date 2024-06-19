use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::resource_errors::VRError;
use std::result::Result::Ok;

/// Scores job task embedding against a set of embeddings and returns the highest score.
pub fn top_score_embeddings(embeddings: Vec<(String, Embedding)>, user_message_embedding: &Embedding) -> f32 {
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


/// Scores job task embedding against "summarize these" embeddings and returns the highest score.
pub async fn top_score_summarize_these_embeddings(
    generator: RemoteEmbeddingGenerator,
    user_message_embedding: &Embedding,
) -> Result<f32, VRError> {
    let embeddings = summarize_these_embeddings(generator).await?;
    Ok(top_score_embeddings(embeddings, user_message_embedding))
}

/// Scores job task embedding against "summarize this" embeddings and returns the highest score.
pub async fn top_score_summarize_this_embeddings(
    generator: RemoteEmbeddingGenerator,
    user_message_embedding: &Embedding,
) -> Result<f32, VRError> {
    let embeddings = summarize_this_embeddings(generator).await?;
    Ok(top_score_embeddings(embeddings, user_message_embedding))
}

/// Scores job task embedding against other summary embeddings and returns the highest score.
pub async fn top_score_summarize_other_embeddings(
    generator: RemoteEmbeddingGenerator,
    user_message_embedding: &Embedding,
) -> Result<f32, VRError> {
    let embeddings = summarize_other_embeddings(generator).await?;
    Ok(top_score_embeddings(embeddings, user_message_embedding))
}

/// Scores job task embedding against message history summary embeddings and returns the highest score.
pub async fn top_score_message_history_summary_embeddings(
    generator: RemoteEmbeddingGenerator,
    user_message_embedding: &Embedding,
) -> Result<f32, VRError> {
    let embeddings = message_history_summary_embeddings(generator).await?;
    Ok(top_score_embeddings(embeddings, user_message_embedding))
}

/// Returns summary embeddings related to requests for summarizing multiple documents or files
pub async fn summarize_these_embeddings(
    generator: RemoteEmbeddingGenerator,
) -> Result<Vec<(String, Embedding)>, VRError> {
    let strings = vec![
        "Summarize these files".to_string(),
        "I want a summary of these".to_string(),
        "These files, I need a summary".to_string(),
        "Summarize all of these together".to_string(),
        "Provide a summary for these documents".to_string(),
        "Can you summarize these?".to_string(),
        "Need a quick summary of these files".to_string(),
        "Sum up these documents for me".to_string(),
        "Give an overview of these files".to_string(),
        "Condense these documents into a summary".to_string(),
        "Wrap up these files in a summary".to_string(),
        "Break down these documents for me".to_string(),
        "Summarize the contents of these files".to_string(),
        "Quick summary of these, please".to_string(),
        "Overview these documents".to_string(),
        "Condense these into a summary".to_string(),
        "Summarize these readings".to_string(),
        "Give a concise summary of these documents".to_string(),
        "Summarize them".to_string(),
        "Summarize these documents/files".to_string(),
        "Give me a summary of these docs/files".to_string(),
        "Summarize the chat context".to_string(),
        "Overview all of these documents".to_string(),
        "Suammarize these".to_string(),
    ];

    let ids = vec!["".to_string(); strings.len()];
    let embeddings = generator.generate_embeddings(&strings, &ids).await?;
    Ok(strings.into_iter().zip(embeddings.into_iter()).collect())
}

/// Returns summary embeddings related to specific requests for summarization
pub async fn summarize_this_embeddings(
    generator: RemoteEmbeddingGenerator,
) -> Result<Vec<(String, Embedding)>, VRError> {
    let strings = vec![
        "Summarize this for me".to_string(),
        "Recap the below for me:".to_string(),
        "Summarize this".to_string(),
        "Give me a summary:".to_string(),
        "Provide a summary of this".to_string(),
        "Can you summarize this?".to_string(),
        "Summarize the following".to_string(),
        "Summarization needed".to_string(),
        "Sum it up for me".to_string(),
        "Overview this content".to_string(),
        "Condense this into a summary".to_string(),
        "Wrap this up in a summary".to_string(),
        "Break this down for me".to_string(),
        "Give me an overview".to_string(),
    ];
    let ids = vec!["".to_string(); strings.len()];
    let embeddings = generator.generate_embeddings(&strings, &ids).await;
    if let Err(e) = embeddings {
        println!("Failed generating this embeddings: {:?}", e);
        return Err(e);
    }

    Ok(strings.into_iter().zip(embeddings.unwrap().into_iter()).collect())
}

/// Returns summary embeddings related to specific requests for summarization
pub async fn summarize_other_embeddings(
    generator: RemoteEmbeddingGenerator,
) -> Result<Vec<(String, Embedding)>, VRError> {
    let strings = vec![
        "Explain what this is".to_string(),
        "What do this/these document(s) talk about".to_string(),
        "What is this about?".to_string(),
        "Go into detail on this".to_string(),
        "Give me a rundown".to_string(),
    ];
    let ids = vec!["".to_string(); strings.len()];
    let embeddings = generator.generate_embeddings(&strings, &ids).await;
    if let Err(e) = embeddings {
        println!("Failed generating this embeddings: {:?}", e);
        return Err(e);
    }

    Ok(strings.into_iter().zip(embeddings.unwrap().into_iter()).collect())
}

/// Returns summary embeddings related to chat message history
pub async fn message_history_summary_embeddings(
    generator: RemoteEmbeddingGenerator,
) -> Result<Vec<(String, Embedding)>, VRError> {
    let strings = vec![
        "Summarize our conversation.".to_string(),
        "Summarize the message history".to_string(),
        "Recap the message history".to_string(),
        "Recap the conversation".to_string(),
        "Give a rundown of our discussion.".to_string(),
        "Outline the key points from our chat.".to_string(),
        "Condense our conversation into a summary.".to_string(),
        "Highlight the main points from this chat.".to_string(),
        "Give a brief overview of our conversation.".to_string(),
        "Summarize the key takeaways from this chat.".to_string(),
        "Summarize the gist of our conversation.".to_string(),
        "Recap the core points of our chat.".to_string(),
    ];
    let ids = vec!["".to_string(); strings.len()];
    let embeddings = generator.generate_embeddings(&strings, &ids).await?;
    Ok(strings.into_iter().zip(embeddings.into_iter()).collect())
}
