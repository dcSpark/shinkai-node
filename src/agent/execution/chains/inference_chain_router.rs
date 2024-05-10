use crate::agent::error::AgentError;
use crate::agent::execution::user_message_parser::ParsedUserMessage;
use crate::agent::job::{Job, JobStepResult};
use crate::agent::job_manager::JobManager;
use crate::cron_tasks::web_scrapper::CronTaskRequest;
use crate::db::ShinkaiDB;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::vector_fs::vector_fs::VectorFS;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};

use tracing::instrument;

use super::cron_chain::cron_creation_chain::CronCreationChainResponse;
use super::qa_chain::qa_inference_chain::QAInferenceChain;
use super::summary_chain::summary_inference_chain::SummaryInferenceChain;

/// The output result of the inference chain router
pub enum InferenceChainDecision {
    QAChain,
    SummaryChain(((bool, f32), (bool, f32), (bool, f32))),
    ToolExecutionChain,
    CodingChain,
    CronCreationChain,
    CronExecutionChainMainTask,
    CronExecutionChainSubtask,
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
        match chosen_chain {
            InferenceChainDecision::SummaryChain(score_results) => {
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
                    score_results,
                )
                .await?
            }
            InferenceChainDecision::QAChain => {
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
            // Add other chains here
            _ => {}
        };

        Ok((inference_response_content, new_execution_context))
    }

    // TODO: Delete this
    // TODO: Merge this with the above function. We are not doing that right now bc we need to decide how to select Chains.
    // Could it be based on the first message of the Job?
    #[instrument(skip(db))]
    pub async fn alt_inference_chain_router(
        db: Arc<ShinkaiDB>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        job_message: JobMessage,
        cron_task_request: CronTaskRequest,
        prev_execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(CronCreationChainResponse, HashMap<String, String>), AgentError> {
        // TODO: this part is very similar to the above function so it is easier to merge them.
        let chosen_chain = InferenceChainDecision::CronCreationChain;
        let mut inference_response_content = CronCreationChainResponse::default();
        let mut new_execution_context = HashMap::new();

        match chosen_chain {
            InferenceChainDecision::CronCreationChain => {
                if let Some(agent) = agent_found {
                    inference_response_content = JobManager::start_cron_creation_chain(
                        db,
                        full_job,
                        job_message.content.clone(),
                        agent,
                        prev_execution_context,
                        user_profile,
                        cron_task_request.cron_description,
                        cron_task_request.task_description,
                        cron_task_request.object_description,
                        0,
                        6, // TODO: Make this configurable
                        None,
                    )
                    .await?;
                } else {
                    return Err(AgentError::AgentNotFound);
                }
            }
            // Add other chains here
            _ => {}
        }
        Ok((inference_response_content, new_execution_context))
    }

    #[instrument(skip(db, chosen_chain))]
    pub async fn cron_inference_chain_router_summary(
        db: Arc<ShinkaiDB>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        task_description: String,
        web_content: String,
        links: Vec<String>,
        prev_execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        chosen_chain: InferenceChainDecision,
    ) -> Result<(String, HashMap<String, String>), AgentError> {
        let mut inference_response_content: String = String::new();
        let mut new_execution_context = HashMap::new();

        // Note: Faking it until you merge it
        match chosen_chain {
            InferenceChainDecision::CronExecutionChainMainTask => {
                if let Some(agent) = agent_found {
                    let response = JobManager::start_cron_execution_chain_for_main_task(
                        db,
                        full_job,
                        agent,
                        prev_execution_context,
                        user_profile,
                        task_description,
                        web_content,
                        links,
                        0,
                        6, // TODO: Make this configurable
                        None,
                    )
                    .await?;
                    inference_response_content = response.summary;
                } else {
                    return Err(AgentError::AgentNotFound);
                }
            }
            InferenceChainDecision::CronExecutionChainSubtask => {
                if let Some(agent) = agent_found {
                    inference_response_content = JobManager::start_cron_execution_chain_for_subtask(
                        db,
                        full_job,
                        agent,
                        prev_execution_context,
                        user_profile,
                        task_description,
                        web_content,
                        0,
                        6, // TODO: Make this configurable
                    )
                    .await?;

                    new_execution_context
                        .insert("previous_step_response".to_string(), inference_response_content.clone());
                } else {
                    return Err(AgentError::AgentNotFound);
                }
            }
            // Add other chains here
            _ => {}
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
        InferenceChainDecision::QAChain
    }
}
