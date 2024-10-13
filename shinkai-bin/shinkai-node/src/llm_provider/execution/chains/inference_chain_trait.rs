use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use shinkai_db::db::ShinkaiDB;
use shinkai_db::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::schemas::job::Job;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::FunctionCallMetadata;
use shinkai_sqlite::SqliteLogger;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::fmt;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

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
    async fn recurse_chain(
        &mut self,
        backup_result: InferenceChainResult,
    ) -> Result<InferenceChainResult, LLMProviderError> {
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
    fn update_message(&mut self, new_message: ParsedUserMessage);

    fn db(&self) -> Arc<ShinkaiDB>;
    fn vector_fs(&self) -> Arc<VectorFS>;
    fn full_job(&self) -> &Job;
    fn user_message(&self) -> &ParsedUserMessage;
    fn message_hash_id(&self) -> Option<String>;
    fn image_files(&self) -> &HashMap<String, String>;
    fn agent(&self) -> &SerializedLLMProvider;
    fn execution_context(&self) -> &HashMap<String, String>;
    fn generator(&self) -> &RemoteEmbeddingGenerator;
    fn user_profile(&self) -> &ShinkaiName;
    fn max_iterations(&self) -> u64;
    fn iteration_count(&self) -> u64;
    fn max_tokens_in_prompt(&self) -> usize;
    fn raw_files(&self) -> &RawFiles;
    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>;
    fn tool_router(&self) -> Option<Arc<ToolRouter>>;
    fn sheet_manager(&self) -> Option<Arc<Mutex<SheetManager>>>;
    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>>;
    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>>;
    fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>>;
    fn llm_stopper(&self) -> Arc<LLMStopper>;

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

    fn update_message(&mut self, new_message: ParsedUserMessage) {
        self.user_message = new_message;
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

    fn message_hash_id(&self) -> Option<String> {
        self.message_hash_id.clone()
    }

    fn image_files(&self) -> &HashMap<String, String> {
        &self.image_files
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

    fn raw_files(&self) -> &Option<Arc<Vec<(String, Vec<u8>)>>> {
        &self.raw_files
    }

    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> {
        self.ws_manager_trait.clone()
    }

    fn tool_router(&self) -> Option<Arc<ToolRouter>> {
        self.tool_router.clone()
    }

    fn sheet_manager(&self) -> Option<Arc<Mutex<SheetManager>>> {
        self.sheet_manager.clone()
    }

    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>> {
        self.my_agent_payments_manager.clone()
    }

    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>> {
        self.ext_agent_payments_manager.clone()
    }

    fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>> {
        self.sqlite_logger.clone()
    }

    fn llm_stopper(&self) -> Arc<LLMStopper> {
        self.llm_stopper.clone()
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
    pub message_hash_id: Option<String>,
    pub image_files: HashMap<String, String>,
    pub llm_provider: SerializedLLMProvider,
    /// Job's execution context, used to store potentially relevant data across job steps.
    pub execution_context: HashMap<String, String>,
    pub generator: RemoteEmbeddingGenerator,
    pub user_profile: ShinkaiName,
    pub max_iterations: u64,
    pub iteration_count: u64,
    pub max_tokens_in_prompt: usize,
    pub raw_files: RawFiles,
    pub ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    pub tool_router: Option<Arc<ToolRouter>>,
    pub sheet_manager: Option<Arc<Mutex<SheetManager>>>,
    pub my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
    pub ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
    pub sqlite_logger: Option<Arc<SqliteLogger>>,
    pub llm_stopper: Arc<LLMStopper>,
}

impl InferenceChainContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        full_job: Job,
        user_message: ParsedUserMessage,
        message_hash_id: Option<String>,
        image_files: HashMap<String, String>,
        agent: SerializedLLMProvider,
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
        sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Self {
        Self {
            db,
            vector_fs,
            full_job,
            user_message,
            message_hash_id,
            image_files,
            llm_provider: agent,
            execution_context,
            generator,
            user_profile,
            max_iterations,
            iteration_count: 1,
            max_tokens_in_prompt,
            raw_files: None,
            ws_manager_trait,
            tool_router,
            sheet_manager,
            my_agent_payments_manager,
            ext_agent_payments_manager,
            sqlite_logger,
            llm_stopper,
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
            .field("message_hash_id", &self.message_hash_id)
            .field("image_files", &self.image_files.len())
            .field("llm_provider", &self.llm_provider)
            .field("execution_context", &self.execution_context)
            .field("generator", &self.generator)
            .field("user_profile", &self.user_profile)
            .field("max_iterations", &self.max_iterations)
            .field("iteration_count", &self.iteration_count)
            .field("max_tokens_in_prompt", &self.max_tokens_in_prompt)
            .field("raw_files", &self.raw_files)
            .field("ws_manager_trait", &self.ws_manager_trait.is_some())
            .field("tool_router", &self.tool_router.is_some())
            .field("sheet_manager", &self.sheet_manager.is_some())
            .field("my_agent_payments_manager", &self.my_agent_payments_manager.is_some())
            .field("ext_agent_payments_manager", &self.ext_agent_payments_manager.is_some())
            .field("sqlite_logger", &self.sqlite_logger.is_some())
            .finish()
    }
}

/// Struct that represents the result of an inference chain.
#[derive(Debug, Clone)]
pub struct InferenceChainResult {
    pub response: String,
    pub tps: Option<String>,
    pub answer_duration: Option<String>,
    pub new_job_execution_context: HashMap<String, String>,
    pub tool_calls: Option<Vec<FunctionCall>>,
}

impl InferenceChainResult {
    pub fn new(response: String, new_job_execution_context: HashMap<String, String>) -> Self {
        Self {
            response,
            new_job_execution_context,
            tps: None,
            answer_duration: None,
            tool_calls: None,
        }
    }

    pub fn with_full_details(
        response: String,
        tps: Option<String>,
        answer_duration_ms: Option<String>,
        new_job_execution_context: HashMap<String, String>,
        tool_calls: Option<Vec<FunctionCall>>,
    ) -> Self {
        Self {
            response,
            tps,
            answer_duration: answer_duration_ms,
            new_job_execution_context,
            tool_calls,
        }
    }

    pub fn new_empty_execution_context(response: String) -> Self {
        Self::new(response, HashMap::new())
    }

    pub fn new_empty() -> Self {
        Self::new_empty_execution_context(String::new())
    }

    pub fn tool_calls_metadata(&self) -> Option<Vec<FunctionCallMetadata>> {
        self.tool_calls
            .as_ref()
            .map(|calls| calls.iter().map(FunctionCall::to_metadata).collect())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: serde_json::Map<String, serde_json::Value>,
    pub tool_router_key: Option<String>,
}

impl FunctionCall {
    pub fn to_metadata(&self) -> FunctionCallMetadata {
        FunctionCallMetadata {
            name: self.name.clone(),
            arguments: self.arguments.clone(),
            tool_router_key: self.tool_router_key.clone(),
        }
    }
}

/// A struct that holds the response from inference an LLM.
#[derive(Debug, Clone)]
pub struct LLMInferenceResponse {
    pub response_string: String,
    pub function_call: Option<FunctionCall>,
    pub json: JsonValue,
    pub tps: Option<f64>,
}

impl LLMInferenceResponse {
    pub fn new(
        original_response_string: String,
        json: JsonValue,
        function_call: Option<FunctionCall>,
        tps: Option<f64>,
    ) -> Self {
        Self {
            response_string: original_response_string,
            json,
            function_call,
            tps,
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

    fn update_message(&mut self, new_message: ParsedUserMessage) {
        (**self).update_message(new_message)
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

    fn message_hash_id(&self) -> Option<String> {
        (**self).message_hash_id()
    }

    fn image_files(&self) -> &HashMap<String, String> {
        (**self).image_files()
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

    fn raw_files(&self) -> &RawFiles {
        (**self).raw_files()
    }

    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> {
        (**self).ws_manager_trait()
    }

    fn tool_router(&self) -> Option<Arc<ToolRouter>> {
        (**self).tool_router()
    }

    fn sheet_manager(&self) -> Option<Arc<Mutex<SheetManager>>> {
        (**self).sheet_manager()
    }

    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>> {
        (**self).my_agent_payments_manager()
    }

    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>> {
        (**self).ext_agent_payments_manager()
    }

    fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>> {
        (**self).sqlite_logger()
    }

    fn llm_stopper(&self) -> Arc<LLMStopper> {
        (**self).llm_stopper()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        (**self).clone_box()
    }
}

/// A Mock implementation of the InferenceChainContextTrait for testing purposes.
pub struct MockInferenceChainContext {
    pub user_message: ParsedUserMessage,
    pub image_files: HashMap<String, String>,
    pub execution_context: HashMap<String, String>,
    pub user_profile: ShinkaiName,
    pub max_iterations: u64,
    pub iteration_count: u64,
    pub max_tokens_in_prompt: usize,
    pub raw_files: RawFiles,
    pub db: Option<Arc<ShinkaiDB>>,
    pub vector_fs: Option<Arc<VectorFS>>,
    pub my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
    pub ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
    pub llm_stopper: Arc<LLMStopper>,
}

impl MockInferenceChainContext {
    #[allow(clippy::complexity)]
    #[allow(dead_code)]
    pub fn new(
        user_message: ParsedUserMessage,
        execution_context: HashMap<String, String>,
        user_profile: ShinkaiName,
        max_iterations: u64,
        iteration_count: u64,
        max_tokens_in_prompt: usize,
        raw_files: Option<Arc<Vec<(String, Vec<u8>)>>>,
        db: Option<Arc<ShinkaiDB>>,
        vector_fs: Option<Arc<VectorFS>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Self {
        Self {
            user_message,
            image_files: HashMap::new(),
            execution_context,
            user_profile,
            max_iterations,
            iteration_count,
            max_tokens_in_prompt,
            raw_files,
            db,
            vector_fs,
            my_agent_payments_manager,
            ext_agent_payments_manager,
            llm_stopper,
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
            image_files: HashMap::new(),
            execution_context: HashMap::new(),
            user_profile,
            max_iterations: 10,
            iteration_count: 0,
            max_tokens_in_prompt: 1000,
            raw_files: None,
            db: None,
            vector_fs: None,
            my_agent_payments_manager: None,
            ext_agent_payments_manager: None,
            llm_stopper: Arc::new(LLMStopper::new()),
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

    fn update_message(&mut self, new_message: ParsedUserMessage) {
        self.user_message = new_message;
    }

    fn db(&self) -> Arc<ShinkaiDB> {
        self.db.clone().expect("DB is not set")
    }

    fn vector_fs(&self) -> Arc<VectorFS> {
        self.vector_fs.clone().expect("VectorFS is not set")
    }

    fn full_job(&self) -> &Job {
        unimplemented!()
    }

    fn user_message(&self) -> &ParsedUserMessage {
        &self.user_message
    }

    fn message_hash_id(&self) -> Option<String> {
        None
    }

    fn image_files(&self) -> &HashMap<String, String> {
        &self.image_files
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

    fn raw_files(&self) -> &Option<Arc<Vec<(String, Vec<u8>)>>> {
        &self.raw_files
    }

    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> {
        None
    }

    fn tool_router(&self) -> Option<Arc<ToolRouter>> {
        unimplemented!()
    }

    fn sheet_manager(&self) -> Option<Arc<Mutex<SheetManager>>> {
        unimplemented!()
    }

    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>> {
        unimplemented!()
    }

    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>> {
        unimplemented!()
    }

    fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>> {
        None
    }

    fn llm_stopper(&self) -> Arc<LLMStopper> {
        self.llm_stopper.clone()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        Box::new(self.clone())
    }
}

impl Clone for MockInferenceChainContext {
    fn clone(&self) -> Self {
        Self {
            user_message: self.user_message.clone(),
            image_files: self.image_files.clone(),
            execution_context: self.execution_context.clone(),
            user_profile: self.user_profile.clone(),
            max_iterations: self.max_iterations,
            iteration_count: self.iteration_count,
            max_tokens_in_prompt: self.max_tokens_in_prompt,
            raw_files: self.raw_files.clone(),
            db: self.db.clone(),
            vector_fs: self.vector_fs.clone(),
            my_agent_payments_manager: self.my_agent_payments_manager.clone(),
            ext_agent_payments_manager: self.ext_agent_payments_manager.clone(),
            llm_stopper: self.llm_stopper.clone(),
        }
    }
}
