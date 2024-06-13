use crate::agent::execution::user_message_parser::ParsedUserMessage;
use crate::agent::{error::AgentError, job::Job};
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::{collections::HashMap, sync::Arc};

/// Trait that abstracts top level functionality between the inference chains. This allows
/// the inference chain router to work with them all easily.
#[async_trait]
pub trait InferenceChain: Send + Sync {
    /// Returns a hardcoded String that uniquely identifies the chain
    fn chain_id() -> String;
    /// Returns the context for the inference chain
    fn chain_context(&mut self) -> &mut InferenceChainContext;

    /// Starts the inference chain
    async fn run_chain(&mut self) -> Result<InferenceChainResult, AgentError>;

    /// Attempts to recursively call the chain, increasing the iteration count. If the maximum number of iterations is reached,
    /// it will return `backup_result` instead of iterating again. Returns error if something errors inside of the chain.
    async fn recurse_chain(&mut self, backup_result: InferenceChainResult) -> Result<InferenceChainResult, AgentError> {
        let context = self.chain_context();
        if context.iteration_count >= context.max_iterations {
            return Ok(backup_result);
        }
        context.iteration_count += 1;
        self.run_chain().await
    }
}

/// Struct that represents the generalized context available to all chains as input. Note not all chains require
/// using all fields in this struct, but they are available nonetheless.
#[derive(Debug, Clone)]
pub struct InferenceChainContext {
    pub db: Arc<ShinkaiDB>,
    pub vector_fs: Arc<VectorFS>,
    pub full_job: Job,
    pub user_message: ParsedUserMessage,
    pub agent: SerializedAgent,
    /// Job's execution context, used to store potentially relevant data across job steps.
    pub execution_context: HashMap<String, String>,
    pub generator: RemoteEmbeddingGenerator,
    pub user_profile: ShinkaiName,
    pub max_iterations: u64,
    pub iteration_count: u64,
    pub max_tokens_in_prompt: usize,
    pub score_results: HashMap<String, ScoreResult>,
}

impl InferenceChainContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: ParsedUserMessage,
        agent: SerializedAgent,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
        score_results: HashMap<String, ScoreResult>,
    ) -> Self {
        Self {
            db,
            vector_fs,
            full_job,
            user_message,
            agent,
            execution_context,
            generator,
            user_profile,
            max_iterations,
            iteration_count: 1,
            max_tokens_in_prompt,
            score_results,
        }
    }

    /// Updates the maximum number of iterations allowed for this chain
    pub fn update_max_iterations(&mut self, new_max_iterations: u64) {
        self.max_iterations = new_max_iterations;
    }
}

/// Struct that represents the result of an inference chain.
pub struct InferenceChainResult {
    pub response: String,
    pub new_job_execution_context: HashMap<String, String>,
}

impl InferenceChainResult {
    pub fn new(response: String, new_job_execution_context: HashMap<String, String>) -> Self {
        Self {
            response,
            new_job_execution_context,
        }
    }

    pub fn new_empty_execution_context(response: String) -> Self {
        Self::new(response, HashMap::new())
    }

    pub fn new_empty() -> Self {
        Self::new_empty_execution_context(String::new())
    }
}

// The result from scoring an inference chain (checking if its the right chain to route to)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreResult {
    pub score: f32,
    pub passed_scoring: bool,
}
impl ScoreResult {
    pub fn new(score: f32, passed_scoring: bool) -> Self {
        Self { score, passed_scoring }
    }

    pub fn new_empty() -> Self {
        Self::new(0.0, false)
    }
}
/// A struct that holds the response from inference an LLM.
#[derive(Debug, Clone)]
pub struct LLMInferenceResponse {
    pub original_response_string: String,
    pub json: JsonValue,
}

impl LLMInferenceResponse {
    pub fn new(original_response_string: String, json: JsonValue) -> Self {
        Self {
            original_response_string,
            json,
        }
    }
}
