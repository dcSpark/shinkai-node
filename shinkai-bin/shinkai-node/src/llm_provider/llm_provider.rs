use std::sync::Arc;

use crate::network::ws_manager::WSUpdateHandler;

use super::error::LLMProviderError;
use super::execution::chains::inference_chain_trait::LLMInferenceResponse;
use super::execution::prompts::prompts::Prompt;
use super::job::JobConfig;
use super::providers::LLMService;
use reqwest::Client;
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::{
    llm_providers::serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider},
    shinkai_name::ShinkaiName,
};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct LLMProvider {
    pub id: String,
    pub full_identity_name: ShinkaiName,
    pub client: Client,
    pub perform_locally: bool,        // Todo: Remove as not used anymore
    pub external_url: Option<String>, // external API URL
    pub api_key: Option<String>,
    pub model: LLMProviderInterface,
    pub toolkit_permissions: Vec<String>,        // Todo: remove as not used
    pub storage_bucket_permissions: Vec<String>, // Todo: remove as not used
    pub allowed_message_senders: Vec<String>,    // list of sub-identities allowed to message the llm provider
}

impl LLMProvider {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        full_identity_name: ShinkaiName,
        perform_locally: bool,
        external_url: Option<String>,
        api_key: Option<String>,
        model: LLMProviderInterface,
        toolkit_permissions: Vec<String>,
        storage_bucket_permissions: Vec<String>,
        allowed_message_senders: Vec<String>,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap();
        Self {
            id,
            full_identity_name,
            client,
            perform_locally,
            external_url,
            api_key,
            model,
            toolkit_permissions,
            storage_bucket_permissions,
            allowed_message_senders,
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
            Ok(response) => Ok(LLMInferenceResponse::new(content, response, None)),
            Err(_e) => Err(LLMProviderError::InferenceFailed),
        }
    }

    pub async fn inference(
        &self,
        prompt: Prompt,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
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
                        config,
                    )
                    .await
            }
            LLMProviderInterface::GenericAPI(genericapi) => {
                genericapi
                    .call_api(
                        &self.client,
                        self.external_url.as_ref(),
                        self.api_key.as_ref(),
                        prompt.clone(),
                        self.model.clone(),
                        inbox_name,
                        ws_manager_trait,
                        config,
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
                        config,
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
                    config,
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
                        config,
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
                    config,
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
                        config,
                    )
                    .await
            }
            LLMProviderInterface::LocalLLM(_local_llm) => {
                self.inference_locally(prompt.generate_single_output_string()?).await
            }
        }?;
        Ok(response)
    }
}

impl LLMProvider {
    pub fn from_serialized_llm_provider(serialized_llm_provider: SerializedLLMProvider) -> Self {
        Self::new(
            serialized_llm_provider.id,
            serialized_llm_provider.full_identity_name,
            serialized_llm_provider.perform_locally,
            serialized_llm_provider.external_url,
            serialized_llm_provider.api_key,
            serialized_llm_provider.model,
            serialized_llm_provider.toolkit_permissions,
            serialized_llm_provider.storage_bucket_permissions,
            serialized_llm_provider.allowed_message_senders,
        )
    }
}
