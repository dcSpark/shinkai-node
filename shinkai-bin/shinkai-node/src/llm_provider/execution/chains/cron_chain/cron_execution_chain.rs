use crate::cron_tasks::web_scrapper::CronTaskRequest;
use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::{
    chains::inference_chain_router::InferenceChainDecision, prompts::prompts::JobPromptGenerator,
};
use crate::llm_provider::job::Job;
use crate::llm_provider::job_manager::JobManager;
use async_recursion::async_recursion;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

use super::cron_creation_chain::CronCreationChainResponse;

#[derive(Debug, Clone, Default)]
pub struct CronExecutionChainResponse {
    pub summary: String,
    pub needs_links: bool,
}

#[derive(Debug, Clone)]
pub struct CronExecutionState {
    stage: String,
    summary: Option<String>,
    needs_links: Option<bool>,
}

impl JobManager {
    #[async_recursion]
    pub async fn start_cron_execution_chain_for_subtask(
        _db: Arc<ShinkaiDB>,
        _full_job: Job,
        agent: SerializedLLMProvider,
        _execution_context: HashMap<String, String>,
        _user_profile: Option<ShinkaiName>,
        task_description: String, // what
        web_content: String,      // where
        iteration_count: u64,
        max_iterations: u64,
    ) -> Result<String, LLMProviderError> {
        if iteration_count > max_iterations {
            return Err(LLMProviderError::InferenceRecursionLimitReached(task_description.clone()));
        }

        let web_prompt = JobPromptGenerator::cron_subtask(task_description.clone(), web_content.clone());
        let response_json = JobManager::inference_agent_markdown(agent.clone(), web_prompt).await?;

        if let Ok(answer_str) = JobManager::direct_extract_key_inference_response(response_json.clone(), "answer") {
            Ok(answer_str)
        } else {
            Err(LLMProviderError::InferenceFailed)
        }
    }

    /// An inference chain for question-answer job tasks which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    pub async fn start_cron_execution_chain_for_main_task(
        db: Arc<ShinkaiDB>,
        full_job: Job,
        agent: SerializedLLMProvider,
        execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        task_description: String, // what
        web_content: String,      // where
        links: Vec<String>,
        iteration_count: u64,
        max_iterations: u64,
        state: Option<CronExecutionState>,
    ) -> Result<CronExecutionChainResponse, LLMProviderError> {
        if iteration_count > max_iterations {
            return Err(LLMProviderError::InferenceRecursionLimitReached(task_description.clone()));
        }

        let (filled_prompt, _response_key, next_stage) = match state.as_ref().map(|s| s.stage.as_str()) {
            None | Some("apply_to_website_prompt") => {
                let filled_web_prompt =
                    JobPromptGenerator::apply_to_website_prompt(task_description.clone(), web_content.clone());
                (filled_web_prompt, "summary", "does_it_need_links")
            }
            Some("does_it_need_links") => {
                // You need to implement the logic for generating the prompt for this stage
                let filled_needs_links_prompt = JobPromptGenerator::cron_web_task_requires_links(
                    task_description.clone(),
                    state.as_ref().and_then(|s| s.summary.clone()).unwrap_or_default(),
                );
                (filled_needs_links_prompt, "needs_links", "match_with_links")
            }
            Some("match_with_links") => {
                // You need to implement the logic for generating the prompt for this stage
                let filled_match_with_links_prompt = JobPromptGenerator::cron_web_task_match_links(
                    task_description.clone(),
                    state.as_ref().and_then(|s| s.summary.clone()).unwrap_or_default(),
                    links.clone(),
                );
                (filled_match_with_links_prompt, "matched_links", "")
            }
            _ => {
                return Err(LLMProviderError::InvalidCronExecutionChainStage(
                    state.as_ref().map(|s| s.stage.clone()).unwrap_or_default(),
                ))
            }
        };

        let response_json = JobManager::inference_agent_markdown(agent.clone(), filled_prompt).await?;

        if let Ok(answer_str) = JobManager::direct_extract_key_inference_response(response_json.clone(), "answer") {
            let mut new_state = state.unwrap_or_else(|| CronExecutionState {
                stage: "apply_to_website_prompt".to_string(),
                summary: Some(answer_str.clone()),
                needs_links: None,
            });

            match new_state.stage.as_str() {
                "does_it_need_links" => {
                    new_state.needs_links = Some(answer_str.parse::<bool>().unwrap_or(false));
                    if !new_state.needs_links.unwrap() {
                        return Ok(CronExecutionChainResponse {
                            summary: new_state.summary.unwrap(),
                            needs_links: new_state.needs_links.unwrap(),
                        });
                    }
                }
                "match_with_links" => {
                    new_state.summary = Some(answer_str.clone());
                    return Ok(CronExecutionChainResponse {
                        summary: new_state.summary.unwrap(),
                        needs_links: new_state.needs_links.unwrap(),
                    });
                }
                _ => (),
            };

            new_state.stage = next_stage.to_string();
            if new_state.stage.is_empty() {
                return Ok(CronExecutionChainResponse {
                    summary: new_state.summary.unwrap(),
                    needs_links: new_state.needs_links.unwrap(),
                });
            } else {
                return Self::start_cron_execution_chain_for_main_task(
                    db,
                    full_job,
                    agent,
                    execution_context,
                    user_profile,
                    task_description,
                    web_content,
                    links,
                    iteration_count + 1,
                    max_iterations,
                    Some(new_state),
                )
                .await;
            }
        } else {
            return Err(LLMProviderError::InferenceFailed);
        }
    }

    // TODO: Delete this
    // TODO: Merge this with the above function. We are not doing that right now bc we need to decide how to select Chains.
    // Could it be based on the first message of the Job?
    #[instrument(skip(db))]
    pub async fn alt_inference_chain_router(
        db: Arc<ShinkaiDB>,
        agent_found: Option<SerializedLLMProvider>,
        full_job: Job,
        job_message: JobMessage,
        cron_task_request: CronTaskRequest,
        prev_execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(CronCreationChainResponse, HashMap<String, String>), LLMProviderError> {
        // TODO: this part is very similar to the above function so it is easier to merge them.
        let chosen_chain = InferenceChainDecision::new_no_results("cron_creation_chain".to_string());
        let mut inference_response_content = CronCreationChainResponse::default();
        let new_execution_context = HashMap::new();

        if chosen_chain.chain_id == *"cron_creation_chain" {
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
                return Err(LLMProviderError::LLMProviderNotFound);
            }
        }
        Ok((inference_response_content, new_execution_context))
    }

    #[instrument(skip(db, chosen_chain))]
    pub async fn cron_inference_chain_router_summary(
        db: Arc<ShinkaiDB>,
        agent_found: Option<SerializedLLMProvider>,
        full_job: Job,
        task_description: String,
        web_content: String,
        links: Vec<String>,
        prev_execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        chosen_chain: InferenceChainDecision,
    ) -> Result<(String, HashMap<String, String>), LLMProviderError> {
        let mut inference_response_content: String = String::new();
        let mut new_execution_context = HashMap::new();

        // Note: Faking it until you merge it
        if chosen_chain.chain_id == *"cron_execution_chain" {
            if let Some(agent) = agent_found.clone() {
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
                return Err(LLMProviderError::LLMProviderNotFound);
            }
        } else if chosen_chain.chain_id == *"cron_execution_chain_subtask" {
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

                new_execution_context.insert("previous_step_response".to_string(), inference_response_content.clone());
            } else {
                return Err(LLMProviderError::LLMProviderNotFound);
            }
        }
        Ok((inference_response_content, new_execution_context))
    }
}
