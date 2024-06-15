use std::{any::Any, collections::HashMap, fmt, marker::PhantomData};

use crate::agent::{execution::chains::inference_chain_trait::InferenceChainContextTrait, job::JobLike};
use async_trait::async_trait;
use dashmap::DashMap;
use shinkai_dsl::{
    dsl_schemas::Workflow,
    sm_executor::{AsyncFunction, FunctionMap, WorkflowEngine, WorkflowError},
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::vector_resource::RetrievedNode;

use crate::agent::{
    error::AgentError,
    execution::{
        chains::inference_chain_trait::{InferenceChain, InferenceChainContext, InferenceChainResult},
        prompts::prompts::JobPromptGenerator,
    },
    job_manager::JobManager,
};

use super::generic_functions;

pub struct DslChain<'a> {
    pub context: InferenceChainContext,
    pub workflow: Workflow,
    pub functions: FunctionMap<'a>,
}

impl<'a> fmt::Debug for DslChain<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DslChain")
            .field("context", &self.context)
            .field("workflow", &self.workflow)
            .field("functions", &"<functions>")
            .finish()
    }
}

#[async_trait]
impl<'a> InferenceChain for DslChain<'a> {
    fn chain_id() -> String {
        "dsl_chain".to_string()
    }

    fn chain_context(&mut self) -> &mut InferenceChainContext {
        &mut self.context
    }

    async fn run_chain(&mut self) -> Result<InferenceChainResult, AgentError> {
        let engine = WorkflowEngine::new(&self.functions);
        let mut final_registers = DashMap::new();        

        // Inject user_message into $R0
        final_registers.insert("$R0".to_string(), self.context.user_message.clone().original_user_message_string);
        let executor = engine.iter(&self.workflow, Some(final_registers.clone()));

        for result in executor {
            match result {
                Ok(registers) => {
                    // Is this required if we are passing a dashmap reference?
                    final_registers = registers;
                }
                Err(e) => {
                    eprintln!("Error in workflow engine: {}", e);
                    return Err(AgentError::WorkflowExecutionError(e.to_string()));
                }
            }
        }

        let response_register = final_registers
            .get("$R1")
            .map(|r| r.clone())
            .unwrap_or_else(String::new);
        let new_contenxt = HashMap::new();
        Ok(InferenceChainResult::new(response_register, new_contenxt))
    }
}

impl<'a> DslChain<'a> {
    pub fn new(context: InferenceChainContext, workflow: Workflow, functions: FunctionMap<'a>) -> Self {
        Self {
            context,
            workflow,
            functions,
        }
    }

    pub fn add_inference_function(&mut self) {
        self.functions.insert(
            "inference".to_string(),
            Box::new(InferenceFunction {
                context: self.context.clone(),
            }),
        );
    }

    pub fn add_generic_function<F>(&mut self, name: &str, func: F)
    where
        F: Fn(Box<dyn InferenceChainContextTrait>, Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> + Send + Sync + 'a,
    {
        self.functions.insert(
            name.to_string(),
            Box::new(GenericFunction {
                func,
                context: Box::new(self.context.clone()) as Box<dyn InferenceChainContextTrait>,
                _marker: PhantomData,
            }),
        );
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
        self.add_generic_function("print_arg", |context, args| {
            generic_functions::print_arg(&*context, args)
        });
        self.add_generic_function("count_files_from_input", |context, args| {
            generic_functions::count_files_from_input(&*context, args)
        });
        self.add_generic_function("retrieve_file_from_input", |context, args| {
            generic_functions::retrieve_file_from_input(&*context, args)
        });
        self.add_generic_function("extract_and_map_csv_column", |context, args| {
            generic_functions::extract_and_map_csv_column(&*context, args)
        });
        // TODO: add for local search of nodes (embeddings)
        // TODO: add for parse into chunks a text (so it fits in the context length of the model)
    }
}

struct InferenceFunction {
    context: InferenceChainContext,
}

#[async_trait]
impl AsyncFunction for InferenceFunction {
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        let user_message = args[0]
            .downcast_ref::<String>()
            .ok_or_else(|| WorkflowError::InvalidArgument("Invalid argument".to_string()))?
            .clone();
        eprintln!("InferenceFunction: {:?}", user_message);

        let db = self.context.db.clone();
        let vector_fs = self.context.vector_fs.clone();
        let full_job = self.context.full_job.clone();
        let agent = self.context.agent.clone();
        let generator = self.context.generator.clone();
        let user_profile = self.context.user_profile.clone();
        let max_tokens_in_prompt = self.context.max_tokens_in_prompt;

        let query_text = user_message.clone();

        // If we need to search for nodes using the scope
        let scope_is_empty = full_job.scope().is_empty();
        let mut ret_nodes: Vec<RetrievedNode> = vec![];
        if !scope_is_empty {
            // TODO: this should also be a generic fn
            let (ret, _summary) = JobManager::keyword_chained_job_scope_vector_search(
                db.clone(),
                vector_fs.clone(),
                full_job.scope(),
                query_text.clone(),
                &user_profile,
                generator.clone(),
                20,
                max_tokens_in_prompt,
            )
            .await
            .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;
            ret_nodes = ret;
        }

        let filled_prompt = JobPromptGenerator::qa_response_prompt_with_vector_search(
            user_message.clone(),
            ret_nodes,
            None,
            Some(query_text),
            Some(full_job.step_history.clone()),
        );

        // Handle response_res without using the `?` operator
        let response = JobManager::inference_agent_markdown(agent.clone(), filled_prompt.clone())
            .await
            .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;

        let answer = JobManager::direct_extract_key_inference_response(response.clone(), "answer")
            .map_err(|e| WorkflowError::ExecutionError(e.to_string()))?;

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("Inference Answer: {:?}", answer.clone()).as_str(),
        );

        Ok(Box::new(answer))
    }
}

#[allow(dead_code)]
struct GenericFunction<F>
where
    F: Fn(Box<dyn InferenceChainContextTrait>, Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> + Send + Sync,
{
    func: F,
    context: Box<dyn InferenceChainContextTrait>,
    _marker: PhantomData<fn() -> F>,
}

#[async_trait]
impl<F> AsyncFunction for GenericFunction<F>
where
    F: Fn(Box<dyn InferenceChainContextTrait>, Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> + Send + Sync,
{
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        (self.func)(self.context.clone(), args)
    }
}
