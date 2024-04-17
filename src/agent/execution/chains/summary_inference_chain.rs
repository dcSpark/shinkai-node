use crate::agent::error::AgentError;
use crate::agent::execution::job_prompts::JobPromptGenerator;
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
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::instrument;

impl JobManager {
    /// An inference chain for summarizing every VR in the job's scope.
    #[async_recursion]
    #[instrument(skip(generator, vector_fs, db))]
    pub async fn start_summary_inference_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        job_task: String,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        search_text: Option<String>,
        iteration_count: u64,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
    ) -> Result<String, AgentError> {
    }
}

async fn no_json_object_retry_logic(
    response: Result<JsonValue, AgentError>,
    db: Arc<ShinkaiDB>,
    vector_fs: Arc<VectorFS>,
    full_job: Job,
    job_task: String,
    agent: SerializedAgent,
    execution_context: HashMap<String, String>,
    generator: RemoteEmbeddingGenerator,
    user_profile: ShinkaiName,
    summary_text: Option<String>,
    new_summary_node_text: Option<String>,
    iteration_count: u64,
    max_iterations: u64,
    max_tokens_in_prompt: usize,
) -> Result<String, AgentError> {
    if let Err(e) = &response {
        // If still more iterations left, then recurse to try one more time, using summary as the new search text to likely get different LLM output
        if iteration_count < max_iterations {
            return JobManager::start_qa_inference_chain(
                db,
                vector_fs,
                full_job,
                job_task.to_string(),
                agent,
                execution_context,
                generator,
                user_profile,
                summary_text.clone(),
                summary_text,
                iteration_count + 1,
                max_iterations,
                max_tokens_in_prompt,
            )
            .await;
        }
        // Else if we're past the max iterations, return either last valid summary from previous iterations or VR summary
        else {
            shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    &format!("Qa inference chain failure due to no parsable JSON produced: {}\nUsing summary backup to respond to user.", e),
                );
            // Try from previous iteration
            let mut summary_answer = String::new();
            if let Some(summary_str) = &summary_text {
                if summary_str.len() > 2 {
                    summary_answer = summary_str.to_string();
                } else {
                    // This propagates the error upwards
                    response?;
                }
            }
            // Else use the VR summary.
            else {
                let mut _temp_resp = JsonValue::Null;
                if let Some(text) = new_summary_node_text {
                    if text.len() > 2 {
                        summary_answer = text.to_string();
                    } else {
                        response?;
                    }
                } else {
                    response?;
                }
            }

            // Return the cleaned summary
            let cleaned_answer =
                ParsingHelper::flatten_to_content_if_json(&ParsingHelper::ending_stripper(summary_answer.as_str()));
            return Ok(cleaned_answer);
        }
    }
    Err(AgentError::InferenceFailed)
}

/// Checks if the job's task contains any variation of the word summary,
/// including common misspellings, or has an extremely high embedding similarity score to the word summary.
pub fn validate_job_task_requests_summary(job_task: String) -> bool {
    // Comprehensive list of common misspellings or variations of the word "summary"
    let variations = vec![
        "summary", "summery", "sumary", "sumarry", "summry", "sumery", "summart", "summare", "summair", "summiry",
        "summorie", "summurie", "summory", "sumnary", "suumary", "summray", "sumamry", "summaty", "summsry", "summarg",
        "sumnmary", "sumaryy", "summaey", "summsary", "summuary", "summaary", "summardy", "summarey", "summiray",
        "summaery",
        // Add more variations as needed
    ];
    // Convert the job_task to lowercase to make the search case-insensitive
    let removed_code_blocks = ParsingHelper::remove_code_blocks(&job_task_lower.to_lowercase);

    println!("Removed code blocks: {}", removed_code_blocks);

    // See if it contains one of the explicit variations
    let mut contains_variation = false;
    for variation in variations {
        if removed_code_blocks.contains(variation) {
            contains_summary = true;
            break;
        }
    }

    // Check if the job_task contains any of the variations
    variations.iter().any(|variation| job_task_lower.contains(variation))
}
