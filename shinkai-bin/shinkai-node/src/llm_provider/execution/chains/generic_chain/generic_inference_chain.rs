use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::{
    InferenceChain, InferenceChainContext, InferenceChainContextTrait, InferenceChainResult,
};
use crate::llm_provider::execution::prompts::general_prompts::JobPromptGenerator;
use crate::llm_provider::execution::user_message_parser::ParsedUserMessage;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::{ToolCallFunctionResponse, ToolRouter};
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;

use crate::utils::environment::{fetch_node_environment, NodeEnvironment};
use async_trait::async_trait;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job::{Job, JobLike};
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::shinkai_fs::ShinkaiFileChunkCollection;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::ws_types::{
    ToolMetadata, ToolStatus, ToolStatusType, WSMessageType, WSUpdateHandler, WidgetMetadata,
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;

use std::fmt;
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
            self.context.sheet_manager.clone(),
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
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        job_callback_manager: Option<Arc<Mutex<JobCallbackManager>>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
        node_env: NodeEnvironment,
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

        if !scope_is_empty || !fs_files_paths.is_empty() || !job_filenames.is_empty() {    
            let ret = JobManager::search_for_chunks_in_resources(
                fs_files_paths.clone(),
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
        if let Some(selected_tool_name) = user_tool_selected {
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
        } else {
            // CASE 2: No specific tool selected - use automatic tool selection
            // Check various conditions to determine if and which tools should be available

            // 2a. Check if streaming is enabled in job config
            let stream = job_config.as_ref().and_then(|config| config.stream);

            // 2b. Check if tools are allowed by job config (defaults to true if not specified)
            let tools_allowed = job_config.as_ref().and_then(|config| config.use_tools).unwrap_or(false);

            // 2c. Check if the LLM provider is an agent
            let is_agent = match &llm_provider {
                ProviderOrAgent::Agent(_) => true,
                ProviderOrAgent::LLMProvider(_) => false,
            };

            // 2d. Check if the LLM provider/agent has tool capabilities
            let can_use_tools = ModelCapabilitiesManager::has_tool_capabilities_for_provider_or_agent(
                llm_provider.clone(),
                db.clone(),
                stream,
            )
            .await;

            // Only proceed with tool selection if both conditions are met:
            // - Tools are allowed by configuration
            // - The LLM provider has tool capabilities
            if can_use_tools && tools_allowed || is_agent {
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
                            .combined_tool_search(&user_message.clone(), 4, false, true)
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
        // If it doesn't exist, fall back to the agent's custom_prompt if the llm_provider is an Agent.
        let custom_prompt = job_config.and_then(|config| config.custom_prompt.clone()).or_else(|| {
            if let ProviderOrAgent::Agent(agent) = &llm_provider {
                agent.config.as_ref().and_then(|config| config.custom_prompt.clone())
            } else {
                None
            }
        });

        let custom_system_prompt = job_config
            .and_then(|config| config.custom_system_prompt.clone())
            .or_else(|| {
                if let ProviderOrAgent::Agent(agent) = &llm_provider {
                    agent
                        .config
                        .as_ref()
                        .and_then(|config| config.custom_system_prompt.clone())
                } else {
                    None
                }
            });

        let mut filled_prompt = JobPromptGenerator::generic_inference_prompt(
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
            node_env.clone(),
        );

        let mut iteration_count = 0;
        let mut tool_calls_history = Vec::new();
        loop {
            // Check if max_iterations is reached
            if iteration_count >= max_iterations {
                return Err(LLMProviderError::MaxIterationsReached(
                    "Maximum iterations reached".to_string(),
                ));
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

            // 5) Check response if it requires a function call
            if let Some(function_call) = response.function_call {
                let parsed_message = ParsedUserMessage::new(user_message.clone());
                let image_files = HashMap::new();
                let context = InferenceChainContext::new(
                    db.clone(),
                    full_job.clone(),
                    parsed_message,
                    None,
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
                    sheet_manager.clone(),
                    my_agent_payments_manager.clone(),
                    ext_agent_payments_manager.clone(),
                    job_callback_manager.clone(),
                    // sqlite_logger.clone(),
                    llm_stopper.clone(),
                );

                // 6) Call workflow or tooling
                // Find the ShinkaiTool that has a tool with the function name
                let shinkai_tool = tools.iter().find(|tool| {
                    tool.name() == function_call.name
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
                        eprintln!("Error calling function: {:?}", e);
                        // Handle different error types here if needed
                        return Err(e);
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

                // 7) Call LLM again with the response (for formatting)
                filled_prompt = JobPromptGenerator::generic_inference_prompt(
                    custom_system_prompt.clone(),
                    custom_prompt.clone(),
                    user_message.clone(),
                    image_files.clone(),
                    ret_nodes.clone(),
                    None,
                    Some(full_job.step_history.clone()),
                    tools.clone(),
                    Some(function_response),
                    full_job.job_id.clone(),
                    node_env.clone(),
                );
            } else {
                // No more function calls required, return the final response
                let answer_duration_ms = Some(format!("{:.2}", start_time.elapsed().as_millis()));

                let inference_result = InferenceChainResult::with_full_details(
                    response.response_string,
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
}
