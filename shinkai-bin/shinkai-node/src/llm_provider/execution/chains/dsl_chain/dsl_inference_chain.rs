use std::{any::Any, collections::HashMap, env, fmt, marker::PhantomData, sync::Arc, time::Instant};

use crate::{
    llm_provider::execution::{
        chains::inference_chain_trait::InferenceChainContextTrait, prompts::general_prompts::JobPromptGenerator,
        user_message_parser::ParsedUserMessage,
    },
    managers::model_capabilities_manager::ModelCapabilitiesManager,
    workflows::sm_executor::{AsyncFunction, FunctionMap, WorkflowEngine, WorkflowError},
};
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use regex::Regex;
use shinkai_baml::baml_builder::{BamlConfig, ClientConfig, GeneratorConfig};
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxName, job::JobLike},
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use shinkai_sqlite::logger::{WorkflowLogEntry, WorkflowLogEntryStatus};
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiTool, workflow_tool::WorkflowTool};
use shinkai_vector_resources::{embeddings::Embedding, vector_resource::RetrievedNode};
use tokio::sync::RwLock;

use crate::llm_provider::{
    error::LLMProviderError,
    execution::chains::inference_chain_trait::{InferenceChain, InferenceChainResult},
    job_manager::JobManager,
};

use super::{
    generic_functions::{self},
    split_text_for_llm::{split_text_at_token_limit, split_text_for_llm},
};
use std::collections::VecDeque;

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
        let logs = Arc::new(RwLock::new(VecDeque::new()));

        // Inject user_message into $R0
        let user_message = self.context.user_message().clone().original_user_message_string;
        final_registers.insert("$INPUT".to_string(), user_message.clone());

        // Log the $INPUT
        {
            let mut logs_write = logs.write().await;
            logs_write.push_back(WorkflowLogEntry {
                subprocess: Some("$INPUT".to_string()),
                input: Some(user_message.clone()),
                additional_info: "User message injected".to_string(),
                timestamp: Utc::now(),
                status: WorkflowLogEntryStatus::Success("Input logged".to_string()),
                result: None,
            });
        }

        let executor = engine.iter(
            &self.workflow_tool.workflow,
            Some(final_registers.clone()),
            Some(logs.clone()), // Pass the updated logs
        );

        for result in executor {
            match result {
                Ok(registers) => {
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

        // Log the $RESULT
        {
            let mut logs_write = logs.write().await;
            logs_write.push_back(WorkflowLogEntry {
                subprocess: Some("$RESULT".to_string()),
                input: None,
                additional_info: "Final result obtained".to_string(),
                timestamp: Utc::now(),
                status: WorkflowLogEntryStatus::Success("Result logged".to_string()),
                result: Some(response_register.clone()),
            });
        }

        let new_context = HashMap::new();

        // Clean up the response_register using regex
        let re = Regex::new(r"\\n").unwrap();
        let cleaned_response = re.replace_all(&response_register, "\n").to_string();

        // Convert logs to a Vec and then to a serde_json::Value
        let logs_vec: Vec<WorkflowLogEntry> = logs.read().await.iter().cloned().collect();
        let logs_json = serde_json::to_value(logs_vec).unwrap_or_else(|_| serde_json::Value::Null);
        println!("Logs as JSON: {}", logs_json);

        // Debug Code
        // Write logs_json to a file
        if let Ok(logs_string) = serde_json::to_string_pretty(&logs_json) {
            if let Err(e) = std::fs::write("logs.json", logs_string) {
                eprintln!("Failed to write logs to file: {}", e);
            }
        } else {
            eprintln!("Failed to convert logs to string");
        }

        // Logging to SQLite
        if let Some(logger) = self.context.sqlite_logger() {
            let message_id = self.context.message_hash_id().unwrap_or_default();
            let workflow = self.workflow_tool.workflow.clone();
            let result = logger.log_workflow_execution(message_id, workflow, logs).await;
            println!("Logged workflow execution: {:?}", result);
        }

        Ok(InferenceChainResult::new(cleaned_response, new_context))
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

    pub fn add_baml_inference_function(&mut self) {
        self.functions.insert(
            "baml_inference".to_string(),
            Box::new(BamlInference {
                context: self.context.clone_box(),
                use_ws_manager: true,
            }),
        );
    }

    pub fn add_opinionated_inference_function(&mut self) {
        self.functions.insert(
            "opinionated_inference".to_string(),
            Box::new(OpinionatedInferenceFunction {
                context: self.context.clone_box(),
                use_ws_manager: true,
            }),
        );
    }

    pub fn add_opinionated_inference_no_ws_function(&mut self) {
        self.functions.insert(
            "opinionated_inference_no_ws".to_string(),
            Box::new(OpinionatedInferenceFunction {
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

    pub async fn add_tools_from_router(&mut self, js_tools: Vec<ShinkaiTool>) -> Result<(), WorkflowError> {
        let start_time = Instant::now();

        for tool in js_tools {
            let function_name = tool.name();
            self.functions.insert(
                function_name,
                Box::new(ShinkaiToolFunction {
                    tool: tool.clone(),
                    context: self.context.clone_box(),
                }),
            );
        }

        let elapsed_time = start_time.elapsed(); // Measure elapsed time
        if env::var("LOG_ALL").unwrap_or_default() == "1" {
            eprintln!("Time taken to add tools: {:?}", elapsed_time);
        }

        Ok(())
    }

    pub fn add_all_generic_functions(&mut self) {
        self.add_generic_function("concat", |context, args| {
            generic_functions::concat_strings(&*context, args)
        });
        self.add_generic_function("generate_json_map", |context, args| {
            generic_functions::generate_json_map(&*context, args)
        });
        self.add_generic_function("search_and_replace", |context, args| {
            generic_functions::search_and_replace(&*context, args)
        });
        self.add_generic_function("count_files_from_input", |context, args| {
            generic_functions::count_files_from_input(&*context, args)
        });
        self.add_generic_function("retrieve_file_from_input", |context, args| {
            generic_functions::retrieve_file_from_input(&*context, args)
        });
        self.add_generic_function("process_embeddings_in_job_scope", |context, args| {
            generic_functions::process_embeddings_in_job_scope(&*context, args)
        });
        self.add_generic_function("process_embeddings_in_job_scope_with_metadata", |context, args| {
            generic_functions::process_embeddings_in_job_scope_with_metadata(&*context, args)
        });
        self.add_generic_function("search_embeddings_in_job_scope", |context, args| {
            generic_functions::search_embeddings_in_job_scope(&*context, args)
        });
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

        let custom_system_prompt: Option<String> = args.get(1).and_then(|arg| arg.downcast_ref::<String>().cloned());
        let custom_user_prompt: Option<String> = args.get(2).and_then(|arg| arg.downcast_ref::<String>().cloned());

        let full_job = self.context.full_job();
        let llm_provider = self.context.agent();

        // TODO: extract files from args

        // TODO: add more debugging to (ie add to logs) the diff operations
        let filled_prompt = JobPromptGenerator::generic_inference_prompt(
            custom_system_prompt,
            custom_user_prompt,
            user_message.clone(),
            HashMap::new(),
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
            None, // this is the config
            self.context.llm_stopper().clone(),
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

        let custom_system_prompt: Option<String> = args.get(1).and_then(|arg| arg.downcast_ref::<String>().cloned());
        let custom_user_prompt: Option<String> = args.get(2).and_then(|arg| arg.downcast_ref::<String>().cloned());

        let db = self.context.db();
        let vector_fs = self.context.vector_fs();
        let full_job = self.context.full_job();
        let llm_provider = self.context.agent();
        let generator = self.context.generator();
        let user_profile = self.context.user_profile();
        let max_tokens_in_prompt = self.context.max_tokens_in_prompt();

        // If both the scope and custom_system_prompt are not empty, we use an empty string
        // for the user_message and the custom_system_prompt as the query_text.
        // This allows for more focused searches based on the system prompt when a scope is provided.
        let (effective_user_message, query_text) = if !full_job.scope_with_files().unwrap().is_empty() && custom_system_prompt.is_some() {
            ("".to_string(), custom_system_prompt.clone().unwrap_or_default())
        } else {
            (user_message.clone(), user_message.clone())
        };

        // TODO: add more debugging to (ie add to logs) the diff operations
        // TODO: extract files from args

        // If we need to search for nodes using the scope
        let scope_is_empty = full_job.scope_with_files().unwrap().is_empty();
        let mut ret_nodes: Vec<RetrievedNode> = vec![];
        let mut summary_node_text = None;
        if !scope_is_empty {
            // TODO: this should also be a generic fn
            let (ret, summary) = JobManager::keyword_chained_job_scope_vector_search(
                db.clone(),
                vector_fs.clone(),
                full_job.scope_with_files().unwrap(),
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
            custom_system_prompt,
            custom_user_prompt,
            effective_user_message.clone(),
            HashMap::new(),
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
            None, // this is the config
            self.context.llm_stopper().clone(),
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
struct BamlInference {
    context: Box<dyn InferenceChainContextTrait>,
    use_ws_manager: bool,
}

#[async_trait]
impl AsyncFunction for BamlInference {
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        let user_message = args[0]
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?
            .clone();
        eprintln!("BamlInference> user_message: {:?}", user_message);

        // TODO: connect them
        // let custom_system_prompt: Option<String> = args.get(1).and_then(|arg| arg.downcast_ref::<String>().cloned());
        // let custom_user_prompt: Option<String> = args.get(2).and_then(|arg| arg.downcast_ref::<String>().cloned());

        let dsl_class_file: Option<String> = args.get(3).and_then(|arg| arg.downcast_ref::<String>().cloned());
        let dsl_class_file = BamlConfig::convert_dsl_class_file(&dsl_class_file.unwrap_or_default());
        eprintln!("BamlInference> dsl_class_file: {:?}", dsl_class_file);

        let fn_name: Option<String> = args.get(4).and_then(|arg| arg.downcast_ref::<String>().cloned());
        eprintln!("BamlInference> fn_name: {:?}", fn_name);

        let param_name: Option<String> = args.get(5).and_then(|arg| arg.downcast_ref::<String>().cloned());
        eprintln!("BamlInference> param_name: {:?}", param_name);

        // TODO: do we need the job for something?
        // let full_job = self.context.full_job();
        let llm_provider = self.context.agent();

        let generator_config = GeneratorConfig::default();

        // TODO: add support for other providers
        let base_url = llm_provider.external_url.clone().unwrap_or_default();
        let base_url = if base_url == "http://localhost:11434" || base_url == "http://localhost:11435" {
            format!("{}/v1", base_url)
        } else {
            base_url
        };

        let client_config = ClientConfig {
            provider: llm_provider.get_provider_string(),
            base_url,
            model: llm_provider.get_model_string(),
            default_role: "user".to_string(),
        };
        eprintln!("BamlInference> client_config: {:?}", client_config);

        // Note(nico): we need to pass the env vars from the job here if we are using an LLM behind an API
        let env_vars = HashMap::new();

        // Prepare BAML execution
        let baml_config = BamlConfig::builder(generator_config, client_config)
            .dsl_class_file(&dsl_class_file)
            .input(&user_message)
            .function_name(&fn_name.unwrap_or_default())
            .param_name(&param_name.unwrap_or_default())
            .build();

        let runtime = baml_config
            .initialize_runtime(env_vars)
            .map_err(|e| WorkflowError::ExecutionError(format!("Failed to initialize BAML runtime: {}", e)))?;

        // Measure time taken for BAML execution
        let start_time = Instant::now();

        // Execute BAML using spawn_blocking
        let result = match tokio::task::spawn_blocking(move || baml_config.execute(&runtime, true)).await {
            Ok(res) => match res {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("BAML execution failed: {}", e);
                    return Err(WorkflowError::ExecutionError(format!("BAML execution failed: {}", e)));
                }
            },
            Err(e) => {
                eprintln!("Failed to execute function: {}", e);
                return Err(WorkflowError::ExecutionError(format!(
                    "Failed to execute function: {}",
                    e
                )));
            }
        };

        let elapsed_time = start_time.elapsed();
        eprintln!("Time taken for BAML execution: {:?}", elapsed_time);

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("BAML Inference Answer: {:?}", result).as_str(),
        );

        Ok(Box::new(result))
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

        let custom_system_prompt: Option<String> = args.get(2).and_then(|arg| arg.downcast_ref::<String>().cloned());
        let custom_user_prompt: Option<String> = args.get(3).and_then(|arg| arg.downcast_ref::<String>().cloned());

        let split_result = split_text_for_llm(self.context.as_ref(), args)?;
        let split_texts = split_result
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid split result".to_string()))?
            .split(":::")
            .map(|s| s.replace(":::", ""))
            .collect::<Vec<String>>();

        // If everything fits in one message
        if split_texts.len() == 1 {
            let inference_args = vec![
                Box::new(split_texts[0].clone()) as Box<dyn Any + Send>,
                Box::new(custom_system_prompt.clone()) as Box<dyn Any + Send>,
                Box::new(custom_user_prompt.clone()) as Box<dyn Any + Send>,
            ];
            let response = self.inference_function_ws.call(inference_args).await?;
            let response_text = response
                .downcast_ref::<String>()
                .ok_or_else(|| WorkflowError::ExecutionError("Invalid response from inference".to_string()))?
                .replace(":::", "");
            return Ok(Box::new(response_text));
        }

        let mut responses = Vec::new();
        let agent = self.context.agent();
        let max_tokens = ModelCapabilitiesManager::get_max_input_tokens(&agent.model);

        for text in split_texts.iter() {
            let inference_args = vec![
                Box::new(text.clone()) as Box<dyn Any + Send>,
                Box::new(custom_system_prompt.clone()) as Box<dyn Any + Send>,
                Box::new(custom_user_prompt.clone()) as Box<dyn Any + Send>,
            ];
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
        let result = match &self.tool {
            ShinkaiTool::JS(js_tool, _) => {
                let params = parse_params(args)?;
                eprintln!("params: {:?}", params);

                for arg in self.tool.input_args().iter() {
                    if !params.contains_key(&arg.name) {
                        if arg.is_required {
                            return Err(WorkflowError::InvalidArgument(format!(
                                "Missing required argument: {}",
                                arg.name
                            )));
                        }
                        continue;
                    }
                }

                let function_config = self.tool.get_config_from_env();
                let result = js_tool
                    .run(params, function_config)
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
            ShinkaiTool::Rust(_, _) => {
                // Note: shouldn't rust tools be supported?
                return Err(WorkflowError::ExecutionError(
                    "Rust tools are not supported in this context".to_string(),
                ));
            }
            ShinkaiTool::Workflow(workflow, _is_enabled) => {
                let arg = args[0].downcast_ref::<String>().ok_or_else(|| {
                    WorkflowError::InvalidArgument("Expected a single argument of type String".to_string())
                })?;

                let mut new_context = self.context.clone();
                new_context.update_message(ParsedUserMessage::new(arg.clone()));

                // Create a new DslChain for the nested workflow
                let functions = HashMap::new();
                let mut nested_dsl_inference = DslChain::new(new_context, workflow.workflow.clone(), functions);

                // TODO: read the fns from the workflow code and the missing ones from the tool router

                // Add necessary functions to the nested DslChain
                nested_dsl_inference.add_inference_function();
                nested_dsl_inference.add_inference_no_ws_function();
                nested_dsl_inference.add_baml_inference_function();
                nested_dsl_inference.add_opinionated_inference_function();
                nested_dsl_inference.add_opinionated_inference_no_ws_function();
                nested_dsl_inference.add_multi_inference_function();
                nested_dsl_inference.add_all_generic_functions();

                // Run the nested workflow
                let result = nested_dsl_inference
                    .run_chain()
                    .await
                    .map_err(|e| WorkflowError::ExecutionError(format!("Nested workflow execution failed: {}", e)))?;

                eprintln!("result nested workflow: {:?}", result.response);

                result.response
            }
            ShinkaiTool::Network(_, _) => {
                // TODO: we should allow for a workflow to call another workflow
                return Err(WorkflowError::ExecutionError(
                    "Network Tools are not supported in this context".to_string(),
                ));
            }
        };

        Ok(Box::new(result))
    }
}

fn parse_params(args: Vec<Box<dyn Any + Send>>) -> Result<serde_json::Map<String, serde_json::Value>, WorkflowError> {
    let mut params = serde_json::Map::new();

    if args.len() == 1 {
        // Check if the single argument is a JSON string
        let arg = args[0]
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Expected a single argument of type String".to_string()))?;

        // Try to parse the argument as a JSON value
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(arg) {
            match json_value {
                serde_json::Value::Object(json_map) => {
                    // If it's a JSON object, use it as the params map
                    params = json_map;
                }
                serde_json::Value::Array(json_array) => {
                    // If it's a JSON array, convert it to a single parameter
                    params.insert("arg".to_string(), serde_json::Value::Array(json_array));
                }
                _ => {
                    return Err(WorkflowError::InvalidArgument(
                        "Expected a JSON object or array".to_string(),
                    ));
                }
            }
        } else {
            // If not a JSON string, treat it as a single parameter
            params.insert("arg".to_string(), serde_json::Value::String(arg.clone()));
        }
    } else {
        // Handle multiple arguments
        if args.len() % 2 != 0 {
            return Err(WorkflowError::InvalidArgument(
                "Expected an even number of arguments".to_string(),
            ));
        }

        // Iterate through the arguments in pairs
        for pair in args.chunks(2) {
            let key = pair[0]
                .downcast_ref::<String>()
                .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for key".to_string()))?;
            let value = pair[1]
                .downcast_ref::<String>()
                .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument for value".to_string()))?;

            // Check if the value is a JSON string and parse it
            if value.starts_with('{') && value.ends_with('}') {
                if let Ok(parsed_value) = serde_json::from_str::<serde_json::Value>(value) {
                    // Serialize the parsed JSON value back to a string
                    let serialized_value = serde_json::to_string(&parsed_value).map_err(|e| {
                        WorkflowError::InvalidArgument(format!("Failed to serialize JSON value: {}", e))
                    })?;
                    params.insert(key.clone(), serde_json::Value::String(serialized_value));
                } else {
                    return Err(WorkflowError::InvalidArgument("Failed to parse JSON value".to_string()));
                }
            } else {
                // Insert each key-value pair into the params map
                params.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }
    }

    Ok(params)
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
