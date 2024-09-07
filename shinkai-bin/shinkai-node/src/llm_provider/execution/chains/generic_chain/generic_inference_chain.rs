use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainContextTrait, InferenceChainResult,
};
use crate::llm_provider::execution::prompts::prompts::JobPromptGenerator;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job::{Job, JobLike};
use crate::llm_provider::job_manager::JobManager;
use crate::managers::sheet_manager::SheetManager;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use crate::network::ws_manager::WSUpdateHandler;
use crate::tools::tool_router::ToolRouter;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use async_trait::async_trait;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::fmt;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct GenericInferenceChain {
    pub context: InferenceChainContext,
    pub ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    // maybe add a new variable to hold a enum that allow for workflows and tools?
    // maybe another one for custom prompting? (so we can run customizedagents)
    // maybe something for general state of the prompt (useful if we are using tooling / workflows)
    // maybe something for websockets so we can send tokens as we get them
    // extend to allow for image(s) as well as inputs and outputs. New Enum?
}

impl fmt::Debug for GenericInferenceChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenericInferenceChain")
            .field("context", &self.context)
            .field("ws_manager_trait", &self.ws_manager_trait.is_some())
            .finish()
    }
}

#[async_trait]
impl InferenceChain for GenericInferenceChain {
    fn chain_id() -> String {
        "generic_inference_chain".to_string()
    }

    fn chain_context(&mut self) -> &mut dyn InferenceChainContextTrait {
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
            self.context.max_iterations,
            self.context.max_tokens_in_prompt,
            self.ws_manager_trait.clone(),
            self.context.tool_router.clone(),
            self.context.sheet_manager.clone(),
            self.context.my_agent_payments_manager.clone(),
            self.context.ext_agent_payments_manager.clone(),
        )
        .await?;
        let job_execution_context = self.context.execution_context.clone();
        Ok(InferenceChainResult::new(response, job_execution_context))
    }
}

impl GenericInferenceChain {
    pub fn new(
        context: InferenceChainContext,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Self {
        Self {
            context,
            ws_manager_trait,
        }
    }

    #[async_recursion]
    #[allow(clippy::too_many_arguments)]
    pub async fn start_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: String,
        llm_provider: SerializedLLMProvider,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
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
        // Only for OpenAI right now
        let mut tools = vec![];
        if let LLMProviderInterface::OpenAI(_openai) = &llm_provider.model.clone() {
            if let Some(tool_router) = &tool_router {
                

                // TODO: enable back the default tools (must tools)
                // // Get default tools
                // if let Ok(default_tools) = tool_router.get_default_tools(&user_profile) {
                //     tools.extend(default_tools);
                // }

                // Search in JS Tools
                let results = tool_router
                    .vector_search_enabled_tools_with_network(&user_message.clone(), 5)
                    .await
                    .unwrap();
                for result in results {
                    if let Some(tool) = tool_router.get_tool_by_name(&result.tool_router_key).await.unwrap() {
                        tools.push(tool);
                    }
                }
            }
        }

        // 3) Generate Prompt
        let mut filled_prompt = JobPromptGenerator::generic_inference_prompt(
            None, // TODO: connect later on
            None, // TODO: connect later on
            user_message.clone(),
            ret_nodes.clone(),
            summary_node_text.clone(),
            Some(full_job.step_history.clone()),
            tools.clone(),
            None,
        );

        let mut iteration_count = 0;
        loop {
            // Check if max_iterations is reached
            if iteration_count >= max_iterations {
                return Err(LLMProviderError::MaxIterationsReached(
                    "Maximum iterations reached".to_string(),
                ));
            }

            // 4) Call LLM
            let inbox_name: Option<InboxName> = match InboxName::get_job_inbox_name_from_params(full_job.job_id.clone())
            {
                Ok(name) => Some(name),
                Err(_) => None,
            };
            let response_res = JobManager::inference_with_llm_provider(
                llm_provider.clone(),
                filled_prompt.clone(),
                inbox_name,
                ws_manager_trait.clone(),
            )
            .await;

            // Error Codes
            if let Err(LLMProviderError::LLMServiceInferenceLimitReached(e)) = &response_res {
                return Err(LLMProviderError::LLMServiceInferenceLimitReached(e.to_string()));
            } else if let Err(LLMProviderError::LLMServiceUnexpectedError(e)) = &response_res {
                return Err(LLMProviderError::LLMServiceUnexpectedError(e.to_string()));
            }

            let response = response_res?;

            // 5) Check response if it requires a function call
            if let Some(function_call) = response.function_call {
                let parsed_message = ParsedUserMessage::new(user_message.clone());
                let context = InferenceChainContext::new(
                    db.clone(),
                    vector_fs.clone(),
                    full_job.clone(),
                    parsed_message,
                    llm_provider.clone(),
                    execution_context.clone(),
                    generator.clone(),
                    user_profile.clone(),
                    max_iterations,
                    max_tokens_in_prompt,
                    HashMap::new(),
                    ws_manager_trait.clone(),
                    tool_router.clone(),
                    sheet_manager.clone(),
                    my_agent_payments_manager.clone(),
                    ext_agent_payments_manager.clone(),
                );

                // 6) Call workflow or tooling
                // Find the ShinkaiTool that has a tool with the function name
                let shinkai_tool = tools.iter().find(|tool| tool.name() == function_call.name);
                if shinkai_tool.is_none() {
                    eprintln!("Function not found: {}", function_call.name);
                    return Err(LLMProviderError::FunctionNotFound(function_call.name.clone()));
                }

                // Note: here we can add logic to handle the case that we have network tools

                // TODO: if shinkai_tool is None we need to retry with the LLM (hallucination)
                let function_response = match tool_router
                    .as_ref()
                    .unwrap()
                    .call_function(function_call, &context, shinkai_tool.unwrap())
                    .await
                {
                    Ok(response) => response,
                    Err(e) => {
                        eprintln!("Error calling function: {:?}", e);
                        // Handle different error types here if needed
                        return Err(e);
                    }
                };

                // 7) Call LLM again with the response (for formatting)
                filled_prompt = JobPromptGenerator::generic_inference_prompt(
                    None, // TODO: connect later on
                    None, // TODO: connect later on
                    user_message.clone(),
                    ret_nodes.clone(),
                    summary_node_text.clone(),
                    Some(full_job.step_history.clone()),
                    tools.clone(),
                    Some(function_response),
                );
            } else {
                // No more function calls required, return the final response
                return Ok(response.response_string);
            }

            // Increment the iteration count
            iteration_count += 1;
        }
    }
}
