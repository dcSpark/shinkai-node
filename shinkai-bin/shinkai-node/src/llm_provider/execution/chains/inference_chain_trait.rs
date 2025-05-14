use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_message_primitives::schemas::job::Job;
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::FunctionCallMetadata;
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;

use std::fmt;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

/// Trait that abstracts top level functionality between the inference chains.
/// This allows the inference chain router to work with them all easily.
#[async_trait]
pub trait InferenceChain: Send + Sync {
    /// Returns a hardcoded String that uniquely identifies the chain
    fn chain_id() -> String;
    /// Returns the context for the inference chain
    fn chain_context(&mut self) -> &mut dyn InferenceChainContextTrait;

    /// Starts the inference chain
    async fn run_chain(&mut self) -> Result<InferenceChainResult, LLMProviderError>;

    /// Attempts to recursively call the chain, increasing the iteration count.
    /// If the maximum number of iterations is reached, it will return
    /// `backup_result` instead of iterating again. Returns error if something
    /// errors inside of the chain.
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

    fn db(&self) -> Arc<SqliteManager>;
    fn full_job(&self) -> &Job;
    fn user_message(&self) -> &ParsedUserMessage;
    fn user_tool_selected(&self) -> Option<String>;
    fn force_tools_scope(&self) -> Option<Vec<String>>;
    fn message_hash_id(&self) -> Option<String>;
    fn image_files(&self) -> &HashMap<String, String>;
    fn agent(&self) -> &ProviderOrAgent;
    fn generator(&self) -> &RemoteEmbeddingGenerator;
    fn user_profile(&self) -> &ShinkaiName;
    fn max_iterations(&self) -> u64;
    fn iteration_count(&self) -> u64;
    fn max_tokens_in_prompt(&self) -> usize;
    fn raw_files(&self) -> &RawFiles;
    fn ws_manager_trait(&self) -> Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>;
    fn tool_router(&self) -> Option<Arc<ToolRouter>>;
    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>>;
    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>>;
    fn job_callback_manager(&self) -> Option<Arc<Mutex<JobCallbackManager>>>;
    // fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>>;
    fn llm_stopper(&self) -> Arc<LLMStopper>;
    fn fs_files_paths(&self) -> &Vec<ShinkaiPath>;
    fn llm_provider(&self) -> &ProviderOrAgent;
    fn job_filenames(&self) -> &Vec<String>;
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

    fn db(&self) -> Arc<SqliteManager> {
        Arc::clone(&self.db)
    }

    fn full_job(&self) -> &Job {
        &self.full_job
    }

    fn user_message(&self) -> &ParsedUserMessage {
        &self.user_message
    }

    fn user_tool_selected(&self) -> Option<String> {
        self.user_tool_selected.clone()
    }

    fn force_tools_scope(&self) -> Option<Vec<String>> {
        self.force_tools_scope.clone()
    }

    fn message_hash_id(&self) -> Option<String> {
        self.message_hash_id.clone()
    }

    fn image_files(&self) -> &HashMap<String, String> {
        &self.image_files
    }

    fn agent(&self) -> &ProviderOrAgent {
        &self.llm_provider
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

    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>> {
        self.my_agent_payments_manager.clone()
    }

    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>> {
        self.ext_agent_payments_manager.clone()
    }

    fn job_callback_manager(&self) -> Option<Arc<Mutex<JobCallbackManager>>> {
        self.job_callback_manager.clone()
    }

    // fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>> {
    //     self.sqlite_logger.clone()
    // }

    fn llm_stopper(&self) -> Arc<LLMStopper> {
        self.llm_stopper.clone()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        Box::new(self.clone())
    }

    fn fs_files_paths(&self) -> &Vec<ShinkaiPath> {
        &self.fs_files_paths
    }

    fn llm_provider(&self) -> &ProviderOrAgent {
        &self.llm_provider
    }

    fn job_filenames(&self) -> &Vec<String> {
        &self.job_filenames
    }
}

/// Struct that represents the generalized context available to all chains as
/// input. Note not all chains require using all fields in this struct, but they
/// are available nonetheless.
#[derive(Clone)]
pub struct InferenceChainContext {
    pub db: Arc<SqliteManager>,
    pub full_job: Job,
    pub user_message: ParsedUserMessage,
    pub user_tool_selected: Option<String>,
    pub force_tools_scope: Option<Vec<String>>,
    pub fs_files_paths: Vec<ShinkaiPath>,
    pub job_filenames: Vec<String>,
    pub message_hash_id: Option<String>,
    pub image_files: HashMap<String, String>,
    pub llm_provider: ProviderOrAgent,
    /// Job's execution context, used to store potentially relevant data across
    /// job steps.
    pub generator: RemoteEmbeddingGenerator,
    pub user_profile: ShinkaiName,
    pub max_iterations: u64,
    pub iteration_count: u64,
    pub max_tokens_in_prompt: usize,
    pub raw_files: RawFiles,
    pub ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    pub tool_router: Option<Arc<ToolRouter>>,
    pub my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
    pub ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
    pub job_callback_manager: Option<Arc<Mutex<JobCallbackManager>>>,
    // pub sqlite_logger: Option<Arc<SqliteLogger>>,
    pub llm_stopper: Arc<LLMStopper>,
}

impl InferenceChainContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<SqliteManager>,
        full_job: Job,
        user_message: ParsedUserMessage,
        user_tool_selected: Option<String>,
        force_tools_scope: Option<Vec<String>>,
        fs_files_paths: Vec<ShinkaiPath>,
        job_filenames: Vec<String>,
        message_hash_id: Option<String>,
        image_files: HashMap<String, String>,
        llm_provider: ProviderOrAgent,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        max_iterations: u64,
        max_tokens_in_prompt: usize,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        job_callback_manager: Option<Arc<Mutex<JobCallbackManager>>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Self {
        Self {
            db,
            full_job,
            user_message,
            user_tool_selected,
            force_tools_scope,
            fs_files_paths,
            job_filenames,
            message_hash_id,
            image_files,
            llm_provider,
            generator,
            user_profile,
            max_iterations,
            iteration_count: 1,
            max_tokens_in_prompt,
            raw_files: None,
            ws_manager_trait,
            tool_router,
            my_agent_payments_manager,
            ext_agent_payments_manager,
            job_callback_manager,
            // sqlite_logger,
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
            .field("full_job", &self.full_job)
            .field("user_message", &self.user_message)
            .field("user_tool_selected", &self.user_tool_selected)
            .field("force_tools_scope", &self.force_tools_scope)
            .field("fs_files_paths", &self.fs_files_paths)
            .field("job_filenames", &self.job_filenames)
            .field("message_hash_id", &self.message_hash_id)
            .field("image_files", &self.image_files.len())
            .field("llm_provider", &self.llm_provider)
            .field("generator", &self.generator)
            .field("user_profile", &self.user_profile)
            .field("max_iterations", &self.max_iterations)
            .field("iteration_count", &self.iteration_count)
            .field("max_tokens_in_prompt", &self.max_tokens_in_prompt)
            .field("raw_files", &self.raw_files)
            .field("ws_manager_trait", &self.ws_manager_trait.is_some())
            .field("tool_router", &self.tool_router.is_some())
            .field("my_agent_payments_manager", &self.my_agent_payments_manager.is_some())
            .field("ext_agent_payments_manager", &self.ext_agent_payments_manager.is_some())
            .field("job_callback_manager", &self.job_callback_manager.is_some())
            // .field("sqlite_logger", &self.sqlite_logger.is_some())
            .finish()
    }
}

/// Struct that represents the result of an inference chain.
#[derive(Debug, Clone)]
pub struct InferenceChainResult {
    pub response: String,
    pub tps: Option<String>,
    pub answer_duration: Option<String>,
    pub tool_calls: Option<Vec<FunctionCall>>,
}

impl InferenceChainResult {
    pub fn new(response: String) -> Self {
        Self {
            response,
            tps: None,
            answer_duration: None,
            tool_calls: None,
        }
    }

    pub fn with_full_details(
        response: String,
        tps: Option<String>,
        answer_duration_ms: Option<String>,
        tool_calls: Option<Vec<FunctionCall>>,
    ) -> Self {
        Self {
            response,
            tps,
            answer_duration: answer_duration_ms,
            tool_calls,
        }
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
    pub response: Option<String>,
    pub index: u64,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
}

impl FunctionCall {
    pub fn to_metadata(&self) -> FunctionCallMetadata {
        FunctionCallMetadata {
            name: self.name.clone(),
            arguments: self.arguments.clone(),
            tool_router_key: self.tool_router_key.clone(),
            response: self.response.clone(),
        }
    }
}

/// A struct that holds the response from inference an LLM.
#[derive(Debug, Clone)]
pub struct LLMInferenceResponse {
    pub response_string: String,
    pub function_calls: Vec<FunctionCall>,
    pub json: JsonValue,
    pub tps: Option<f64>,
}

impl LLMInferenceResponse {
    pub fn new(response_string: String, json: JsonValue, function_calls: Vec<FunctionCall>, tps: Option<f64>) -> Self {
        Self {
            response_string,
            json,
            function_calls,
            tps,
        }
    }

    pub fn is_function_calls_empty(&self) -> bool {
        self.function_calls.is_empty()
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

    fn db(&self) -> Arc<SqliteManager> {
        (**self).db()
    }

    fn full_job(&self) -> &Job {
        (**self).full_job()
    }

    fn user_message(&self) -> &ParsedUserMessage {
        (**self).user_message()
    }

    fn user_tool_selected(&self) -> Option<String> {
        (**self).user_tool_selected()
    }

    fn force_tools_scope(&self) -> Option<Vec<String>> {
        (**self).force_tools_scope()
    }

    fn message_hash_id(&self) -> Option<String> {
        (**self).message_hash_id()
    }

    fn image_files(&self) -> &HashMap<String, String> {
        (**self).image_files()
    }

    fn agent(&self) -> &ProviderOrAgent {
        (**self).agent()
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

    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>> {
        (**self).my_agent_payments_manager()
    }

    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>> {
        (**self).ext_agent_payments_manager()
    }

    fn job_callback_manager(&self) -> Option<Arc<Mutex<JobCallbackManager>>> {
        (**self).job_callback_manager()
    }

    // fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>> {
    //     (**self).sqlite_logger()
    // }

    fn llm_stopper(&self) -> Arc<LLMStopper> {
        (**self).llm_stopper()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        (**self).clone_box()
    }

    fn fs_files_paths(&self) -> &Vec<ShinkaiPath> {
        (**self).fs_files_paths()
    }

    fn llm_provider(&self) -> &ProviderOrAgent {
        (**self).llm_provider()
    }

    fn job_filenames(&self) -> &Vec<String> {
        (**self).job_filenames()
    }
}

/// A Mock implementation of the InferenceChainContextTrait for testing
/// purposes.
pub struct MockInferenceChainContext {
    pub user_message: ParsedUserMessage,
    pub image_files: HashMap<String, String>,
    pub user_profile: ShinkaiName,
    pub max_iterations: u64,
    pub iteration_count: u64,
    pub max_tokens_in_prompt: usize,
    pub raw_files: RawFiles,
    pub db: Option<Arc<SqliteManager>>,
    pub my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
    pub ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
    pub llm_stopper: Arc<LLMStopper>,
    pub fs_files_paths: Vec<ShinkaiPath>,
    pub job_filenames: Vec<String>,
    pub llm_provider: ProviderOrAgent,
}

impl MockInferenceChainContext {
    #[allow(clippy::complexity)]
    #[allow(dead_code)]
    pub fn new(
        user_message: ParsedUserMessage,
        user_profile: ShinkaiName,
        max_iterations: u64,
        iteration_count: u64,
        max_tokens_in_prompt: usize,
        raw_files: Option<Arc<Vec<(String, Vec<u8>)>>>,
        db: Option<Arc<SqliteManager>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        llm_stopper: Arc<LLMStopper>,
        fs_files_paths: Vec<ShinkaiPath>,
        job_filenames: Vec<String>,
        llm_provider: ProviderOrAgent,
    ) -> Self {
        Self {
            user_message,
            image_files: HashMap::new(),
            user_profile,
            max_iterations,
            iteration_count,
            max_tokens_in_prompt,
            raw_files,
            db,
            my_agent_payments_manager,
            ext_agent_payments_manager,
            llm_stopper,
            fs_files_paths,
            job_filenames,
            llm_provider,
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
            user_profile,
            max_iterations: 10,
            iteration_count: 0,
            max_tokens_in_prompt: 1000,
            raw_files: None,
            db: None,
            my_agent_payments_manager: None,
            ext_agent_payments_manager: None,
            llm_stopper: Arc::new(LLMStopper::new()),
            fs_files_paths: vec![],
            job_filenames: vec![],
            llm_provider: ProviderOrAgent::LLMProvider(SerializedLLMProvider::mock_provider()),
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

    fn db(&self) -> Arc<SqliteManager> {
        self.db.clone().expect("DB is not set")
    }

    fn full_job(&self) -> &Job {
        unimplemented!()
    }

    fn user_message(&self) -> &ParsedUserMessage {
        &self.user_message
    }

    fn user_tool_selected(&self) -> Option<String> {
        None
    }

    fn force_tools_scope(&self) -> Option<Vec<String>> {
        None
    }

    fn message_hash_id(&self) -> Option<String> {
        None
    }

    fn image_files(&self) -> &HashMap<String, String> {
        &self.image_files
    }

    fn agent(&self) -> &ProviderOrAgent {
        unimplemented!()
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

    fn my_agent_payments_manager(&self) -> Option<Arc<Mutex<MyAgentOfferingsManager>>> {
        unimplemented!()
    }

    fn ext_agent_payments_manager(&self) -> Option<Arc<Mutex<ExtAgentOfferingsManager>>> {
        unimplemented!()
    }

    fn job_callback_manager(&self) -> Option<Arc<Mutex<JobCallbackManager>>> {
        unimplemented!()
    }

    // fn sqlite_logger(&self) -> Option<Arc<SqliteLogger>> {
    //     None
    // }

    fn llm_stopper(&self) -> Arc<LLMStopper> {
        self.llm_stopper.clone()
    }

    fn clone_box(&self) -> Box<dyn InferenceChainContextTrait> {
        Box::new(self.clone())
    }

    fn fs_files_paths(&self) -> &Vec<ShinkaiPath> {
        &self.fs_files_paths
    }

    fn llm_provider(&self) -> &ProviderOrAgent {
        &self.llm_provider
    }

    fn job_filenames(&self) -> &Vec<String> {
        &self.job_filenames
    }
}

impl Clone for MockInferenceChainContext {
    fn clone(&self) -> Self {
        Self {
            user_message: self.user_message.clone(),
            image_files: self.image_files.clone(),
            user_profile: self.user_profile.clone(),
            max_iterations: self.max_iterations,
            iteration_count: self.iteration_count,
            max_tokens_in_prompt: self.max_tokens_in_prompt,
            raw_files: self.raw_files.clone(),
            db: self.db.clone(),
            my_agent_payments_manager: self.my_agent_payments_manager.clone(),
            ext_agent_payments_manager: self.ext_agent_payments_manager.clone(),
            llm_stopper: self.llm_stopper.clone(),
            fs_files_paths: self.fs_files_paths.clone(),
            job_filenames: self.job_filenames.clone(),
            llm_provider: self.llm_provider.clone(),
        }
    }
}
