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
        agent_found: Option<SerializedAgent>,
        execution_context: HashMap<String, String>,
        user_profile: Option<ShinkaiName>,
        task: String,
        image: String,
        iteration_count: u64,
        max_iterations: u64,
    ) -> Result<(String, HashMap<String, String>), AgentError> {
        if iteration_count > max_iterations {
            return Err(AgentError::InferenceRecursionLimitReached("Image Analysis".to_string()));
        }

        let agent = match agent_found {
            Some(agent) => agent,
            None => return Err(AgentError::AgentNotFound),
        };

        let image_prompt = JobPromptGenerator::image_to_text_analysis(task, image);
        let response_json = JobManager::inference_agent(agent.clone(), image_prompt).await?;
        let mut new_execution_context = HashMap::new();

        if let Ok(answer_str) = JobManager::extract_inference_json_response(response_json.clone(), "answer") {
            new_execution_context.insert("previous_step_response".to_string(), answer_str.clone());
            Ok((answer_str, new_execution_context))
        } else {
            Err(AgentError::InferenceFailed)
        }
    }
}
