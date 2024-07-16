use std::{any::Any, collections::HashMap, fmt, marker::PhantomData};

use crate::{
    llm_provider::{
        execution::chains::inference_chain_trait::InferenceChainContextTrait, job::JobLike,
        providers::shared::openai::FunctionCall,
    },
    managers::model_capabilities_manager::ModelCapabilitiesManager,
    tools::{shinkai_tool::ShinkaiTool, workflow_tool::WorkflowTool},
};
use async_trait::async_trait;
use dashmap::DashMap;
use shinkai_dsl::{
    dsl_schemas::Workflow,
    sm_executor::{AsyncFunction, FunctionMap, WorkflowEngine, WorkflowError},
};
use shinkai_message_primitives::{
    schemas::inbox_name::InboxName,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use shinkai_vector_resources::{embeddings::Embedding, vector_resource::RetrievedNode};

use crate::llm_provider::{
    error::LLMProviderError,
    execution::{
        chains::inference_chain_trait::{InferenceChain, InferenceChainResult},
        prompts::prompts::JobPromptGenerator,
    },
    job_manager::JobManager,
};

use super::{
    generic_functions,
    split_text_for_llm::{split_text_at_token_limit, split_text_for_llm},
};

pub struct DslChain<'a> {
    pub context: Box<dyn InferenceChainContextTrait>,
    pub workflow_tool: WorkflowTool,
    pub functions: FunctionMap<'a>,
}

impl<'a> fmt::Debug for DslChain<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DslChain")
            .field("workflow_tool", &self.workflow_tool)
            .field("functions", &"<functions>")
            .finish()
    }
}

#[async_trait]
impl<'a> InferenceChain for DslChain<'a> {
    fn chain_id() -> String {
        "dsl_chain".to_string()
    }

    fn chain_context(&mut self) -> &mut dyn InferenceChainContextTrait {
        &mut *self.context
    }

    async fn run_chain(&mut self) -> Result<InferenceChainResult, LLMProviderError> {
        let engine = WorkflowEngine::new(&self.functions);
        let mut final_registers = DashMap::new();
        let logs = DashMap::new();

        // Inject user_message into $R0
        final_registers.insert(
            "$INPUT".to_string(),
            self.context.user_message().clone().original_user_message_string,
        );
        let executor = engine.iter(
            &self.workflow_tool.workflow,
            Some(final_registers.clone()),
            Some(logs.clone()),
        );

        for result in executor {
            match result {
                Ok(registers) => {
                    // Is this required if we are passing a dashmap reference?
                    final_registers = registers;
                }
                Err(e) => {
                    eprintln!("Error in workflow engine: {}", e);
                    return Err(LLMProviderError::WorkflowExecutionError(e.to_string()));
                }
            }
        }

        let response_register = final_registers
            .get("$RESULT")
            .map(|r| r.clone())
            .unwrap_or_else(String::new);
        let new_contenxt = HashMap::new();

        // Debug
        // let logs = WorkflowEngine::formatted_logs(&logs);

        Ok(InferenceChainResult::new(response_register, new_contenxt))
    }
}

impl<'a> DslChain<'a> {
    pub fn new(context: Box<dyn InferenceChainContextTrait>, workflow: Workflow, functions: FunctionMap<'a>) -> Self {
        Self {
            context,
            workflow_tool: WorkflowTool {
                workflow,
                embedding: None,
            },
            functions,
        }
    }

    pub fn update_embedding(&mut self, new_embedding: Embedding) {
        self.workflow_tool.embedding = Some(new_embedding);
    }

    pub fn add_inference_function(&mut self) {
        self.functions.insert(
            "inference".to_string(),
            Box::new(InferenceFunction {
                context: self.context.clone_box(),
                use_ws_manager: true,
            }),
        );
    }

    pub fn add_inference_no_ws_function(&mut self) {
        self.functions.insert(
            "inference_no_ws".to_string(),
            Box::new(InferenceFunction {
                context: self.context.clone_box(),
                use_ws_manager: false,
            }),
        );
    }

    pub fn add_multi_inference_function(&mut self) {
        self.functions.insert(
            "multi_inference".to_string(),
            Box::new(MultiInferenceFunction {
                context: self.context.clone_box(),
                inference_function_ws: InferenceFunction {
                    context: self.context.clone_box(),
                    use_ws_manager: true,
                },
                inference_function_no_ws: InferenceFunction {
                    context: self.context.clone_box(),
                    use_ws_manager: false,
                },
            }),
        );
    }

    pub fn add_generic_function<F>(&mut self, name: &str, func: F)
    where
        F: Fn(
                Box<dyn InferenceChainContextTrait>,
                Vec<Box<dyn Any + Send>>,
            ) -> Result<Box<dyn Any + Send>, WorkflowError>
            + Send
            + Sync
            + Clone
            + 'a,
    {
        self.functions.insert(
            name.to_string(),
            Box::new(GenericFunction {
                func,
                context: self.context.clone_box(),
                _marker: PhantomData,
            }),
        );
    }

    pub async fn add_tools_from_router(&mut self) -> Result<(), WorkflowError> {
        let tool_router = self
            .context
            .tool_router()
            .as_ref()
            .ok_or_else(|| WorkflowError::ExecutionError("ToolRouter not available".to_string()))?
            .clone();

        let tool_router_locked = tool_router.lock().await;

        let tools = tool_router_locked
            .all_available_js_tools(self.context.user_profile(), self.context.db().clone())
            .map_err(|e| WorkflowError::ExecutionError(format!("Failed to fetch tools: {}", e)))?;

        for tool in tools {
            let function_name = format!("{}_{}", tool.toolkit_name(), tool.name());
            eprintln!("add_tools_from_router> Adding function: {}", function_name.clone());
            self.functions.insert(
                function_name.clone(),
                Box::new(ShinkaiToolFunction {
                    tool: tool.clone(),
                    context: self.context.clone_box(),
                }),
            );
        }

        Ok(())
    }

    pub fn add_all_generic_functions(&mut self) {
        self.add_generic_function("concat", |context, args| {
            generic_functions::concat_strings(&*context, args)
        });
        self.add_generic_function("search_and_replace", |context, args| {
            generic_functions::search_and_replace(&*context, args)
        });
        self.add_generic_function("download_webpage", |context, args| {
            generic_functions::download_webpage(&*context, args)
        });
        self.add_generic_function("html_to_markdown", |context, args| {
            generic_functions::html_to_markdown(&*context, args)
        });
        self.add_generic_function("fill_variable_in_md_template", |context, args| {
            generic_functions::fill_variable_in_md_template(&*context, args)
        });
        self.add_generic_function("array_to_markdown_template", |context, args| {
            generic_functions::array_to_markdown_template(&*context, args)
        });
        // self.add_generic_function("print_arg", |context, args| {
        //     generic_functions::print_arg(&*context, args)
        // });
        self.add_generic_function("count_files_from_input", |context, args| {
            generic_functions::count_files_from_input(&*context, args)
        });
        self.add_generic_function("retrieve_file_from_input", |context, args| {
            generic_functions::retrieve_file_from_input(&*context, args)
        });
        self.add_generic_function("extract_and_map_csv_column", |context, args| {
            generic_functions::extract_and_map_csv_column(&*context, args)
        });
        self.add_generic_function("process_embeddings_in_job_scope", |context, args| {
            generic_functions::process_embeddings_in_job_scope(&*context, args)
        });
        // TODO: add for local search of nodes (embeddings)
        // TODO: add for parse into chunks a text (so it fits in the context length of the model)
    }
}

#[derive(Clone)]
struct InferenceFunction {
    context: Box<dyn InferenceChainContextTrait>,
    use_ws_manager: bool,
}

#[async_trait]
impl AsyncFunction for InferenceFunction {
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        let user_message = args[0]
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?
            .clone();

        let full_job = self.context.full_job();
        let llm_provider = self.context.agent();

        // TODO: add more debugging to (ie add to logs) the diff operations
        let filled_prompt = JobPromptGenerator::generic_inference_prompt(
            None, // TODO: connect later on
            None, // TODO: connect later on
            user_message.clone(),
            vec![],
            None,
            None,
            vec![],
            None,
        );

        // Handle response_res without using the `?` operator
        let inbox_name: Option<InboxName> = match InboxName::get_job_inbox_name_from_params(full_job.job_id.clone()) {
            Ok(name) => Some(name),
            Err(_) => None,
        };
        let response = JobManager::inference_with_llm_provider(
            llm_provider.clone(),
            filled_prompt.clone(),
            inbox_name,
            if self.use_ws_manager {
                self.context.ws_manager_trait()
            } else {
                None
            },
        )
        .await
        .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;

        let answer = response.response_string;

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("Inference Answer: {:?}", answer.clone()).as_str(),
        );

        Ok(Box::new(answer))
    }
}

#[derive(Clone)]
struct OpinionatedInferenceFunction {
    context: Box<dyn InferenceChainContextTrait>,
    use_ws_manager: bool,
}

#[async_trait]
impl AsyncFunction for OpinionatedInferenceFunction {
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        let user_message = args[0]
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?
            .clone();

        let db = self.context.db();
        let vector_fs = self.context.vector_fs();
        let full_job = self.context.full_job();
        let llm_provider = self.context.agent();
        let generator = self.context.generator();
        let user_profile = self.context.user_profile();
        let max_tokens_in_prompt = self.context.max_tokens_in_prompt();

        let query_text = user_message.clone();

        // TODO: add more debugging to (ie add to logs) the diff operations

        // If we need to search for nodes using the scope
        let scope_is_empty = full_job.scope().is_empty();
        let mut ret_nodes: Vec<RetrievedNode> = vec![];
        let mut summary_node_text = None;
        if !scope_is_empty {
            // TODO: this should also be a generic fn
            let (ret, summary) = JobManager::keyword_chained_job_scope_vector_search(
                db.clone(),
                vector_fs.clone(),
                full_job.scope(),
                query_text.clone(),
                user_profile,
                generator.clone(),
                20,
                max_tokens_in_prompt,
            )
            .await
            .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
            ret_nodes = ret;
            summary_node_text = summary;
        }

        let filled_prompt = JobPromptGenerator::generic_inference_prompt(
            None, // TODO: connect later on
            None, // TODO: connect later on
            user_message.clone(),
            ret_nodes,
            summary_node_text,
            Some(full_job.step_history.clone()),
            vec![],
            None,
        );

        // Handle response_res without using the `?` operator
        let inbox_name: Option<InboxName> = match InboxName::get_job_inbox_name_from_params(full_job.job_id.clone()) {
            Ok(name) => Some(name),
            Err(_) => None,
        };
        let response = JobManager::inference_with_llm_provider(
            llm_provider.clone(),
            filled_prompt.clone(),
            inbox_name,
            if self.use_ws_manager {
                self.context.ws_manager_trait()
            } else {
                None
            },
        )
        .await
        .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;

        let answer = response.response_string;

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("Inference Answer: {:?}", answer.clone()).as_str(),
        );

        Ok(Box::new(answer))
    }
}

#[derive(Clone)]
struct MultiInferenceFunction {
    context: Box<dyn InferenceChainContextTrait>,
    inference_function_ws: InferenceFunction,
    inference_function_no_ws: InferenceFunction,
}

#[async_trait]
impl AsyncFunction for MultiInferenceFunction {
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        let first_argument = args[0]
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for first argument".to_string()))?
            .clone();

        let split_result = split_text_for_llm(self.context.as_ref(), args)?;
        let split_texts = split_result
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid split result".to_string()))?
            .split(":::")
            .map(|s| s.replace(":::", ""))
            .collect::<Vec<String>>();

        let mut responses = Vec::new();
        let agent = self.context.agent();
        let max_tokens = ModelCapabilitiesManager::get_max_input_tokens(&agent.model);

        for text in split_texts.iter() {
            let inference_args = vec![Box::new(text.clone()) as Box<dyn Any + Send>];
            let response = self.inference_function_no_ws.call(inference_args).await?;
            let response_text = response
                .downcast_ref::<String>()
                .ok_or_else(|| WorkflowError::ExecutionError("Invalid response from inference".to_string()))?
                .replace(":::", "");
            responses.push(response_text);
        }

        // Perform one more inference with all the responses together
        let combined_responses = responses.join(" ");
        let combined_token_count = ModelCapabilitiesManager::count_tokens_from_message_llama3(&combined_responses);
        let max_safe_tokens = (max_tokens as f64 * 0.8).ceil() as usize;

        let final_text = if combined_token_count > max_safe_tokens {
            let (part, _) = split_text_at_token_limit(&combined_responses, max_safe_tokens, combined_token_count);
            part
        } else {
            combined_responses
        };

        // Concatenate the first argument with the final text for the final inference call
        let concatenated_final_text = format!("{}{}", first_argument, final_text);
        let final_inference_args = vec![Box::new(concatenated_final_text) as Box<dyn Any + Send>];
        let final_response = self.inference_function_ws.call(final_inference_args).await?;
        let final_response_text = final_response
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::ExecutionError("Invalid final response from inference".to_string()))?
            .replace(":::", "");

        Ok(Box::new(final_response_text))
    }
}

#[derive(Clone)]
struct ShinkaiToolFunction {
    tool: ShinkaiTool,
    context: Box<dyn InferenceChainContextTrait>,
}

#[async_trait]
impl AsyncFunction for ShinkaiToolFunction {
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        if args.len() != 1 {
            return Err(WorkflowError::InvalidArgument(
                "Expected 1 argument: function_args".to_string(),
            ));
        }

        let mut params = serde_json::Map::new();

        // Iterate through the tool's input_args and the provided args
        for (i, arg) in self.tool.input_args().iter().enumerate() {
            if i >= args.len() {
                if arg.is_required {
                    return Err(WorkflowError::InvalidArgument(format!(
                        "Missing required argument: {}",
                        arg.name
                    )));
                }
                continue;
            }

            let value = args[i]
                .downcast_ref::<String>()
                .ok_or_else(|| WorkflowError::InvalidArgument(format!("Invalid argument for {}", arg.name)))?;

            params.insert(arg.name.clone(), serde_json::Value::String(value.clone()));
        }

        let function_call = FunctionCall {
            name: self.tool.name(),
            arguments: serde_json::Value::Object(params),
        };

        let result = match &self.tool {
            ShinkaiTool::JS(js_tool) => {
                let result = js_tool
                    .run(function_call.arguments)
                    .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
                let data = &result.data;

                // Check if the result has only one main type
                let main_result = if let Some(main_type) = js_tool.result.properties.as_object().and_then(|props| {
                    if props.len() == 1 {
                        props.keys().next().cloned()
                    } else {
                        None
                    }
                }) {
                    data.get(&main_type).cloned().unwrap_or_else(|| data.clone())
                } else {
                    data.clone()
                };

                match main_result {
                    serde_json::Value::String(s) => s,
                    _ => serde_json::to_string(&main_result)
                        .map_err(|e| WorkflowError::ExecutionError(format!("Failed to stringify result: {}", e)))?,
                }
            }
            ShinkaiTool::JSLite(_) => {
                return Err(WorkflowError::ExecutionError(
                    "Simplified JS tools are not supported in this context".to_string(),
                ));
            }
            ShinkaiTool::Rust(_) => {
                return Err(WorkflowError::ExecutionError(
                    "Rust tools are not supported in this context".to_string(),
                ));
            }
            ShinkaiTool::Workflow(_) => {
                // TODO: we should allow for a workflow to call another workflow
                return Err(WorkflowError::ExecutionError(
                    "Workflows are not supported in this context".to_string(),
                ));
            }
        };

        Ok(Box::new(result))
    }
}

#[allow(dead_code)]
#[derive(Clone)]
struct GenericFunction<F>
where
    F: Fn(Box<dyn InferenceChainContextTrait>, Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError>
        + Send
        + Sync
        + Clone,
{
    func: F,
    context: Box<dyn InferenceChainContextTrait>,
    _marker: PhantomData<fn() -> F>,
}

#[async_trait]
impl<F> AsyncFunction for GenericFunction<F>
where
    F: Fn(Box<dyn InferenceChainContextTrait>, Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError>
        + Send
        + Sync
        + Clone,
{
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        (self.func)(self.context.clone(), args)
    }
}
