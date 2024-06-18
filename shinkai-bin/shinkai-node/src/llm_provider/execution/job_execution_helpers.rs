use super::chains::inference_chain_trait::LLMInferenceResponse;
use super::prompts::prompts::Prompt;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::job::Job;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_provider::LLMProvider;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::result::Result::Ok;
use std::sync::Arc;

impl JobManager {
    /// Inferences the Agent's LLM with the given markdown prompt. Automatically validates the response is
    /// a valid markdown, and processes it into a json.
    pub async fn inference_agent_markdown(
        llm_provider: SerializedLLMProvider,
        filled_prompt: Prompt,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let llm_provider_cloned = llm_provider.clone();
        let prompt_cloned = filled_prompt.clone();

        let task_response = tokio::spawn(async move {
            let llm_provider = LLMProvider::from_serialized_llm_provider(llm_provider_cloned);
            llm_provider.inference_markdown(prompt_cloned).await
        })
        .await;

        let response = task_response?;
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("inference_llm_provider_markdown> response: {:?}", response).as_str(),
        );

        response
    }

    /// Fetches boilerplate/relevant data required for a job to process a step
    /// it may return an outdated node_name
    pub async fn fetch_relevant_job_data(
        job_id: &str,
        db: Arc<ShinkaiDB>,
    ) -> Result<(Job, Option<SerializedLLMProvider>, String, Option<ShinkaiName>), LLMProviderError> {
        // Fetch the job
        let full_job = { db.get_job(job_id)? };

        // Acquire Agent
        let llm_provider_id = full_job.parent_llm_provider_id.clone();
        let mut llm_provider_found = None;
        let mut profile_name = String::new();
        let mut user_profile: Option<ShinkaiName> = None;
        let llm_providers = JobManager::get_all_llm_providers(db).await.unwrap_or(vec![]);
        for llm_provider in llm_providers {
            if llm_provider.id == llm_provider_id {
                llm_provider_found = Some(llm_provider.clone());
                profile_name.clone_from(&llm_provider.full_identity_name.full_name);
                user_profile = Some(llm_provider.full_identity_name.extract_profile().unwrap());
                break;
            }
        }

        Ok((full_job, llm_provider_found, profile_name, user_profile))
    }

    pub async fn get_all_llm_providers(db: Arc<ShinkaiDB>) -> Result<Vec<SerializedLLMProvider>, ShinkaiDBError> {
        db.get_all_llm_providers()
    }
}
