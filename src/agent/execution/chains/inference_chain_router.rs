use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::job::Job;
use crate::agent::job_manager::JobManager;
use crate::cron_tasks::web_scrapper::CronTaskRequest;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::instrument;

use super::cron_creation_chain::CronCreationChainResponse;
use super::cron_execution_chain::CronExecutionChainResponse;

pub enum InferenceChain {
    QAChain,
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
    pub async fn inference_chain_router(
        db: Arc<Mutex<ShinkaiDB>>,
        vector_fs: Arc<Mutex<VectorFS>>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        job_message: JobMessage,
        prev_execution_context: HashMap<String, String>,
        generator: &dyn EmbeddingGenerator,
        user_profile: ShinkaiName,
    ) -> Result<(String, HashMap<String, String>), AgentError> {
        // TODO: Later implement inference chain decision making here before choosing which chain to use.
        // For now we just use qa inference chain by default.
        let chosen_chain = InferenceChain::QAChain;
        let mut inference_response_content = String::new();
        let mut new_execution_context = HashMap::new();
        // Trim `\n` to prevent dumb models from responding with crappy results
        let job_message_content = job_message.content.trim_end_matches('\n');

        match chosen_chain {
            InferenceChain::QAChain => {
                if let Some(agent) = agent_found {
                    inference_response_content = JobManager::start_qa_inference_chain(
                        db,
                        vector_fs,
                        full_job,
                        job_message_content.to_string(),
                        agent,
                        prev_execution_context,
                        generator,
                        user_profile,
                        None,
                        None,
                        1,
                        2, // TODO: Make this configurable based on model capabilities
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
        };

        Ok((inference_response_content, new_execution_context))
    }

    // TODO: Delete this
    // TODO: Merge this with the above function. We are not doing that right now bc we need to decide how to select Chains.
    // Could it be based on the first message of the Job?
    #[instrument(skip(db))]
    pub async fn alt_inference_chain_router(
        db: Arc<Mutex<ShinkaiDB>>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        job_message: JobMessage,
        cron_task_request: CronTaskRequest,
        prev_execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(CronCreationChainResponse, HashMap<String, String>), AgentError> {
        // TODO: this part is very similar to the above function so it is easier to merge them.
        let chosen_chain = InferenceChain::CronCreationChain;
        let mut inference_response_content = CronCreationChainResponse::default();
        let mut new_execution_context = HashMap::new();

        match chosen_chain {
            InferenceChain::CronCreationChain => {
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

                    new_execution_context.insert(
                        "previous_step_response".to_string(),
                        inference_response_content.clone().cron_expression,
                    );
                    new_execution_context.insert(
                        "previous_step_response".to_string(),
                        inference_response_content.clone().pddl_plan_problem,
                    );
                    new_execution_context.insert(
                        "previous_step_response".to_string(),
                        inference_response_content.clone().pddl_plan_domain,
                    );
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
        db: Arc<Mutex<ShinkaiDB>>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        task_description: String,
        web_content: String,
        links: Vec<String>,
        prev_execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        chosen_chain: InferenceChain,
    ) -> Result<(String, HashMap<String, String>), AgentError> {
        let mut inference_response_content: String = String::new();
        let mut new_execution_context = HashMap::new();

        // Note: Faking it until you merge it
        match chosen_chain {
            InferenceChain::CronExecutionChainMainTask => {
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

                    new_execution_context
                        .insert("previous_step_response".to_string(), inference_response_content.clone());
                } else {
                    return Err(AgentError::AgentNotFound);
                }
            }
            InferenceChain::CronExecutionChainSubtask => {
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
