use super::inference_chain_trait::{InferenceChain, InferenceChainContext, InferenceChainResult, ScoreResult};
use super::qa_chain::qa_inference_chain::QAInferenceChain;
use super::summary_chain::summary_inference_chain::SummaryInferenceChain;
use crate::db::ShinkaiDB;
use crate::llm_provider::error::AgentError;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job::{Job, JobStepResult};
use crate::llm_provider::job_manager::JobManager;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::vector_fs::vector_fs::VectorFS;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

/// The chosen chain result by the inference chain router
pub struct InferenceChainDecision {
    pub chain_id: String,
    pub score_results: HashMap<String, ScoreResult>,
}

impl InferenceChainDecision {
    pub fn new(chain_id: String, score_results: HashMap<String, ScoreResult>) -> Self {
        Self {
            chain_id,
            score_results,
        }
    }

    pub fn new_no_results(chain_id: String) -> Self {
        Self {
            chain_id,
            score_results: HashMap::new(),
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
    ) -> Result<InferenceChainResult, AgentError> {
        // Initializations
        let mut inference_result = InferenceChainResult::new_empty();
        let agent = agent_found.ok_or(AgentError::AgentNotFound)?;
        let max_tokens_in_prompt = ModelCapabilitiesManager::get_max_input_tokens(&agent.model);
        let parsed_user_message = ParsedUserMessage::new(job_message.content.to_string());
        let job_scope_contains_significant_content = full_job.scope.contains_significant_content();

        // Choose the inference chain based on the user message
        let chosen_chain = choose_inference_chain(
            parsed_user_message.clone(),
            generator.clone(),
            &full_job.scope,
            &full_job.step_history,
        )
        .await;
        // Create the inference chain context
        let mut chain_context = InferenceChainContext::new(
            db,
            vector_fs,
            full_job,
            parsed_user_message,
            agent,
            prev_execution_context,
            generator,
            user_profile,
            2,
            max_tokens_in_prompt,
            HashMap::new(),
        );

        // If the Summary chain was chosen
        if chosen_chain.chain_id == SummaryInferenceChain::chain_id() {
            let mut summary_chain = SummaryInferenceChain::new(chain_context, chosen_chain.score_results);
            inference_result = summary_chain.run_chain().await?;
        }
        // If the QA chain was chosen
        else if chosen_chain.chain_id == QAInferenceChain::chain_id() {
            let qa_iteration_count = if job_scope_contains_significant_content { 3 } else { 2 };
            chain_context.update_max_iterations(qa_iteration_count);

            let mut qa_chain = QAInferenceChain::new(chain_context);
            inference_result = qa_chain.run_chain().await?;
        }

        Ok(inference_result)
    }
}

/// Chooses the inference chain based on the user message
async fn choose_inference_chain(
    parsed_user_message: ParsedUserMessage,
    generator: RemoteEmbeddingGenerator,
    job_scope: &JobScope,
    step_history: &Vec<JobStepResult>,
) -> InferenceChainDecision {
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
