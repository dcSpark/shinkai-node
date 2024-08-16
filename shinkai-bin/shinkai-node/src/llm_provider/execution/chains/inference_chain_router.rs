use super::generic_chain::generic_inference_chain::GenericInferenceChain;
use super::inference_chain_trait::{InferenceChain, InferenceChainContext, InferenceChainResult};
use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job::Job;
use crate::llm_provider::job_manager::JobManager;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::sheet_manager::SheetManager;
use crate::network::ws_manager::WSUpdateHandler;
use crate::tools::tool_router::ToolRouter;
use crate::vector_fs::vector_fs::VectorFS;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::instrument;

impl JobManager {
    /// Chooses an inference chain based on the job message (using the agent's LLM)
    /// and then starts using the chosen chain.
    /// Returns the final String result from the inferencing, and a new execution context.
    #[instrument(skip(generator, vector_fs, db, ws_manager_trait, tool_router, sheet_manager))]
    #[allow(clippy::too_many_arguments)]
    pub async fn inference_chain_router(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        llm_provider_found: Option<SerializedLLMProvider>,
        full_job: Job,
        job_message: JobMessage,
        prev_execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
    ) -> Result<InferenceChainResult, LLMProviderError> {
        // Initializations
        let llm_provider = llm_provider_found.ok_or(LLMProviderError::LLMProviderNotFound)?;
        let max_tokens_in_prompt = ModelCapabilitiesManager::get_max_input_tokens(&llm_provider.model);
        let parsed_user_message = ParsedUserMessage::new(job_message.content.to_string());

        // Create the inference chain context
        let chain_context = InferenceChainContext::new(
            db,
            vector_fs,
            full_job,
            parsed_user_message,
            llm_provider,
            prev_execution_context,
            generator,
            user_profile,
            2,
            max_tokens_in_prompt,
            HashMap::new(),
            ws_manager_trait.clone(),
            tool_router.clone(),
            sheet_manager.clone(),
        );

        let mut generic_chain = GenericInferenceChain::new(chain_context, ws_manager_trait);
        generic_chain.run_chain().await
    }
}
