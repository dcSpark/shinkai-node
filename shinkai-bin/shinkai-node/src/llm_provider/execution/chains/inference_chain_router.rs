use super::generic_chain::generic_inference_chain::GenericInferenceChain;
use super::inference_chain_trait::{InferenceChain, InferenceChainContext, InferenceChainResult};
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_message_primitives::schemas::job::Job;
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_sqlite::SqliteManager;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

impl JobManager {
    /// Chooses an inference chain based on the job message (using the agent's
    /// LLM) and then starts using the chosen chain.
    /// Returns the final String result from the inferencing, and a new
    /// execution context.
    #[allow(clippy::too_many_arguments)]
    pub async fn inference_chain_router(
        db: Arc<SqliteManager>,
        llm_provider_found: Option<ProviderOrAgent>,
        full_job: Job,
        job_message: JobMessage,
        message_hash_id: Option<String>,
        image_files: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        job_callback_manager: Arc<Mutex<JobCallbackManager>>,
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
                    .get_llm_provider(llm_id, &user_profile)
                    .map_err(|e| e.to_string())?
                    .ok_or(LLMProviderError::LLMProviderNotFound)?;
                &llm_provider.model.clone()
            }
        };
        let max_tokens_in_prompt = ModelCapabilitiesManager::get_max_input_tokens(&model);
        let parsed_user_message = ParsedUserMessage::new(job_message.content.to_string());

        // Get max_iterations from preferences, default to 10 if not found
        // Try first as u64, then as String (in case it's stored as a string)
        let max_iterations = match db.get_preference::<u64>("max_iterations") {
            Ok(Some(value)) => value,
            _ => {
                // If it fails or is None, try as string and parse
                match db.get_preference::<String>("max_iterations") {
                    Ok(Some(str_value)) => str_value.parse::<u64>().unwrap_or(10),
                    _ => 10, // Default if nothing works
                }
            }
        };

        // Create the inference chain context
        let chain_context = InferenceChainContext::new(
            db,
            full_job.clone(),
            parsed_user_message,
            job_message.tool_key,
            job_message.tools,
            job_message.fs_files_paths,
            job_message.job_filenames,
            message_hash_id,
            image_files,
            llm_provider,
            generator,
            user_profile,
            max_iterations,
            max_tokens_in_prompt,
            ws_manager_trait.clone(),
            tool_router.clone(),
            my_agent_payments_manager.clone(),
            ext_agent_payments_manager.clone(),
            Some(job_callback_manager.clone()),
            // sqlite_logger.clone(),
            llm_stopper.clone(),
        );

        // Check for associated_ui and choose the appropriate chain (check AssociatedUI)
        let mut generic_chain = GenericInferenceChain::new(chain_context, ws_manager_trait);
        generic_chain.run_chain().await
    }
}
