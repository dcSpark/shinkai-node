use super::generic_chain::generic_inference_chain::GenericInferenceChain;
use super::inference_chain_trait::{InferenceChain, InferenceChainContext, InferenceChainResult};
use super::sheet_ui_chain::sheet_ui_inference_chain::SheetUIInferenceChain;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::job::Job;
use shinkai_sqlite::SqliteLogger;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use shinkai_db::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{AssociatedUI, JobMessage};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

impl JobManager {
    /// Chooses an inference chain based on the job message (using the agent's LLM)
    /// and then starts using the chosen chain.
    /// Returns the final String result from the inferencing, and a new execution context.
    #[allow(clippy::too_many_arguments)]
    pub async fn inference_chain_router(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        llm_provider_found: Option<SerializedLLMProvider>,
        full_job: Job,
        job_message: JobMessage,
        message_hash_id: Option<String>,
        image_files: HashMap<String, String>,
        prev_execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<InferenceChainResult, LLMProviderError> {
        // Initializations
        let llm_provider = llm_provider_found.ok_or(LLMProviderError::LLMProviderNotFound)?;
        let max_tokens_in_prompt = ModelCapabilitiesManager::get_max_input_tokens(&llm_provider.model);
        let parsed_user_message = ParsedUserMessage::new(job_message.content.to_string());

        // Create the inference chain context
        let chain_context = InferenceChainContext::new(
            db,
            vector_fs,
            full_job.clone(),
            parsed_user_message,
            message_hash_id,
            image_files,
            llm_provider,
            prev_execution_context,
            generator,
            user_profile,
            3,
            max_tokens_in_prompt,
            ws_manager_trait.clone(),
            tool_router.clone(),
            sheet_manager.clone(),
            my_agent_payments_manager.clone(),
            ext_agent_payments_manager.clone(),
            sqlite_logger.clone(),
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
