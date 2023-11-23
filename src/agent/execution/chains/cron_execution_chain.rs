use std::{collections::HashMap, sync::Arc};
use async_recursion::async_recursion;
use shinkai_message_primitives::schemas::{agents::serialized_agent::SerializedAgent, shinkai_name::ShinkaiName};
use tokio::sync::Mutex;

use crate::{
    agent::{error::AgentError, execution::job_prompts::JobPromptGenerator, job::Job, job_manager::JobManager},
    db::ShinkaiDB,
};

impl JobManager {
    /// An inference chain for question-answer job tasks which vector searches the Vector Resources
    /// in the JobScope to find relevant content for the LLM to use at each step.
    #[async_recursion]
    pub async fn start_cron_execution_chain_for_summary(
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
        println!("start_cron_execution_chain>  message: {:?}", task_description);
        if iteration_count > max_iterations {
            return Err(AgentError::InferenceRecursionLimitReached(task_description.clone()));
        }

        let web_prompt = JobPromptGenerator::apply_to_website_prompt(task_description.clone(), web_content.clone());
        let response_json = JobManager::inference_agent(agent.clone(), web_prompt).await?;

        if let Ok(answer_str) = JobManager::extract_inference_json_response(response_json.clone(), "answer") {
            Ok(answer_str)
        } else {
            return Self::start_cron_execution_chain_for_summary(
                db,
                full_job,
                agent,
                execution_context,
                user_profile,
                task_description,
                web_content,
                iteration_count + 1,
                max_iterations,
            ).await;
        }
    }
}
