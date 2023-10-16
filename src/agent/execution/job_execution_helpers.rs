use crate::agent::agent::Agent;
use crate::agent::error::AgentError;
use crate::agent::job::Job;
use crate::agent::job_manager::AgentManager;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::source::{SourceFileType, VRSource};
use std::result::Result::Ok;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::job_prompts::{JobPromptGenerator, Prompt};

impl AgentManager {
    /// Extracts a String using the provided key in the JSON response
    /// Errors if the key is not present.
    pub fn extract_inference_json_response(&self, response_json: JsonValue, key: &str) -> Result<String, AgentError> {
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
        &self,
        agent: Arc<Mutex<Agent>>,
        filled_prompt: Prompt,
        key: &str,
    ) -> Result<String, AgentError> {
        let response_json = self.inference_agent(agent.clone(), filled_prompt).await?;
        self.extract_inference_json_response(response_json, key)
    }

    /// Inferences the Agent's LLM with the given prompt. Automatically validates the response is
    /// a valid JSON object, and if it isn't re-inferences to ensure that it is returned as one.
    pub async fn inference_agent(
        &self,
        agent: Arc<Mutex<Agent>>,
        filled_prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        let agent_cloned = agent.clone();
        let response = tokio::spawn(async move {
            let mut agent = agent_cloned.lock().await;
            agent.inference(filled_prompt).await
        })
        .await?;
        println!("inference_agent> response: {:?}", response);

        // Validates that the response is a proper JSON object, else inferences again to get the
        // LLM to parse the previous response into proper JSON
        self.extract_json_value_from_inference_response(response, agent.clone())
            .await
    }

    /// Attempts to extract the JsonValue out of the LLM's response. If it is not proper JSON
    /// then inferences the LLM again asking it to take its previous answer and make sure it responds with a proper JSON object.
    async fn extract_json_value_from_inference_response(
        &self,
        response: Result<JsonValue, AgentError>,
        agent: Arc<Mutex<Agent>>,
    ) -> Result<JsonValue, AgentError> {
        match response {
            Ok(json) => Ok(json),
            Err(AgentError::FailedExtractingJSONObjectFromResponse(text)) => {
                eprintln!("Retrying inference with new prompt");
                match self.json_not_found_retry(agent.clone(), text.clone()).await {
                    Ok(json) => Ok(json),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(AgentError::FailedExtractingJSONObjectFromResponse(e.to_string())),
        }
    }

    /// Inferences the LLM again asking it to take its previous answer and make sure it responds with a proper JSON object
    /// that we can parse.
    async fn json_not_found_retry(&self, agent: Arc<Mutex<Agent>>, text: String) -> Result<JsonValue, AgentError> {
        let response = tokio::spawn(async move {
            let mut agent = agent.lock().await;
            let prompt = JobPromptGenerator::basic_json_retry_response_prompt(text);
            agent.inference(prompt).await
        })
        .await?;
        Ok(response?)
    }

    /// Fetches boilerplate/relevant data required for a job to process a step
    pub async fn fetch_relevant_job_data(
        &self,
        job_id: &str,
    ) -> Result<(Job, Option<Arc<Mutex<Agent>>>, String, Option<ShinkaiName>), AgentError> {
        // Fetch the job
        let full_job = { self.db.lock().await.get_job(job_id)? };

        // Acquire Agent
        let agent_id = full_job.parent_agent_id.clone();
        let mut agent_found = None;
        let mut profile_name = String::new();
        let mut user_profile: Option<ShinkaiName> = None;
        for agent in &self.agents {
            let locked_agent = agent.lock().await;
            if locked_agent.id == agent_id {
                agent_found = Some(agent.clone());
                profile_name = locked_agent.full_identity_name.full_name.clone();
                user_profile = Some(locked_agent.full_identity_name.extract_profile().unwrap());
                break;
            }
        }

        Ok((full_job, agent_found, profile_name, user_profile))
    }
}
