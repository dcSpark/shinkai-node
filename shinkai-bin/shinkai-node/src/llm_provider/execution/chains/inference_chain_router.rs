use super::generic_chain::generic_inference_chain::GenericInferenceChain;
use super::inference_chain_trait::{InferenceChain, InferenceChainContext, InferenceChainResult};
use super::sheet_ui_chain::sheet_ui_inference_chain::SheetUIInferenceChain;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use shinkai_message_primitives::schemas::job::Job;
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{AssociatedUI, JobMessage};
use shinkai_sqlite::SqliteManager;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};

impl JobManager {
    /// Chooses an inference chain based on the job message (using the agent's LLM)
    /// and then starts using the chosen chain.
    /// Returns the final String result from the inferencing.
    #[allow(clippy::too_many_arguments)]
    pub async fn inference_chain_router(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
        llm_provider_found: Option<ProviderOrAgent>,
        full_job: Job,
        job_message: JobMessage,
        message_hash_id: Option<String>,
        image_files: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<InferenceChainResult, LLMProviderError> {
        // Initializations
        let llm_provider = llm_provider_found.ok_or(LLMProviderError::LLMProviderNotFound)?;
        let model = {
            if let ProviderOrAgent::LLMProvider(llm_provider) = llm_provider.clone() {
                &llm_provider.model.clone()
            } else {
                // If it's an agent, we need to get the LLM provider from the agent
                let llm_id = llm_provider.get_llm_provider_id();
                let llm_provider = db
                    .read()
                    .await
                    .get_llm_provider(llm_id, &user_profile)
                    .map_err(|e| e.to_string())?
                    .ok_or(LLMProviderError::LLMProviderNotFound)?;
                &llm_provider.model.clone()
            }
        };
        let max_tokens_in_prompt = ModelCapabilitiesManager::get_max_input_tokens(&model);
        let parsed_user_message = ParsedUserMessage::new(job_message.content.to_string());

        // Create the inference chain context
        let chain_context = InferenceChainContext::new(
            db,
            vector_fs,
            full_job.clone(),
            parsed_user_message,
            job_message.tool_key,
            message_hash_id,
            image_files,
            llm_provider,
            generator,
            user_profile,
            3,
            max_tokens_in_prompt,
            ws_manager_trait.clone(),
            tool_router.clone(),
            sheet_manager.clone(),
            my_agent_payments_manager.clone(),
            ext_agent_payments_manager.clone(),
            // sqlite_logger.clone(),
            llm_stopper.clone(),
        );

        // Check for associated_ui and choose the appropriate chain
        if let Some(AssociatedUI::Sheet(sheet_string)) = &full_job.associated_ui {
            let mut sheet_ui_chain = SheetUIInferenceChain::new(chain_context, ws_manager_trait, sheet_string.clone());
            sheet_ui_chain.run_chain().await
        } else {
            let mut generic_chain = GenericInferenceChain::new(chain_context, ws_manager_trait);
            generic_chain.run_chain().await
        }
    }
}
