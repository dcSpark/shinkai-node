use super::inference_chain_trait::InferenceChain;
use super::qa_chain::qa_inference_chain::QAInferenceChain;
use super::summary_chain::summary_inference_chain::SummaryInferenceChain;
use crate::agent::error::AgentError;
use crate::agent::execution::user_message_parser::ParsedUserMessage;
use crate::agent::job::{Job, JobStepResult};
use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::vector_fs::vector_fs::VectorFS;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::Embedding;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

/// The chosen chain result by the inference chain router
pub struct InferenceChainDecision {
    pub chain_id: String,
    pub score_results: ((bool, f32), (bool, f32), (bool, f32)),
}

impl InferenceChainDecision {
    pub fn new(chain_id: String, score_results: ((bool, f32), (bool, f32), (bool, f32))) -> Self {
        Self {
            chain_id,
            score_results,
        }
    }

    pub fn new_no_results(chain_id: String) -> Self {
        Self {
            chain_id,
            score_results: ((false, 0.0), (false, 0.0), (false, 0.0)),
        }
    }
}

impl JobManager {
    /// Chooses an inference chain based on the job message (using the agent's LLM)
    /// and then starts using the chosen chain.
    /// Returns the final String result from the inferencing, and a new execution context.
    #[instrument(skip(generator, vector_fs, db))]
    #[allow(clippy::too_many_arguments)]
    pub async fn inference_chain_router(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        job_message: JobMessage,
        prev_execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
    ) -> Result<(String, HashMap<String, String>), AgentError> {
        // Initializations
        let mut inference_response_content = String::new();
        let mut new_execution_context = HashMap::new();
        let agent = agent_found.ok_or(AgentError::AgentNotFound)?;
        let max_tokens_in_prompt = ModelCapabilitiesManager::get_max_input_tokens(&agent.model);
        let parsed_user_message = ParsedUserMessage::new(job_message.content.to_string());

        // Choose the inference chain based on the user message
        let chosen_chain = choose_inference_chain(
            parsed_user_message.clone(),
            generator.clone(),
            &full_job.scope,
            &full_job.step_history,
        )
        .await;
        if chosen_chain.chain_id.to_string() == SummaryInferenceChain::chain_id() {
            inference_response_content = SummaryInferenceChain::start_summary_inference_chain(
                db,
                vector_fs,
                full_job,
                parsed_user_message,
                agent,
                prev_execution_context,
                generator,
                user_profile,
                3,
                max_tokens_in_prompt,
                chosen_chain.score_results,
            )
            .await?
        } else if chosen_chain.chain_id.to_string() == QAInferenceChain::chain_id() {
            let qa_iteration_count = if full_job.scope.contains_significant_content() {
                3
            } else {
                2
            };
            inference_response_content = QAInferenceChain::start_qa_inference_chain(
                db,
                vector_fs,
                full_job,
                parsed_user_message.get_output_string(),
                agent,
                prev_execution_context,
                generator,
                user_profile,
                None,
                None,
                1,
                qa_iteration_count,
                max_tokens_in_prompt as usize,
            )
            .await?;
        }

        Ok((inference_response_content, new_execution_context))
    }
}

/// Chooses the inference chain based on the user message
async fn choose_inference_chain(
    parsed_user_message: ParsedUserMessage,
    generator: RemoteEmbeddingGenerator,
    job_scope: &JobScope,
    step_history: &Vec<JobStepResult>,
) -> InferenceChainDecision {
    eprintln!("Choosing inference chain");
    if let Some(summary_chain_decision) = SummaryInferenceChain::validate_user_message_requests_summary(
        parsed_user_message,
        generator.clone(),
        job_scope,
        step_history,
    )
    .await
    {
        summary_chain_decision
    } else {
        InferenceChainDecision::new_no_results(QAInferenceChain::chain_id())
    }
}

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
