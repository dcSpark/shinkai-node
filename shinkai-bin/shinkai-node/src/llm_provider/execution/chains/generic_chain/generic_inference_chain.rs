use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::dsl_chain::generic_functions::RustToolFunctions;
use crate::llm_provider::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainContextTrait, InferenceChainResult,
};
use crate::llm_provider::execution::prompts::prompts::JobPromptGenerator;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job::{Job, JobLike};
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::providers::shared::openai::{FunctionCall, FunctionCallResponse};
use crate::network::ws_manager::WSUpdateHandler;
use crate::tools::argument::ToolArgument;
use crate::tools::router::ShinkaiTool;
use crate::tools::rust_tools::RustTool;
use crate::vector_fs::vector_fs::VectorFS;
use async_recursion::async_recursion;
use async_trait::async_trait;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::vector_resource::RetrievedNode;
use std::any::Any;
use std::fmt;
use std::result::Result::Ok;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::instrument;

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
            self.context.max_iterations,
            self.context.max_tokens_in_prompt,
            self.ws_manager_trait.clone(),
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
    #[instrument(skip(generator, vector_fs, db, ws_manager_trait))]
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
        // if let LLMProviderInterface::OpenAI(openai) = &llm_provider.model.clone() {
        //     // Perform the specific action for OpenAI models
        //     // delete
        //     let concat_strings_desc = "Concatenates 2 to 4 strings.".to_string();
        //     let tool = RustTool::new(
        //         "concat_strings".to_string(),
        //         concat_strings_desc.clone(),
        //         vec![
        //             ToolArgument::new(
        //                 "first_string".to_string(),
        //                 "string".to_string(),
        //                 "The first string to concatenate".to_string(),
        //                 true,
        //             ),
        //             ToolArgument::new(
        //                 "second_string".to_string(),
        //                 "string".to_string(),
        //                 "The second string to concatenate".to_string(),
        //                 true,
        //             ),
        //             ToolArgument::new(
        //                 "third_string".to_string(),
        //                 "string".to_string(),
        //                 "The third string to concatenate (optional)".to_string(),
        //                 false,
        //             ),
        //             ToolArgument::new(
        //                 "fourth_string".to_string(),
        //                 "string".to_string(),
        //                 "The fourth string to concatenate (optional)".to_string(),
        //                 false,
        //             ),
        //         ],
        //         generator
        //             .generate_embedding_default(&concat_strings_desc)
        //             .await
        //             .unwrap(),
        //     );
        //     tools.push(ShinkaiTool::Rust(tool));
        //     // end delete
        // }

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
            let response_res =
                JobManager::inference_with_llm_provider(llm_provider.clone(), filled_prompt.clone(), ws_manager_trait.clone())
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
                );

                // 6) Call workflow or tooling
                let function_response = Self::call_function(function_call, &context).await?;

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

    async fn call_function(
        function_call: FunctionCall,
        context: &dyn InferenceChainContextTrait,
    ) -> Result<FunctionCallResponse, LLMProviderError> {
        // TODO: Update to support JS -- It's only for rust for now

        // Extract function name and arguments from the function_call
        let function_name = function_call.name.clone();
        let function_args = function_call.arguments.clone();

        eprintln!("function_name: {:?}", function_name);
        eprintln!("function_args: {:?}", function_args);

        // Find the function in the tool map
        let tool_function = RustToolFunctions::get_tool_function(&function_name)
            .ok_or_else(|| LLMProviderError::FunctionNotFound(function_name.clone()))?;

        // Convert arguments to the required format
        let args: Vec<Box<dyn Any + Send>> = match function_args {
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .map(|arg| match arg {
                    serde_json::Value::String(s) => Box::new(s) as Box<dyn Any + Send>,
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Box::new(i) as Box<dyn Any + Send>
                        } else if let Some(f) = n.as_f64() {
                            Box::new(f) as Box<dyn Any + Send>
                        } else {
                            Box::new(n.to_string()) as Box<dyn Any + Send>
                        }
                    }
                    serde_json::Value::Bool(b) => Box::new(b) as Box<dyn Any + Send>,
                    _ => Box::new(arg.to_string()) as Box<dyn Any + Send>,
                })
                .collect(),
            serde_json::Value::Object(map) => map
                .into_iter()
                .map(|(_, value)| match value {
                    serde_json::Value::String(s) => Box::new(s) as Box<dyn Any + Send>,
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Box::new(i) as Box<dyn Any + Send>
                        } else if let Some(f) = n.as_f64() {
                            Box::new(f) as Box<dyn Any + Send>
                        } else {
                            Box::new(n.to_string()) as Box<dyn Any + Send>
                        }
                    }
                    serde_json::Value::Bool(b) => Box::new(b) as Box<dyn Any + Send>,
                    _ => Box::new(value.to_string()) as Box<dyn Any + Send>,
                })
                .collect(),
            _ => {
                return Err(LLMProviderError::InvalidFunctionArguments(format!(
                    "Invalid arguments: {:?}",
                    function_args
                )))
            }
        };

        // Call the function
        let result =
            tool_function(context, args).map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;

        // Convert the result back to a string (assuming the result is a string)
        let result_str = result
            .downcast_ref::<String>()
            .ok_or_else(|| LLMProviderError::InvalidFunctionResult(format!("Invalid result: {:?}", result)))?
            .clone();

        Ok(FunctionCallResponse {
            response: result_str,
            function_call,
        })
    }
}
