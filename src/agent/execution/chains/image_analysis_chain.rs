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
    pub async fn image_analysis_chain(
        db: Arc<Mutex<ShinkaiDB>>,
        full_job: Job,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        image: String,
        iteration_count: u64,
        max_iterations: u64,
    ) -> Result<String, AgentError> {
        if iteration_count > max_iterations {
            return Err(AgentError::InferenceRecursionLimitReached("Image Analysis".to_string()));
        }

        let web_prompt = JobPromptGenerator::cron_subtask(task_description.clone(), web_content.clone());
        let response_json = JobManager::inference_agent(agent.clone(), web_prompt).await?;
        if let Ok(answer_str) = JobManager::extract_inference_json_response(response_json.clone(), "answer") {
            Ok(answer_str)
        } else {
            Err(AgentError::InferenceFailed)
        }
    }
}
