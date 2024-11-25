use super::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_provider::LLMProvider;
use crate::llm_provider::llm_stopper::LLMStopper;
use shinkai_db::db::db_errors::ShinkaiDBError;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job::Job;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::{SqliteManager, SqliteManagerError};
use std::result::Result::Ok;
use std::sync::Arc;
use tokio::sync::Mutex;

impl JobManager {
    /// Inferences the Agent's LLM with the given prompt.
    pub async fn inference_with_llm_provider(
        llm_provider: ProviderOrAgent,
        filled_prompt: Prompt,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
        db: Arc<SqliteManager>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let llm_provider_cloned = llm_provider.clone();
        let prompt_cloned = filled_prompt.clone();

        let task_response = tokio::spawn(async move {
            let llm_provider = LLMProvider::from_provider_or_agent(llm_provider_cloned, db.clone())?;
            llm_provider
                .inference(prompt_cloned, inbox_name, ws_manager_trait, config, llm_stopper)
                .await
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
        sqlite_manager: Arc<SqliteManager>,
    ) -> Result<(Job, Option<ProviderOrAgent>, String, Option<ShinkaiName>), LLMProviderError> {
        // Fetch the job
        let full_job = { db.get_job(job_id)? };

        // Acquire Agent
        let agent_or_llm_provider_id = full_job.parent_agent_or_llm_provider_id.clone();
        let mut agent_or_llm_provider_found = None;
        let mut profile_name = String::new();
        let mut user_profile: Option<ShinkaiName> = None;
        let agents_and_llm_providers = JobManager::get_all_agents_and_llm_providers(sqlite_manager)
            .await
            .unwrap_or(vec![]);
        for agent_or_llm_provider in agents_and_llm_providers {
            if agent_or_llm_provider.get_id() == &agent_or_llm_provider_id {
                agent_or_llm_provider_found = Some(agent_or_llm_provider.clone());
                profile_name.clone_from(&agent_or_llm_provider.get_full_identity_name().full_name);
                user_profile = Some(
                    agent_or_llm_provider
                        .get_full_identity_name()
                        .extract_profile()
                        .unwrap(),
                );
                break;
            }
        }

        Ok((full_job, agent_or_llm_provider_found, profile_name, user_profile))
    }

    pub async fn get_all_agents_and_llm_providers(
        db: Arc<SqliteManager>,
    ) -> Result<Vec<ProviderOrAgent>, SqliteManagerError> {
        let llm_providers = db.get_all_llm_providers()?;
        let agents = db.get_all_agents()?;

        let mut providers_and_agents = Vec::new();

        for llm_provider in llm_providers {
            providers_and_agents.push(ProviderOrAgent::LLMProvider(llm_provider));
        }

        for agent in agents {
            providers_and_agents.push(ProviderOrAgent::Agent(agent));
        }

        Ok(providers_and_agents)
    }
}
