use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::job::{Job, JobLike};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainContextTrait, InferenceChainResult,
};
use crate::llm_provider::execution::chains::sheet_ui_chain::sheet_rust_functions::SheetRustFunctions;
use crate::llm_provider::execution::prompts::general_prompts::JobPromptGenerator;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::shared::openai::FunctionCallResponse;
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use shinkai_db::schemas::ws_types::WSUpdateHandler;
use async_trait::async_trait;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::any::Any;
use std::fmt;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tokio::task;

#[derive(Clone)]
pub struct SheetUIInferenceChain {
    pub context: InferenceChainContext,
    pub ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    pub sheet_id: String,
}

impl fmt::Debug for SheetUIInferenceChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SheetUIInferenceChain")
            .field("context", &self.context)
            .field("ws_manager_trait", &self.ws_manager_trait.is_some())
            .finish()
    }
}

#[async_trait]
impl InferenceChain for SheetUIInferenceChain {
    fn chain_id() -> String {
        "sheet_ui_inference_chain".to_string()
    }

    fn chain_context(&mut self) -> &mut dyn InferenceChainContextTrait {
        &mut self.context
    }

    async fn run_chain(&mut self) -> Result<InferenceChainResult, LLMProviderError> {
        let response = SheetUIInferenceChain::start_chain(
            self.context.db.clone(),
            self.context.vector_fs.clone(),
            self.context.full_job.clone(),
            self.context.user_message.original_user_message_string.to_string(),
            self.context.image_files.clone(),
            self.context.llm_provider.clone(),
            self.context.execution_context.clone(),
            self.context.generator.clone(),
            self.context.user_profile.clone(),
            self.context.max_iterations,
            self.context.max_tokens_in_prompt,
            self.ws_manager_trait.clone(),
            self.context.tool_router.clone(),
            self.context.sheet_manager.clone(),
            self.sheet_id.clone(),
            self.context.my_agent_payments_manager.clone(),
            self.context.ext_agent_payments_manager.clone(),
            self.context.llm_stopper.clone(),
        )
        .await?;
        let job_execution_context = self.context.execution_context.clone();
        Ok(InferenceChainResult::new(response, job_execution_context))
    }
}

impl SheetUIInferenceChain {
    pub fn new(
        context: InferenceChainContext,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        sheet_id: String,
    ) -> Self {
        Self {
            context,
            ws_manager_trait,
            sheet_id,
        }
    }

    // Note: this code is very similar to the one from Generic, maybe we could inject
    // the tool code handling in the future so we can reuse the code
    #[allow(clippy::too_many_arguments)]
    pub async fn start_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: String,
        image_files: HashMap<String, String>,
        llm_provider: SerializedLLMProvider,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        sheet_id: String,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<String, LLMProviderError> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("start_sheet_ui_inference_chain>  message: {:?}", user_message),
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
            tools.extend(SheetRustFunctions::sheet_rust_fn());

            if let Some(tool_router) = &tool_router {
                // TODO: enable back the default tools (must tools)
                // // Get default tools
                // if let Ok(default_tools) = tool_router.get_default_tools(&user_profile) {
                //     tools.extend(default_tools);
                // }

                // Search in JS Tools
                let results = tool_router
                    .vector_search_enabled_tools(&user_message.clone(), 2)
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
        let job_config = full_job.config();
        let custom_prompt = job_config.and_then(|config| config.custom_prompt.clone());

        let mut filled_prompt = JobPromptGenerator::generic_inference_prompt(
            custom_prompt,
            None, // TODO: connect later on
            user_message.clone(),
            image_files.clone(),
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
                job_config.cloned(),
                llm_stopper.clone(),
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
                // 6) Call workflow or tooling
                // Find the ShinkaiTool that has a tool with the function name
                let shinkai_tool = tools.iter().find(|tool| tool.name() == function_call.name);
                if shinkai_tool.is_none() {
                    eprintln!("Function not found: {}", function_call.name);
                    return Err(LLMProviderError::FunctionNotFound(function_call.name.clone()));
                }

                // Check if the tool is Rust-based or JS/workflow
                let function_response = if shinkai_tool.unwrap().is_rust_based() {
                    // Rust-based tool
                    let function = SheetRustFunctions::get_tool_function(function_call.name.clone());
                    if function.is_none() {
                        eprintln!("Function not found: {}", function_call.name);
                        return Err(LLMProviderError::FunctionNotFound(function_call.name.clone()));
                    }

                    let function = function.unwrap();
                    let sheet_manager_clone = sheet_manager.clone().unwrap();
                    let sheet_id_clone = sheet_id.clone();
                    let mut args = HashMap::new();
                    if let Some(arguments) = function_call.arguments.as_object() {
                        for (key, value) in arguments {
                            let mut val = value.to_string();
                            if val.starts_with('"') && val.ends_with('"') {
                                val = val.strip_prefix('"').unwrap().strip_suffix('"').unwrap().to_string();
                            }
                            args.insert(key.clone(), Box::new(val) as Box<dyn Any + Send>);
                        }
                    } else {
                        return Err(LLMProviderError::InvalidFunctionArguments(
                            "Function arguments should be a JSON object".to_string(),
                        ));
                    }

                    let handle = task::spawn(async move { function(sheet_manager_clone, sheet_id_clone, args).await });

                    let response = match handle.await {
                        Ok(Ok(response)) => response,
                        Ok(Err(e)) => {
                            eprintln!("Error calling function: {:?}", e);
                            return Err(LLMProviderError::FunctionExecutionError(e));
                        }
                        Err(e) => {
                            eprintln!("Task join error: {:?}", e);
                            return Err(LLMProviderError::FunctionExecutionError(e.to_string()));
                        }
                    };

                    FunctionCallResponse {
                        response,
                        function_call: function_call.clone(),
                    }
                } else {
                    let parsed_message = ParsedUserMessage::new(user_message.clone());
                    let context = InferenceChainContext::new(
                        db.clone(),
                        vector_fs.clone(),
                        full_job.clone(),
                        parsed_message,
                        image_files.clone(),
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
                        llm_stopper.clone(),
                    );
                    // JS or workflow tool
                    match tool_router
                        .as_ref()
                        .unwrap()
                        .call_function(function_call, &context, shinkai_tool.unwrap())
                        .await
                    {
                        Ok(response) => response,
                        Err(e) => {
                            eprintln!("Error calling function: {:?}", e);
                            return Err(e);
                        }
                    }
                };

                // 7) Call LLM again with the response (for formatting)
                filled_prompt = JobPromptGenerator::generic_inference_prompt(
                    None, // TODO: connect later on
                    None, // TODO: connect later on
                    user_message.clone(),
                    image_files.clone(),
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
