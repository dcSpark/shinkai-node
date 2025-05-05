use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainContextTrait, InferenceChainResult
};
use crate::llm_provider::execution::prompts::general_prompts::JobPromptGenerator;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::tool_router::{ToolCallFunctionResponse, ToolRouter};
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;

use crate::utils::environment::{fetch_node_environment, NodeEnvironment};
use async_trait::async_trait;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_fs::shinkai_fs_error::ShinkaiFsError;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job::{Job, JobLike};
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::shinkai_fs::ShinkaiFileChunkCollection;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::ws_types::{
    ToolMetadata, ToolStatus, ToolStatusType, WSMessageType, WSUpdateHandler, WidgetMetadata
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;

use std::fmt;
use std::path::PathBuf;
use std::result::Result::Ok;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

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

    fn chain_context(&mut self) -> &mut dyn InferenceChainContextTrait {
        &mut self.context
    }

    async fn run_chain(&mut self) -> Result<InferenceChainResult, LLMProviderError> {
        let response = GenericInferenceChain::start_chain(
            self.context.db.clone(),
            self.context.full_job.clone(),
            self.context.user_message.original_user_message_string.to_string(),
            self.context.user_tool_selected.clone(),
            self.context.force_tools_scope.clone(),
            self.context.fs_files_paths.clone(),
            self.context.job_filenames.clone(),
            self.context.message_hash_id.clone(),
            self.context.image_files.clone(),
            self.context.llm_provider.clone(),
            self.context.generator.clone(),
            self.context.user_profile.clone(),
            self.context.max_iterations,
            self.context.max_tokens_in_prompt,
            self.ws_manager_trait.clone(),
            self.context.tool_router.clone(),
            self.context.my_agent_payments_manager.clone(),
            self.context.ext_agent_payments_manager.clone(),
            self.context.job_callback_manager.clone(),
            // self.context.sqlite_logger.clone(),
            self.context.llm_stopper.clone(),
            fetch_node_environment(),
        )
        .await?;
        Ok(response)
    }
}

impl GenericInferenceChain {
    /// Process image files from file paths, folder paths, and job scope
    fn process_image_files(
        paths: &[ShinkaiPath],
        folder_paths: &[ShinkaiPath],
        scope: &MinimalJobScope,
    ) -> HashMap<String, String> {
        let mut image_files = HashMap::new();

        // Process individual files
        for file_path in paths {
            if let Some(file_name) = file_path.path.file_name() {
                let filename_lower = file_name.to_string_lossy().to_lowercase();
                if filename_lower.ends_with(".png")
                    || filename_lower.ends_with(".jpg")
                    || filename_lower.ends_with(".jpeg")
                    || filename_lower.ends_with(".gif")
                {
                    // Retrieve the file content
                    match ShinkaiFileManager::get_file_content(file_path.clone()) {
                        Ok(content) => {
                            let base64_content = base64::encode(&content);
                            image_files.insert(file_path.relative_path().to_string(), base64_content);
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        // Process scope files
        for file_path in &scope.vector_fs_items {
            if let Some(file_name) = file_path.path.file_name() {
                let filename_lower = file_name.to_string_lossy().to_lowercase();
                if filename_lower.ends_with(".png")
                    || filename_lower.ends_with(".jpg")
                    || filename_lower.ends_with(".jpeg")
                    || filename_lower.ends_with(".gif")
                {
                    match ShinkaiFileManager::get_file_content(file_path.clone()) {
                        Ok(content) => {
                            let base64_content = base64::encode(&content);
                            image_files.insert(file_path.relative_path().to_string(), base64_content);
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        // Process all folders (including scope folders)
        let mut all_folders = folder_paths.to_vec();
        all_folders.extend(scope.vector_fs_folders.clone());

        if let Ok(additional_files) =
            ShinkaiFileManager::get_absolute_path_for_additional_files(Vec::new(), all_folders)
        {
            for file_path in additional_files {
                let path = PathBuf::from(file_path);
                if path.is_file() {
                    if let Some(file_name) = path.file_name() {
                        let filename_lower = file_name.to_string_lossy().to_lowercase();
                        if filename_lower.ends_with(".png")
                            || filename_lower.ends_with(".jpg")
                            || filename_lower.ends_with(".jpeg")
                            || filename_lower.ends_with(".gif")
                        {
                            // Convert path to ShinkaiPath for consistent handling
                            let shinkai_path = ShinkaiPath::from_string(path.to_string_lossy().to_string());
                            match ShinkaiFileManager::get_file_content(shinkai_path.clone()) {
                                Ok(content) => {
                                    let base64_content = base64::encode(&content);
                                    image_files.insert(shinkai_path.relative_path().to_string(), base64_content);
                                }
                                Err(_) => continue,
                            }
                        }
                    }
                }
            }
        }

        image_files
    }

    pub fn new(
        context: InferenceChainContext,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Self {
        Self {
            context,
            ws_manager_trait,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_chain(
        db: Arc<SqliteManager>,
        full_job: Job,
        user_message: String,
        user_tool_selected: Option<String>,
        force_tools_scope: Option<Vec<String>>,
        fs_files_paths: Vec<ShinkaiPath>,
        job_filenames: Vec<String>,
        message_hash_id: Option<String>,
        mut image_files: HashMap<String, String>,
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
        _node_env: NodeEnvironment,
    ) -> Result<InferenceChainResult, LLMProviderError> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("start_generic_inference_chain>  message: {:?}", user_message),
        );
        let start_time = Instant::now();

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
        let mut ret_nodes: ShinkaiFileChunkCollection = ShinkaiFileChunkCollection {
            chunks: vec![],
            paths: None,
        };

        // Merge agent scope fs_files_paths if llm_provider is an agent
        let mut merged_fs_files_paths = fs_files_paths.clone();
        let mut merged_fs_folder_paths = Vec::new();
        if let ProviderOrAgent::Agent(agent) = &llm_provider {
            merged_fs_files_paths.extend(agent.scope.vector_fs_items.clone());
            merged_fs_folder_paths.extend(agent.scope.vector_fs_folders.clone());
        }

        // We always automatically add the job folder to the scope
        if let Ok(file_infos) = ShinkaiFileManager::get_all_files_and_folders_for_job(&full_job.job_id, &db) {
            for file_info in file_infos {
                let path = ShinkaiPath::from_string(file_info.path);
                if file_info.is_directory {
                    merged_fs_folder_paths.push(path);
                } else {
                    merged_fs_files_paths.push(path);
                }
            }
        }

        // Process image files from merged paths, folders and scope
        let additional_image_files =
            Self::process_image_files(&merged_fs_files_paths, &merged_fs_folder_paths, full_job.scope());

        // Deduplicate image files based on filename (case insensitive)
        let mut deduplicated_files = HashMap::new();
        for (path, content) in image_files.iter().chain(additional_image_files.iter()) {
            let filename = path.split('/').last().unwrap_or(path).to_lowercase();
            if !deduplicated_files.contains_key(&filename) {
                deduplicated_files.insert(filename, (path.clone(), content.clone()));
            }
        }

        // Convert back to original format with full paths
        image_files = deduplicated_files
            .into_iter()
            .map(|(_, (path, content))| (path, content))
            .collect();

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("start_generic_inference_chain> image files: {:?}", image_files.keys()),
        );

        if !scope_is_empty
            || !merged_fs_files_paths.is_empty()
            || !merged_fs_folder_paths.is_empty()
            || !job_filenames.is_empty()
        {
            let ret = JobManager::search_for_chunks_in_resources(
                merged_fs_files_paths.clone(),
                merged_fs_folder_paths.clone(),
                job_filenames.clone(),
                full_job.job_id.clone(),
                full_job.scope(),
                db.clone(),
                user_message.clone(),
                20,
                max_tokens_in_prompt,
                generator.clone(),
            )
            .await?;
            ret_nodes = ret;
        }

        // 2) Vector search for tooling / workflows if the workflow / tooling scope isn't empty
        let job_config = full_job.config();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("job_config: {:?}", job_config),
        );
        let mut tools = vec![];

        // Decision Process for Tool Selection:
        // 1. Check if a specific tool was requested by the user
        // 2. If not, fall back to automatic tool selection based on capabilities and context
        // Combine the check for Some and non-empty string using filter
        if let Some(selected_tool_name) = user_tool_selected.filter(|name| !name.is_empty()) {
            // CASE 1: User explicitly selected a tool
            // This takes precedence over all other tool selection methods
            if let Some(tool_router) = &tool_router {
                match tool_router.get_tool_by_name(&selected_tool_name).await {
                    Ok(Some(tool)) => tools.push(tool),
                    Ok(None) => {
                        return Err(LLMProviderError::ToolNotFound(format!(
                            "Selected tool not found: {}",
                            selected_tool_name
                        )));
                    }
                    Err(e) => {
                        return Err(LLMProviderError::ToolRetrievalError(format!(
                            "Error retrieving selected tool: {:?}",
                            e
                        )));
                    }
                }
            }
        } else if let Some(forced_tools) = force_tools_scope.clone() {
            // CASE 2: force_tools_scope is provided - This takes precedence over automatic tool selection
            // force_tools_scope allows explicit specification of which tools should be available,
            // provided as a Vec<String> of tool names. For each tool name:
            // 1. First tries exact name match
            // 2. If exact match fails, performs both:
            //    - Full-text search (FTS) for exact keyword matches
            //    - Vector search for semantic similarity (confidence threshold 0.2)
            // 3. Combines results prioritizing:
            //    - FTS exact matches first
            //    - Then high-confidence vector search matches
            // 4. Returns error if no matches found for a forced tool
            if let Some(tool_router) = &tool_router {
                for tool_name in forced_tools {
                    match tool_router.get_tool_by_name(&tool_name).await {
                        Ok(Some(tool)) => tools.push(tool),
                        Ok(None) => {
                            // If tool not found directly, try FTS and vector search
                            let sanitized_query = tool_name.replace(|c: char| !c.is_alphanumeric() && c != ' ', " ");

                            // Perform FTS search
                            let fts_results = tool_router.sqlite_manager.search_tools_fts(&sanitized_query);

                            // Perform vector search
                            let vector_results = tool_router
                                .sqlite_manager
                                .tool_vector_search(&sanitized_query, 5, false, true)
                                .await;

                            match (fts_results, vector_results) {
                                (Ok(fts_tools), Ok(vector_tools)) => {
                                    let mut combined_tools = Vec::new();
                                    let mut seen_ids = std::collections::HashSet::new();

                                    // Add FTS results first (exact matches)
                                    for fts_tool in fts_tools {
                                        if seen_ids.insert(fts_tool.tool_router_key.clone()) {
                                            combined_tools.push(fts_tool);
                                        }
                                    }

                                    // Add vector search results with high confidence (score < 0.2)
                                    for (tool, score) in vector_tools {
                                        if score < 0.2 && seen_ids.insert(tool.tool_router_key.clone()) {
                                            combined_tools.push(tool);
                                        }
                                    }

                                    if combined_tools.is_empty() {
                                        return Err(LLMProviderError::ToolNotFound(format!(
                                            "Forced tool not found: {} (no matches found in search)",
                                            tool_name
                                        )));
                                    }

                                    // Add the best matching tool
                                    if let Some(best_tool) = combined_tools.first() {
                                        match tool_router.get_tool_by_name(&best_tool.name).await {
                                            Ok(Some(tool)) => tools.push(tool),
                                            _ => {
                                                return Err(LLMProviderError::ToolNotFound(format!(
                                                    "Best matching tool could not be retrieved: {}",
                                                    best_tool.name
                                                )));
                                            }
                                        }
                                    }
                                }
                                (Err(e), _) | (_, Err(e)) => {
                                    return Err(LLMProviderError::ToolRetrievalError(format!(
                                        "Error searching for tool alternatives: {:?}",
                                        e
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            return Err(LLMProviderError::ToolRetrievalError(format!(
                                "Error retrieving forced tool: {:?}",
                                e
                            )));
                        }
                    }
                }
            }
        } else {
            // CASE 3: No specific tool selected and no force_tools_scope - use automatic
            // tool selection Check various conditions to determine if and which
            // tools should be available

            // 2a. Check if streaming is enabled in job config
            let stream = job_config.as_ref().and_then(|config| config.stream);

            // 2b. Check if tools are allowed by job config (defaults to true if not
            // specified)
            let tools_allowed = job_config.as_ref().and_then(|config| config.use_tools).unwrap_or(false);

            // 2c. Check if the LLM provider is an agent with tools
            let is_agent_with_tools = match &llm_provider {
                ProviderOrAgent::Agent(agent) => !agent.tools.is_empty(),
                ProviderOrAgent::LLMProvider(_) => false,
            };

            // 2d. Check if the LLM provider/agent has tool capabilities
            let can_use_tools = ModelCapabilitiesManager::has_tool_capabilities_for_provider_or_agent(
                llm_provider.clone(),
                db.clone(),
                stream,
            )
            .await;

            // Only proceed with tool selection if either:
            // - Tools are allowed by configuration AND the LLM provider has tool capabilities
            // - OR it's an agent with available tools
            if (can_use_tools || is_agent_with_tools) && tools_allowed {
                // CASE 2.1: If using an Agent, get its specifically configured tools
                if let ProviderOrAgent::Agent(agent) = &llm_provider {
                    for tool in &agent.tools {
                        if let Some(tool_router) = &tool_router {
                            match tool_router
                                .get_tool_by_name_and_version(&tool.to_string_without_version(), tool.version())
                                .await
                            {
                                Ok(Some(tool)) => tools.push(tool),
                                Ok(None) => {
                                    return Err(LLMProviderError::ToolNotFound(format!(
                                        "Tool not found for name: {}",
                                        tool.to_string_with_version()
                                    )));
                                }
                                Err(e) => {
                                    return Err(LLMProviderError::ToolRetrievalError(format!(
                                        "Error retrieving tool: {:?}",
                                        e
                                    )));
                                }
                            }
                        }
                    }
                } else {
                    // CASE 2.2: For regular LLM providers, perform vector search
                    // to find the most relevant tools for the user's message
                    if let Some(tool_router) = &tool_router {
                        let results = tool_router
                            .combined_tool_search(&user_message.clone(), 7, false, true)
                            .await;

                        match results {
                            Ok(results) => {
                                for result in results {
                                    match tool_router.get_tool_by_name(&result.tool_router_key).await {
                                        Ok(Some(tool)) => tools.push(tool),
                                        Ok(None) => {
                                            return Err(LLMProviderError::ToolNotFound(format!(
                                                "Tool not found for key: {}",
                                                result.tool_router_key
                                            )));
                                        }
                                        Err(e) => {
                                            return Err(LLMProviderError::ToolRetrievalError(format!(
                                                "Error retrieving tool: {:?}",
                                                e
                                            )));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                return Err(LLMProviderError::ToolSearchError(format!(
                                    "Error during tool search: {:?}",
                                    e
                                )));
                            }
                        }
                    }
                }
            }
        }

        // After this point, 'tools' vector contains either:
        // 1. A single specifically requested tool
        // 2. Tools from an Agent's configuration
        // 3. Tools found through vector search
        // 4. Empty vector if no tools were selected/allowed

        // 3) Generate Prompt
        // First, attempt to use the custom_prompt from the job's config.
        // If it doesn't exist, fall back to the agent's custom_prompt if the
        // llm_provider is an Agent.
        let custom_prompt = job_config
            .and_then(|config| {
                // Only return Some if custom_prompt exists and is not empty
                config
                    .custom_prompt
                    .clone()
                    .and_then(|prompt| if prompt.is_empty() { None } else { Some(prompt) })
            })
            .or_else(|| {
                if let ProviderOrAgent::Agent(agent) = &llm_provider {
                    agent.config.as_ref().and_then(|config| {
                        // Also check for empty string in agent config
                        config.custom_prompt.clone().and_then(
                            |prompt| {
                                if prompt.is_empty() {
                                    None
                                } else {
                                    Some(prompt)
                                }
                            },
                        )
                    })
                } else {
                    None
                }
            });

        let custom_system_prompt = job_config
            .and_then(|config| {
                // Only return Some if custom_system_prompt exists and is not empty
                config.custom_system_prompt.clone().and_then(
                    |prompt| {
                        if prompt.is_empty() {
                            None
                        } else {
                            Some(prompt)
                        }
                    },
                )
            })
            .or_else(|| {
                if let ProviderOrAgent::Agent(agent) = &llm_provider {
                    agent.config.as_ref().and_then(|config| {
                        // Also check for empty string in agent config
                        config.custom_system_prompt.clone().and_then(|prompt| {
                            if prompt.is_empty() {
                                None
                            } else {
                                Some(prompt)
                            }
                        })
                    })
                } else {
                    None
                }
            });

        let additional_files = Self::get_additional_files(
            &db,
            &full_job,
            job_filenames.clone(),
            merged_fs_files_paths.clone(),
            merged_fs_folder_paths.clone(),
        )?;

        println!(
            "Generating prompt with user message: {:?} containing {:?} image files and {:?} additional files",
            user_message,
            image_files.keys(),
            additional_files
        );

        // We'll keep a record of *every* function call + response across all iterations:
        let mut all_function_responses = Vec::new();

        // NEW: Accumulate all LLM response messages
        let mut all_llm_messages = Vec::new();

        let mut filled_prompt = JobPromptGenerator::generic_inference_prompt(
            db.clone(),
            custom_system_prompt.clone(),
            custom_prompt.clone(),
            user_message.clone(),
            image_files.clone(),
            ret_nodes.clone(),
            None,
            Some(full_job.step_history.clone()),
            tools.clone(),
            None,
            full_job.job_id.clone(),
            additional_files.clone(),
        )
        .await;

        let mut iteration_count = 0;
        let mut tool_calls_history = Vec::new();
        loop {
            // Check if max_iterations is reached
            if iteration_count >= max_iterations {
                let answer_duration_ms = Some(format!("{:.2}", start_time.elapsed().as_millis()));
                let max_iterations_message = format!(
                    "Maximum iterations ({}) reached. Process stopped after {} tool calls.",
                    max_iterations,
                    tool_calls_history.len()
                );

                // NEW: Join all accumulated messages for the result
                let full_conversation = all_llm_messages
                    .iter()
                    .map(|msg: &String| msg.trim())
                    .filter(|msg| !msg.is_empty())
                    .collect::<Vec<&str>>()
                    .join("\n\n");

                let inference_result = InferenceChainResult::with_full_details(
                    format!("{}\n\n{}", full_conversation, max_iterations_message),
                    None,
                    answer_duration_ms,
                    Some(tool_calls_history.clone()),
                );

                return Ok(inference_result);
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
                db.clone(),
            )
            .await;

            // Error Codes
            if let Err(LLMProviderError::LLMServiceInferenceLimitReached(e)) = &response_res {
                return Err(LLMProviderError::LLMServiceInferenceLimitReached(e.to_string()));
            } else if let Err(LLMProviderError::LLMServiceUnexpectedError(e)) = &response_res {
                return Err(LLMProviderError::LLMServiceUnexpectedError(e.to_string()));
            }

            let response = response_res?;

            // NEW: Accumulate this LLM message
            all_llm_messages.push(response.response_string.clone());

            // 5) Check response if it requires a function call
            if !response.is_function_calls_empty() {
                let mut iteration_function_responses = Vec::new();
                let mut should_retry = false;

                for function_call in response.function_calls {
                    let parsed_message = ParsedUserMessage::new(user_message.clone());
                    let image_files = HashMap::new();
                    let context = InferenceChainContext::new(
                        db.clone(),
                        full_job.clone(),
                        parsed_message,
                        None,
                        force_tools_scope.clone(),
                        fs_files_paths.clone(),
                        job_filenames.clone(),
                        message_hash_id.clone(),
                        image_files.clone(),
                        llm_provider.clone(),
                        generator.clone(),
                        user_profile.clone(),
                        max_iterations,
                        max_tokens_in_prompt,
                        ws_manager_trait.clone(),
                        tool_router.clone(),
                        my_agent_payments_manager.clone(),
                        ext_agent_payments_manager.clone(),
                        job_callback_manager.clone(),
                        // sqlite_logger.clone(),
                        llm_stopper.clone(),
                    );

                    // 6) Call workflow or tooling
                    // Find the ShinkaiTool that has a tool with the function name
                    let shinkai_tool = tools.iter().find(|tool| {
                        tool.internal_sanitized_name() == function_call.name
                            || tool.tool_router_key().to_string_without_version()
                                == function_call.tool_router_key.clone().unwrap_or_default()
                    });

                    if shinkai_tool.is_none() {
                        eprintln!("Function not found: {}", function_call.name);
                        return Err(LLMProviderError::FunctionNotFound(function_call.name.clone()));
                    }
                    let shinkai_tool = shinkai_tool.unwrap();

                    // Note: here we can add logic to handle the case that we have network tools
                    // TODO: if shinkai_tool is None we need to retry with the LLM (hallucination)
                    let function_response = match tool_router
                        .as_ref()
                        .unwrap()
                        .call_function(function_call.clone(), &context, &shinkai_tool, user_profile.clone())
                        .await
                    {
                        Ok(response) => response,
                        Err(e) => {
                            match &e {
                                LLMProviderError::ToolRouterError(ref error_msg)
                                    if error_msg.contains("Invalid function arguments") =>
                                {
                                    // For invalid arguments, we'll retry with the LLM by including the error
                                    // message in the next prompt to help it fix
                                    // the parameters
                                    let mut function_call_with_error = function_call.clone();
                                    function_call_with_error.response = Some(error_msg.clone());
                                    tool_calls_history.push(function_call_with_error);

                                    // Store the error response to be included in the next prompt
                                    iteration_function_responses.push(ToolCallFunctionResponse {
                                        function_call: function_call.clone(),
                                        response: error_msg.clone(),
                                    });

                                    // Update prompt with error information for retry
                                    filled_prompt = JobPromptGenerator::generic_inference_prompt(
                                        db.clone(),
                                        custom_system_prompt.clone(),
                                        custom_prompt.clone(),
                                        user_message.clone(),
                                        image_files.clone(),
                                        ret_nodes.clone(),
                                        None,
                                        Some(full_job.step_history.clone()),
                                        tools.clone(),
                                        // Pass all function responses (including the error) to keep context
                                        Some(
                                            all_function_responses
                                                .iter()
                                                .chain(iteration_function_responses.iter())
                                                .cloned()
                                                .collect(),
                                        ),
                                        full_job.job_id.clone(),
                                        additional_files.clone(),
                                    )
                                    .await;

                                    // Set flag to retry and break out of the function calls loop
                                    iteration_count += 1;
                                    should_retry = true;
                                    break;
                                }
                                LLMProviderError::ToolRouterError(ref error_msg)
                                    if error_msg.contains("MissingConfigError") =>
                                {
                                    // For missing config, we'll pass through the error directly
                                    // This will show up in the UI prompting the user to update their config
                                    eprintln!("Missing config error: {:?}", error_msg);
                                    return Err(e);
                                }
                                _ => {
                                    eprintln!("Error calling function: {:?}", e);
                                    return Err(e);
                                }
                            }
                        }
                    };

                    let mut function_call_with_router_key = function_call.clone();
                    function_call_with_router_key.tool_router_key =
                        Some(shinkai_tool.tool_router_key().to_string_without_version());
                    function_call_with_router_key.response = Some(function_response.response.clone());
                    tool_calls_history.push(function_call_with_router_key);

                    // Trigger WS update after receiving function_response
                    Self::trigger_ws_update(
                        &ws_manager_trait,
                        &Some(full_job.job_id.clone()),
                        &function_response,
                        shinkai_tool.tool_router_key().to_string_without_version(),
                    )
                    .await;

                    // Store all function responses to use in the next prompt
                    iteration_function_responses.push(function_response);
                }

                let additional_files = Self::get_additional_files(
                    &db,
                    &full_job,
                    job_filenames.clone(),
                    merged_fs_files_paths.clone(),
                    merged_fs_folder_paths.clone(),
                )?;

                // If we need to retry, continue the outer loop
                if should_retry {
                    continue;
                }

                // Add this iteration's responses to our cumulative collection
                all_function_responses.extend(iteration_function_responses);

                // Call LLM again with ALL responses from all iterations
                filled_prompt = JobPromptGenerator::generic_inference_prompt(
                    db.clone(),
                    custom_system_prompt.clone(),
                    custom_prompt.clone(),
                    user_message.clone(),
                    image_files.clone(),
                    ret_nodes.clone(),
                    None,
                    Some(full_job.step_history.clone()),
                    tools.clone(),
                    Some(all_function_responses.clone()),
                    full_job.job_id.clone(),
                    additional_files,
                )
                .await;
            } else {
                // No more function calls required, return the final response
                let answer_duration_ms = Some(format!("{:.2}", start_time.elapsed().as_millis()));

                // NEW: Join all accumulated messages for the result
                let full_conversation = all_llm_messages
                    .iter()
                    .map(|msg: &String| msg.trim())
                    .filter(|msg| !msg.is_empty())
                    .collect::<Vec<&str>>()
                    .join("\n\n");

                let inference_result = InferenceChainResult::with_full_details(
                    full_conversation,
                    response.tps.map(|tps| tps.to_string()),
                    answer_duration_ms,
                    Some(tool_calls_history.clone()),
                );

                return Ok(inference_result);
            }

            // Increment the iteration count
            iteration_count += 1;
        }
    }

    /// Triggers a WebSocket update after receiving a function response.
    async fn trigger_ws_update(
        ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        job_id: &Option<String>,
        function_response: &ToolCallFunctionResponse,
        tool_router_key: String,
    ) {
        if let Some(ref manager) = ws_manager_trait {
            if let Some(job_id) = job_id {
                // Derive inbox name from job_id
                let inbox_name_result = InboxName::get_job_inbox_name_from_params(job_id.clone());
                let inbox_name_string = match inbox_name_result {
                    Ok(inbox_name) => inbox_name.to_string(),
                    Err(e) => {
                        // Log the error and exit the function
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to create inbox name from job_id {}: {}", job_id, e),
                        );
                        return;
                    }
                };

                let m = manager.lock().await;

                // Prepare ToolMetadata with result and Completed status
                let tool_metadata = ToolMetadata {
                    tool_name: function_response.function_call.name.clone(),
                    tool_router_key: Some(tool_router_key),
                    args: serde_json::to_value(&function_response.function_call)
                        .unwrap_or_else(|_| serde_json::json!({}))
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                    result: serde_json::from_str(&function_response.response)
                        .map(Some)
                        .unwrap_or_else(|_| Some(serde_json::Value::String(function_response.response.clone()))),
                    status: ToolStatus {
                        type_: ToolStatusType::Complete,
                        reason: None,
                    },
                    index: function_response.function_call.index,
                };

                let ws_message_type = WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

                let _ = m
                    .queue_message(
                        WSTopic::Inbox,
                        inbox_name_string,
                        serde_json::to_string(&function_response).unwrap_or_else(|_| "{}".to_string()),
                        ws_message_type,
                        true,
                    )
                    .await;
            }
        }
    }

    pub fn get_additional_files(
        db: &SqliteManager,
        full_job: &Job,
        job_filenames: Vec<String>,
        merged_fs_files_paths: Vec<ShinkaiPath>,
        merged_fs_folder_paths: Vec<ShinkaiPath>,
    ) -> Result<Vec<String>, ShinkaiFsError> {
        let mut additional_files: Vec<String> = vec![];
        // Get agent/context files
        let f = ShinkaiFileManager::get_absolute_path_for_additional_files(
            merged_fs_files_paths.clone(),
            merged_fs_folder_paths.clone(),
        )?;
        additional_files.extend(f);

        // Get Job files
        let folder_path: Result<ShinkaiPath, shinkai_sqlite::errors::SqliteManagerError> =
            db.get_job_folder_name(&full_job.job_id.clone());

        if let Ok(folder_path) = folder_path {
            additional_files.extend(ShinkaiFileManager::get_absolute_paths_with_folder(
                job_filenames.clone(),
                folder_path.path.clone(),
            ));
        }

        // Deduplicate files based on filename (case insensitive)
        let mut seen_filenames = std::collections::HashSet::new();
        additional_files.retain(|path| {
            let filename = path.split('/').last().unwrap_or(path).to_lowercase();
            seen_filenames.insert(filename)
        });

        Ok(additional_files)
    }
}
