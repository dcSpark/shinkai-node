use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainResult,
};
use crate::llm_provider::execution::prompts::prompts::JobPromptGenerator;
use crate::llm_provider::job::{Job, JobLike};
use crate::llm_provider::job_manager::JobManager;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use async_trait::async_trait;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct GenericInferenceChain {
    pub context: InferenceChainContext,
    // maybe add a new variable to hold a enum that allow for workflows and tools?
    // maybe another one for custom prompting? (so we can run customizedagents)
    // maybe something for general state of the prompt (useful if we are using tooling / workflows)
    // maybe something for websockets so we can send tokens as we get them
    // extend to allow for image(s) as well as inputs and outputs. New Enum?
}

#[async_trait]
impl InferenceChain for GenericInferenceChain {
    fn chain_id() -> String {
        "generic_inference_chain".to_string()
    }

    fn chain_context(&mut self) -> &mut InferenceChainContext {
        &mut self.context
    }

    async fn run_chain(&mut self) -> Result<InferenceChainResult, LLMProviderError> {
        let response = GenericInferenceChain::start_chain(
            self.context.db.clone(),
            self.context.vector_fs.clone(),
            self.context.full_job.clone(),
            self.context.user_message.original_user_message_string.to_string(),
            self.context.llm_provider.clone(),
            self.context.execution_context.clone(),
            self.context.generator.clone(),
            self.context.user_profile.clone(),
            0,
            self.context.max_iterations,
            self.context.max_tokens_in_prompt,
        )
        .await?;
        let job_execution_context = self.context.execution_context.clone();
        Ok(InferenceChainResult::new(response, job_execution_context))
    }
}

impl GenericInferenceChain {
    pub fn new(context: InferenceChainContext) -> Self {
        Self { context }
    }

    #[async_recursion]
    #[instrument(skip(generator, vector_fs, db))]
    #[allow(clippy::too_many_arguments)]
    pub async fn start_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: String,
        agent: SerializedLLMProvider,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        iteration_count: u64,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
    ) -> Result<String, LLMProviderError> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("start_generic_inference_chain>  message: {:?}", user_message),
        );

        /*
        How it (should) work:

        1) Vector search for knowledge if the scope isn't empty
        2) Vector search for tooling / workflows if the workflow / tooling scope isn't empty
        3) Generate Prompt
        4) Call LLM
        5) Check response if it requires a function call
        6) (as required) Call workflow or tooling
        7) (as required) Call LLM again with the response (for formatting)
        8) (as required) back to 5)
        9) (profit) return response

        Note: we need to handle errors and retry
        */

        // 1) Vector search for knowledge if the scope isn't empty
        let scope_is_empty = full_job.scope().is_empty();
        let mut ret_nodes: Vec<RetrievedNode> = vec![];
        let mut summary_node_text = None;
        if !scope_is_empty {
            let (ret, summary) = JobManager::keyword_chained_job_scope_vector_search(
                db.clone(),
                vector_fs.clone(),
                full_job.scope(),
                user_message.clone(),
                &user_profile,
                generator.clone(),
                20,
                max_tokens_in_prompt,
            )
            .await?;
            ret_nodes = ret;
            summary_node_text = summary;
        }

        // 2) Vector search for tooling / workflows if the workflow / tooling scope isn't empty
        // WIP

        // 3) Generate Prompt
        let filled_prompt = JobPromptGenerator::generic_inference_prompt(
            None, // TODO: connect later on
            None, // TODO: connect later on
            user_message.clone(),
            ret_nodes,
            summary_node_text,
            Some(full_job.step_history.clone()),
        );

        // 4) Call LLM
        let response_res = JobManager::inference_with_llm_provider(agent.clone(), filled_prompt.clone()).await;

        // TODO: modify LLMInferenceResponse so it holds more information e.g. function call required, etc. choices, etc.
        // TODO: modify inference_with_llm_provider (or create a new one) that can take some extra information so it can stream tokens out
        // TODO: handle errors and potential retry (depending on the error)

        // Previous code
        // // Check if it failed to produce a proper json object at all, and if so go through more advanced retry logic
        // if let Err(LLMProviderError::LLMServiceInferenceLimitReached(e)) = &response_res {
        //     return Err(LLMProviderError::LLMServiceInferenceLimitReached(e.to_string()));
        // } else if let Err(LLMProviderError::LLMServiceUnexpectedError(e)) = &response_res {
        //     return Err(LLMProviderError::LLMServiceUnexpectedError(e.to_string()));

        let answer = response_res?.original_response_string;
        Ok(answer)
    }
}
