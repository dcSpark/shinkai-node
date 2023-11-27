use async_recursion::async_recursion;
use shinkai_message_primitives::schemas::{agents::serialized_agent::SerializedAgent, shinkai_name::ShinkaiName};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use crate::{
    agent::{error::AgentError, execution::job_prompts::JobPromptGenerator, job::Job, job_manager::JobManager},
    db::ShinkaiDB,
};

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
        db: Arc<Mutex<ShinkaiDB>>,
        full_job: Job,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        task_description: String, // what
        web_content: String,      // where
        iteration_count: u64,
        max_iterations: u64,
    ) -> Result<String, AgentError> {
        if iteration_count > max_iterations {
            return Err(AgentError::InferenceRecursionLimitReached(task_description.clone()));
        }

        let web_prompt = JobPromptGenerator::cron_subtask(task_description.clone(), web_content.clone());
        let response_json = JobManager::inference_agent(agent.clone(), web_prompt).await?;

        if let Ok(answer_str) = JobManager::extract_inference_json_response(response_json.clone(), "answer") {
            Ok(answer_str)
        } else {
            Err(AgentError::InferenceFailed)
        }
    }

    /// An inference chain for question-answer job tasks which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    pub async fn start_cron_execution_chain_for_main_task(
        db: Arc<Mutex<ShinkaiDB>>,
        full_job: Job,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        task_description: String, // what
        web_content: String,      // where
        links: Vec<String>,
        iteration_count: u64,
        max_iterations: u64,
        state: Option<CronExecutionState>,
    ) -> Result<CronExecutionChainResponse, AgentError> {
        if iteration_count > max_iterations {
            return Err(AgentError::InferenceRecursionLimitReached(task_description.clone()));
        }

        let (filled_prompt, response_key, next_stage) = match state.as_ref().map(|s| s.stage.as_str()) {
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
                return Err(AgentError::InvalidCronExecutionChainStage(
                    state
                        .as_ref()
                        .map(|s| s.stage.clone())
                        .unwrap_or_else(|| "".to_string()),
                ))
            }
        };

        let response_json = JobManager::inference_agent(agent.clone(), filled_prompt).await?;

        if let Ok(answer_str) = JobManager::extract_inference_json_response(response_json.clone(), "answer") {
            let mut new_state = state.unwrap_or_else(|| CronExecutionState {
                stage: "apply_to_website_prompt".to_string(),
                summary: Some(answer_str.clone()),
                needs_links: None,
            });

            match new_state.stage.as_str() {
                "does_it_need_links" => {
                    new_state.needs_links = Some(answer_str.parse::<bool>().unwrap_or(false));
                    if new_state.needs_links.unwrap() == false {
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
            return Err(AgentError::InferenceFailed);
        }
    }
}
