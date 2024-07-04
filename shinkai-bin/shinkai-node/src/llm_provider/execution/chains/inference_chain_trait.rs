use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::providers::shared::openai::FunctionCall;
use crate::llm_provider::{error::LLMProviderError, job::Job};
use crate::db::ShinkaiDB;
use crate::network::ws_manager::WSUpdateHandler;
use crate::tools::tool_router::ToolRouter;
use crate::vector_fs::vector_fs::VectorFS;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use tokio::sync::Mutex;
use std::fmt;
use std::{collections::HashMap, sync::Arc};

/// Trait that abstracts top level functionality between the inference chains. This allows
/// the inference chain router to work with them all easily.
#[async_trait]
pub trait InferenceChain: Send + Sync {
    /// Returns a hardcoded String that uniquely identifies the chain
    fn chain_id() -> String;
    /// Returns the context for the inference chain
    fn chain_context(&mut self) -> &mut dyn InferenceChainContextTrait;

    /// Starts the inference chain
    async fn run_chain(&mut self) -> Result<InferenceChainResult, LLMProviderError>;

    /// Attempts to recursively call the chain, increasing the iteration count. If the maximum number of iterations is reached,
    /// it will return `backup_result` instead of iterating again. Returns error if something errors inside of the chain.
    async fn recurse_chain(&mut self, backup_result: InferenceChainResult) -> Result<InferenceChainResult, LLMProviderError> {
        let context = self.chain_context();
        if context.iteration_count() >= context.max_iterations() {
            return Ok(backup_result);
        }
        context.update_iteration_count(context.iteration_count() + 1);
        self.run_chain().await
    }
}

pub type RawFiles = Option<Arc<Vec<(String, Vec<u8>)>>>;

/// Trait for InferenceChainContext to facilitate mocking for tests.
pub trait InferenceChainContextTrait: Send + Sync {
    fn update_max_iterations(&mut self, new_max_iterations: u64);
    fn update_raw_files(&mut self, new_raw_files: RawFiles);
    fn update_iteration_count(&mut self, new_iteration_count: u64);

    fn db(&self) -> Arc<ShinkaiDB>;
    fn vector_fs(&self) -> Arc<VectorFS>;
    fn full_job(&self) -> &Job;
    fn user_message(&self) -> &ParsedUserMessage;
    fn agent(&self) -> &SerializedLLMProvider;
    fn execution_context(&self) -> &HashMap<String, String>;
    fn generator(&self) -> &RemoteEmbeddingGenerator;
    fn user_profile(&self) -> &ShinkaiName;
    fn max_iterations(&self) -> u64;
    fn iteration_count(&self) -> u64;
    fn max_tokens_in_prompt(&self) -> usize;
    fn score_results(&self) -> &HashMap<String, ScoreResult>;
    fn raw_files(&self) -> &RawFiles;
    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>;
    fn tool_router(&self) -> Option<Arc<Mutex<ToolRouter>>>;

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait>;
}

impl Clone for Box<dyn InferenceChainContextTrait> {
    fn clone(&self) -> Box<dyn InferenceChainContextTrait> {
        self.clone_box()
    }
}

impl InferenceChainContextTrait for InferenceChainContext {
    fn update_max_iterations(&mut self, new_max_iterations: u64) {
        self.max_iterations = new_max_iterations;
    }

    fn update_raw_files(&mut self, new_raw_files: Option<Arc<Vec<(String, Vec<u8>)>>>) {
        self.raw_files = new_raw_files;
    }

    fn update_iteration_count(&mut self, new_iteration_count: u64) {
        self.iteration_count = new_iteration_count;
    }

    fn db(&self) -> Arc<ShinkaiDB> {
        Arc::clone(&self.db)
    }

    fn vector_fs(&self) -> Arc<VectorFS> {
        Arc::clone(&self.vector_fs)
    }

    fn full_job(&self) -> &Job {
        &self.full_job
    }

    fn user_message(&self) -> &ParsedUserMessage {
        &self.user_message
    }

    fn agent(&self) -> &SerializedLLMProvider {
        &self.llm_provider
    }

    fn execution_context(&self) -> &HashMap<String, String> {
        &self.execution_context
    }

    fn generator(&self) -> &RemoteEmbeddingGenerator {
        &self.generator
    }

    fn user_profile(&self) -> &ShinkaiName {
        &self.user_profile
    }

    fn max_iterations(&self) -> u64 {
        self.max_iterations
    }

    fn iteration_count(&self) -> u64 {
        self.iteration_count
    }

    fn max_tokens_in_prompt(&self) -> usize {
        self.max_tokens_in_prompt
    }

    fn score_results(&self) -> &HashMap<String, ScoreResult> {
        &self.score_results
    }

    fn raw_files(&self) -> &Option<Arc<Vec<(String, Vec<u8>)>>> {
        &self.raw_files
    }

    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> {
        self.ws_manager_trait.clone()
    }

    fn tool_router(&self) -> Option<Arc<Mutex<ToolRouter>>> {
        self.tool_router.clone()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        Box::new(self.clone())
    }
}

/// Struct that represents the generalized context available to all chains as input. Note not all chains require
/// using all fields in this struct, but they are available nonetheless.
#[derive(Clone)]
pub struct InferenceChainContext {
    pub db: Arc<ShinkaiDB>,
    pub vector_fs: Arc<VectorFS>,
    pub full_job: Job,
    pub user_message: ParsedUserMessage,
    pub llm_provider: SerializedLLMProvider,
    /// Job's execution context, used to store potentially relevant data across job steps.
    pub execution_context: HashMap<String, String>,
    pub generator: RemoteEmbeddingGenerator,
    pub user_profile: ShinkaiName,
    pub max_iterations: u64,
    pub iteration_count: u64,
    pub max_tokens_in_prompt: usize,
    pub score_results: HashMap<String, ScoreResult>,
    pub raw_files: RawFiles,
    pub ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    pub tool_router: Option<Arc<Mutex<ToolRouter>>>,
}

impl InferenceChainContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: ParsedUserMessage,
        agent: SerializedLLMProvider,
        execution_context: HashMap<String, String>,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
        score_results: HashMap<String, ScoreResult>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
    ) -> Self {
        Self {
            db,
            vector_fs,
            full_job,
            user_message,
            llm_provider: agent,
            execution_context,
            generator,
            user_profile,
            max_iterations,
            iteration_count: 1,
            max_tokens_in_prompt,
            score_results,
            raw_files: None,
            ws_manager_trait,
            tool_router,
        }
    }

    /// Updates the maximum number of iterations allowed for this chain
    pub fn update_max_iterations(&mut self, new_max_iterations: u64) {
        self.max_iterations = new_max_iterations;
    }

    /// Updates the raw files for this context
    pub fn update_raw_files(&mut self, new_raw_files: RawFiles) {
        self.raw_files = new_raw_files;
    }
}

impl fmt::Debug for InferenceChainContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InferenceChainContext")
            .field("db", &self.db)
            .field("vector_fs", &self.vector_fs)
            .field("full_job", &self.full_job)
            .field("user_message", &self.user_message)
            .field("llm_provider", &self.llm_provider)
            .field("execution_context", &self.execution_context)
            .field("generator", &self.generator)
            .field("user_profile", &self.user_profile)
            .field("max_iterations", &self.max_iterations)
            .field("iteration_count", &self.iteration_count)
            .field("max_tokens_in_prompt", &self.max_tokens_in_prompt)
            .field("score_results", &self.score_results)
            .field("raw_files", &self.raw_files)
            .field("ws_manager_trait", &self.ws_manager_trait.is_some())
            .field("tool_router", &self.tool_router.is_some())
            .finish()
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
    pub response_string: String,
    pub function_call: Option<FunctionCall>,
    pub json: JsonValue,
}

impl LLMInferenceResponse {
    pub fn new(original_response_string: String, json: JsonValue, function_call: Option<FunctionCall>) -> Self {
        Self {
            response_string: original_response_string,
            json,
            function_call
        }
    }
}

impl InferenceChainContextTrait for Box<dyn InferenceChainContextTrait> {
    fn update_max_iterations(&mut self, new_max_iterations: u64) {
        (**self).update_max_iterations(new_max_iterations)
    }

    fn update_raw_files(&mut self, new_raw_files: RawFiles) {
        (**self).update_raw_files(new_raw_files)
    }

    fn update_iteration_count(&mut self, new_iteration_count: u64) {
        (**self).update_iteration_count(new_iteration_count)
    }

    fn db(&self) -> Arc<ShinkaiDB> {
        (**self).db()
    }

    fn vector_fs(&self) -> Arc<VectorFS> {
        (**self).vector_fs()
    }

    fn full_job(&self) -> &Job {
        (**self).full_job()
    }

    fn user_message(&self) -> &ParsedUserMessage {
        (**self).user_message()
    }

    fn agent(&self) -> &SerializedLLMProvider {
        (**self).agent()
    }

    fn execution_context(&self) -> &HashMap<String, String> {
        (**self).execution_context()
    }

    fn generator(&self) -> &RemoteEmbeddingGenerator {
        (**self).generator()
    }

    fn user_profile(&self) -> &ShinkaiName {
        (**self).user_profile()
    }

    fn max_iterations(&self) -> u64 {
        (**self).max_iterations()
    }

    fn iteration_count(&self) -> u64 {
        (**self).iteration_count()
    }

    fn max_tokens_in_prompt(&self) -> usize {
        (**self).max_tokens_in_prompt()
    }

    fn score_results(&self) -> &HashMap<String, ScoreResult> {
        (**self).score_results()
    }

    fn raw_files(&self) -> &RawFiles {
        (**self).raw_files()
    }

    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> {
        (**self).ws_manager_trait()
    }

    fn tool_router(&self) -> Option<Arc<Mutex<ToolRouter>>> {
        (**self).tool_router()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        (**self).clone_box()
    }
}

/// A Mock implementation of the InferenceChainContextTrait for testing purposes.
pub struct MockInferenceChainContext {
    pub user_message: ParsedUserMessage,
    pub execution_context: HashMap<String, String>,
    pub user_profile: ShinkaiName,
    pub max_iterations: u64,
    pub iteration_count: u64,
    pub max_tokens_in_prompt: usize,
    pub score_results: HashMap<String, ScoreResult>,
    pub raw_files: RawFiles,
}

impl MockInferenceChainContext {
    #[allow(clippy::complexity)]
    pub fn new(
        user_message: ParsedUserMessage,
        execution_context: HashMap<String, String>,
        user_profile: ShinkaiName,
        max_iterations: u64,
        iteration_count: u64,
        max_tokens_in_prompt: usize,
        score_results: HashMap<String, ScoreResult>,
        raw_files: Option<Arc<Vec<(String, Vec<u8>)>>>,
    ) -> Self {
        Self {
            user_message,
            execution_context,
            user_profile,
            max_iterations,
            iteration_count,
            max_tokens_in_prompt,
            score_results,
            raw_files,
        }
    }
}

impl Default for MockInferenceChainContext {
    fn default() -> Self {
        let user_message = ParsedUserMessage {
            original_user_message_string: "".to_string(),
            elements: vec![],
        };
        let user_profile = ShinkaiName::default_testnet_localhost();
        Self {
            user_message,
            execution_context: HashMap::new(),
            user_profile,
            max_iterations: 10,
            iteration_count: 0,
            max_tokens_in_prompt: 1000,
            score_results: HashMap::new(),
            raw_files: None,
        }
    }
}

impl InferenceChainContextTrait for MockInferenceChainContext {
    fn update_max_iterations(&mut self, new_max_iterations: u64) {
        self.max_iterations = new_max_iterations;
    }

    fn update_raw_files(&mut self, new_raw_files: Option<Arc<Vec<(String, Vec<u8>)>>>) {
        self.raw_files = new_raw_files;
    }

    fn update_iteration_count(&mut self, new_iteration_count: u64) {
        self.iteration_count = new_iteration_count;
    }

    fn db(&self) -> Arc<ShinkaiDB> {
        unimplemented!()
    }

    fn vector_fs(&self) -> Arc<VectorFS> {
        unimplemented!()
    }

    fn full_job(&self) -> &Job {
        unimplemented!()
    }

    fn user_message(&self) -> &ParsedUserMessage {
        &self.user_message
    }

    fn agent(&self) -> &SerializedLLMProvider {
        unimplemented!()
    }

    fn execution_context(&self) -> &HashMap<String, String> {
        &self.execution_context
    }

    fn generator(&self) -> &RemoteEmbeddingGenerator {
        unimplemented!()
    }

    fn user_profile(&self) -> &ShinkaiName {
        &self.user_profile
    }

    fn max_iterations(&self) -> u64 {
        self.max_iterations
    }

    fn iteration_count(&self) -> u64 {
        self.iteration_count
    }

    fn max_tokens_in_prompt(&self) -> usize {
        self.max_tokens_in_prompt
    }

    fn score_results(&self) -> &HashMap<String, ScoreResult> {
        &self.score_results
    }

    fn raw_files(&self) -> &Option<Arc<Vec<(String, Vec<u8>)>>> {
        &self.raw_files
    }

    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> {
        unimplemented!()
    }

    fn tool_router(&self) -> Option<Arc<Mutex<ToolRouter>>> {
        unimplemented!()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        Box::new(self.clone())
    }
}

impl Clone for MockInferenceChainContext {
    fn clone(&self) -> Self {
        Self {
            user_message: self.user_message.clone(),
            execution_context: self.execution_context.clone(),
            user_profile: self.user_profile.clone(),
            max_iterations: self.max_iterations,
            iteration_count: self.iteration_count,
            max_tokens_in_prompt: self.max_tokens_in_prompt,
            score_results: self.score_results.clone(),
            raw_files: self.raw_files.clone(),
        }
    }
}
