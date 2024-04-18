use crate::agent::error::AgentError;
use crate::agent::execution::job_task_parser::ParsedJobTask;
use crate::agent::job::{Job, JobId, JobLike};
use crate::agent::job_manager::JobManager;
use crate::agent::parsing_helper::ParsingHelper;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use keyphrases::KeyPhraseExtractor;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

impl JobManager {
    /// An inference chain for summarizing every VR in the job's scope.
    #[async_recursion]
    #[instrument(skip(generator, vector_fs, db))]
    pub async fn start_summary_inference_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        job_task: ParsedJobTask,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
    ) -> Result<String, AgentError> {
        Ok("Summary inference chain has been chosen".to_string())
    }

    /// Checks if the job's task contains any variation of the word summary,
    /// including common misspellings, or has an extremely high embedding similarity score to the word summary.
    pub async fn validate_job_task_requests_summary(
        job_task: ParsedJobTask,
        generator: RemoteEmbeddingGenerator,
        job_scope: &JobScope,
    ) -> bool {
        // Filter out code blocks
        let only_text_job_task = job_task.get_output_string_filtered(false, true);
        let job_task_embedding = if let Ok(e) = generator.generate_embedding(&only_text_job_task, "").await {
            e
        } else {
            return false;
        };
        // TODO: fetch message history summary embeddings separately, and only pass if job scope not empty
        let all_summary_embeddings = if let Ok(e) = Self::all_summary_embeddings(generator).await {
            e
        } else {
            return false;
        };
        for (summary_string, summary_embedding) in all_summary_embeddings {
            let score = summary_embedding.score_similarity(&job_task_embedding);
            eprintln!("{} - Score: {:.2}", summary_string, score);
            if score > 0.9 {
                return true;
            }
        }

        return false;
    }

    /// Returns all summary embeddings which can be used to detect if the job task is requesting a summary.
    async fn all_summary_embeddings(generator: RemoteEmbeddingGenerator) -> Result<Vec<(String, Embedding)>, VRError> {
        let mut all_embeddings = vec![];
        all_embeddings.extend(Self::message_history_summary_embeddings(generator).await?);
        Ok(all_embeddings)
    }

    /// Returns summary embeddings related to chat message history
    async fn message_history_summary_embeddings(
        generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<(String, Embedding)>, VRError> {
        let strings = vec![
            "Summarize our conversation.".to_string(),
            "Summarize this chat.".to_string(),
            "Summarize this conversation.".to_string(),
            "Summarize this chat in 300 words or less.".to_string(),
            "Summarize the message history".to_string(),
            "Recap the message history".to_string(),
            "Recap the conversation".to_string(),
            "Recap our chat".to_string(),
        ];
        let ids = vec!["".to_string(); strings.len()];
        let embeddings = generator.generate_embeddings(&strings, &ids).await?;
        Ok(strings.into_iter().zip(embeddings.into_iter()).collect())
    }
}
