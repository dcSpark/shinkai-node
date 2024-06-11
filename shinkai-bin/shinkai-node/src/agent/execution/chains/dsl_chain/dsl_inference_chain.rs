use std::{any::Any, collections::HashMap, fmt, marker::PhantomData};

use crate::agent::job::JobLike;
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
        let executor = engine.iter(&self.workflow);
        let mut final_registers = DashMap::new();

        for result in executor {
            match result {
                Ok(registers) => {
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
        F: Fn(Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> + Send + Sync + 'a,
    {
        self.functions.insert(
            name.to_string(),
            Box::new(GenericFunction {
                func,
                _marker: PhantomData,
            }),
        );
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
        let db = self.context.db.clone();
        let vector_fs = self.context.vector_fs.clone();
        let full_job = self.context.full_job.clone();
        let agent = self.context.agent.clone();
        let generator = self.context.generator.clone();
        let user_profile = self.context.user_profile.clone();
        let max_tokens_in_prompt = self.context.max_tokens_in_prompt;

        let query_text = user_message.clone();

        let scope_is_empty = full_job.scope().is_empty();
        let mut ret_nodes: Vec<RetrievedNode> = vec![];
        let mut summary_node_text = None;
        if !scope_is_empty {
            let (ret, summary) = JobManager::keyword_chained_job_scope_vector_search(
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
            summary_node_text = summary;
        }

        let filled_prompt = JobPromptGenerator::qa_response_prompt_with_vector_search(
            user_message.clone(),
            ret_nodes,
            None,
            Some(query_text),
            Some(full_job.step_history.clone()),
            max_tokens_in_prompt,
        );

        // Handle response_res without using the `?` operator
        let response = JobManager::inference_agent_markdown(agent.clone(), filled_prompt.clone())
            .await
            .map_err(|e| {
                eprintln!("Error calling inference agent markdown: {}", e);
                WorkflowError::ExecutionError(e.to_string())
            })?;

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

struct GenericFunction<F>
where
    F: Fn(Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> + Send + Sync,
{
    func: F,
    _marker: PhantomData<fn() -> F>,
}

#[async_trait]
impl<F> AsyncFunction for GenericFunction<F>
where
    F: Fn(Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> + Send + Sync,
{
    async fn call(&self, args: Vec<Box<dyn Any + Send>>) -> Result<Box<dyn Any + Send>, WorkflowError> {
        (self.func)(args)
    }
}
