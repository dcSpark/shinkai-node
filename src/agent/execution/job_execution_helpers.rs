use super::job_prompts::{JobPromptGenerator, Prompt};
use crate::agent::error::AgentError;
use crate::agent::job::Job;
use crate::agent::{agent::Agent, job_manager::JobManager};
use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use async_std::println;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::source::{SourceFileType, VRSource};
use std::result::Result::Ok;
use std::sync::Arc;
use tokio::sync::Mutex;

impl JobManager {
    /// Extracts a String using the provided key in the JSON response
    /// Errors if the key is not present.
    pub fn extract_inference_json_response(response_json: JsonValue, key: &str) -> Result<String, AgentError> {
        if let Some(value) = response_json.get(key) {
            let value_str = value
                .as_str()
                .ok_or_else(|| AgentError::InferenceJSONResponseMissingField(key.to_string()))?;
            Ok(value_str.to_string())
        } else {
            Err(AgentError::InferenceJSONResponseMissingField(key.to_string()))
        }
    }

    /// Inferences the Agent's LLM with the given prompt. Automatically validates the response is
    /// a valid JSON object/retrying if it isn't, and finally attempt to extract the provided key
    /// from the JSON object. Errors if the key is not found.
    pub async fn inference_agent_and_extract(
        agent: SerializedAgent,
        filled_prompt: Prompt,
        key: &str,
    ) -> Result<String, AgentError> {
        let response_json = JobManager::inference_agent(agent.clone(), filled_prompt).await?;
        JobManager::extract_inference_json_response(response_json, key)
    }

    /// Inferences the Agent's LLM with the given prompt. Automatically validates the response is
    /// a valid JSON object, and if it isn't re-inferences to ensure that it is returned as one.
    pub async fn inference_agent(agent: SerializedAgent, filled_prompt: Prompt) -> Result<JsonValue, AgentError> {
        let agent_cloned = agent.clone();
        let prompt_cloned = filled_prompt.clone();
        let task_response = tokio::spawn(async move {
            let agent = Agent::from_serialized_agent(agent_cloned);
            agent.inference(prompt_cloned).await
        })
        .await;

        let response = match task_response {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Task panicked with error: {:?}", e);
                return Err(AgentError::InferenceFailed);
            }
        };

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("inference_agent> response: {:?}", response).as_str(),
        );

        // Validates that the response is a proper JSON object, else inferences again to get the
        // LLM to parse the previous response into proper JSON
        JobManager::extract_json_value_from_inference_response(response, agent.clone(), filled_prompt).await
    }

    /// Attempts to extract the JsonValue out of the LLM's response. If it is not proper JSON
    /// then inferences the LLM again asking it to take its previous answer and make sure it responds with a proper JSON object.
    async fn extract_json_value_from_inference_response(
        response: Result<JsonValue, AgentError>,
        agent: SerializedAgent,
        filled_prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        match response {
            Ok(json) => Ok(json),
            Err(AgentError::FailedExtractingJSONObjectFromResponse(text)) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    "FailedExtractingJSONObjectFromResponse",
                );
                match JobManager::json_not_found_retry(agent.clone(), text.clone(), filled_prompt).await {
                    Ok(json) => Ok(json),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(AgentError::FailedExtractingJSONObjectFromResponse(e.to_string())),
        }
    }

    /// Inferences the LLM again asking it to take its previous answer and make sure it responds with a proper JSON object
    /// that we can parse.
    async fn json_not_found_retry(
        agent: SerializedAgent,
        text: String,
        prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        let response = tokio::spawn(async move {
            let agent = Agent::from_serialized_agent(agent);
            let prompt = JobPromptGenerator::basic_json_retry_response_prompt(text, prompt);
            agent.inference(prompt).await
        })
        .await;
        let response = match response {
            Ok(res) => res?,
            Err(e) => {
                eprintln!("Task panicked with error: {:?}", e);
                return Err(AgentError::InferenceFailed);
            }
        };

        Ok(response)
    }

    /// Fetches boilerplate/relevant data required for a job to process a step
    pub async fn fetch_relevant_job_data(
        job_id: &str,
        db: Arc<Mutex<ShinkaiDB>>,
    ) -> Result<(Job, Option<SerializedAgent>, String, Option<ShinkaiName>), AgentError> {
        // Fetch the job
        let full_job = { db.lock().await.get_job(job_id)? };

        // Acquire Agent
        let agent_id = full_job.parent_agent_id.clone();
        let mut agent_found = None;
        let mut profile_name = String::new();
        let mut user_profile: Option<ShinkaiName> = None;
        let agents = JobManager::get_all_agents(db).await.unwrap_or(vec![]);
        for agent in agents {
            if agent.id == agent_id {
                agent_found = Some(agent.clone());
                profile_name = agent.full_identity_name.full_name.clone();
                user_profile = Some(agent.full_identity_name.extract_profile().unwrap());
                break;
            }
        }

        Ok((full_job, agent_found, profile_name, user_profile))
    }

    pub async fn get_all_agents(db: Arc<Mutex<ShinkaiDB>>) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        let db = db.lock().await;
        db.get_all_agents()
    }
}
