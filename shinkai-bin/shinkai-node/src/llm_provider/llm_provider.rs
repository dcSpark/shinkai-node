use std::sync::Arc;

use super::error::LLMProviderError;
use super::execution::chains::inference_chain_trait::LLMInferenceResponse;
use super::llm_stopper::LLMStopper;
use super::providers::LLMService;
use reqwest::Client;
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::agent::Agent;
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::schemas::{
    llm_providers::serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider},
    shinkai_name::ShinkaiName,
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct LLMProvider {
    pub id: String,
    pub full_identity_name: ShinkaiName,
    pub client: Client,
    pub external_url: Option<String>, // external API URL
    pub api_key: Option<String>,
    pub model: LLMProviderInterface,
    pub agent: Option<Agent>,
    pub db: Arc<SqliteManager>,
}

impl LLMProvider {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        full_identity_name: ShinkaiName,
        external_url: Option<String>,
        api_key: Option<String>,
        model: LLMProviderInterface,
        agent: Option<Agent>,
        db: Arc<SqliteManager>,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 min TTFT
            .build()
            .unwrap();
        Self {
            id,
            full_identity_name,
            client,
            external_url,
            api_key,
            model,
            agent,
            db,
        }
    }

    /// Inferences an LLM locally based on info held in the LLM Provider
    /// TODO: For now just mocked, eventually get around to this, and create a struct that implements the Provider trait to unify local with remote interface.
    async fn inference_locally(&self, content: String) -> Result<LLMInferenceResponse, LLMProviderError> {
        // Here we run our GPU-intensive task on a separate thread
        let handle = tokio::task::spawn_blocking(move || {
            let mut map = Map::new();
            map.insert(
                "answer".to_string(),
                JsonValue::String("\n\nHello there, how may I assist you today?".to_string()),
            );
            JsonValue::Object(map)
        });

        match handle.await {
            Ok(response) => Ok(LLMInferenceResponse::new(content, response, Vec::new(), None)),
            Err(_e) => Err(LLMProviderError::InferenceFailed),
        }
    }

    pub async fn inference(
        &self,
        prompt: Prompt,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        // Merge config with agent's config, preferring the provided config
        let merged_config = if let Some(agent) = &self.agent {
            if let Some(agent_config) = &agent.config {
                // Prefer `config` over `agent_config`
                Some(config.unwrap_or_else(JobConfig::empty).merge(agent_config))
            } else {
                // Use provided config or create an empty one if none is provided
                config.or_else(|| Some(JobConfig::empty()))
            }
        } else {
            // Use provided config if no agent is present
            config
        };

        let response = match &self.model {
            LLMProviderInterface::OpenAI(openai) => {
                openai
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
            LLMProviderInterface::TogetherAI(togetherai) => {
                togetherai
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
            LLMProviderInterface::Ollama(ollama) => {
                ollama
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
            LLMProviderInterface::Exo(exo) => {
                exo.call_api(
                    &self.client,
                    self.external_url.as_ref(),
                    self.api_key.as_ref(),
                    prompt.clone(),
                    self.model.clone(),
                    inbox_name,
                    ws_manager_trait,
                    merged_config,
                    llm_stopper,
                    self.db.clone(),
                )
                .await
            }
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => {
                shinkai_backend
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
            LLMProviderInterface::Groq(groq) => {
                groq.call_api(
                    &self.client,
                    self.external_url.as_ref(),
                    self.api_key.as_ref(),
                    prompt.clone(),
                    self.model.clone(),
                    inbox_name,
                    ws_manager_trait,
                    merged_config,
                    llm_stopper,
                    self.db.clone(),
                )
                .await
            }
            LLMProviderInterface::Gemini(gemini) => {
                gemini
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
            LLMProviderInterface::OpenRouter(openrouter) => {
                openrouter
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
            LLMProviderInterface::Claude(claude) => {
                claude
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
            LLMProviderInterface::LocalLLM(_local_llm) => {
                self.inference_locally(prompt.generate_single_output_string()?).await
            }
            LLMProviderInterface::LocalRegex(local_regex) => {
                local_regex
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        merged_config,
                        llm_stopper,
                        self.db.clone(),
                    )
                    .await
            }
        }?;
        Ok(response)
    }
}

impl LLMProvider {
    pub fn from_serialized_llm_provider(serialized_llm_provider: SerializedLLMProvider, db: Arc<SqliteManager>) -> Self {
        Self::new(
            serialized_llm_provider.id,
            serialized_llm_provider.full_identity_name,
            serialized_llm_provider.external_url,
            serialized_llm_provider.api_key,
            serialized_llm_provider.model,
            None,
            db,
        )
    }

    pub async fn from_provider_or_agent(
        provider_or_agent: ProviderOrAgent,
        db: Arc<SqliteManager>,
    ) -> Result<Self, LLMProviderError> {
        match provider_or_agent {
            ProviderOrAgent::LLMProvider(serialized_llm_provider) => {
                Ok(Self::from_serialized_llm_provider(serialized_llm_provider, db))
            }
            ProviderOrAgent::Agent(agent) => {
                let llm_id = &agent.llm_provider_id;
                let llm_provider = db
                    .get_llm_provider(llm_id, &agent.full_identity_name)
                    .map_err(|_e| LLMProviderError::AgentNotFound(llm_id.clone()))?;
                if let Some(llm_provider) = llm_provider {
                    Ok(Self::from_serialized_llm_provider(llm_provider, db))
                } else {
                    Err(LLMProviderError::AgentNotFound(llm_id.clone()))
                }
            }
        }
    }
}
