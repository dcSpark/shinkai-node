use std::env;
use std::sync::Arc;
use std::time::Instant;

use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, InferenceChainContextTrait};
use crate::network::v2_api::api_v2_commands_app_files::get_app_folder_path;
use crate::network::Node;
use crate::tools::tool_definitions::definition_generation::{generate_tool_definitions, get_rust_tools};
use crate::tools::tool_execution::execution_header_generator::{check_tool_config, generate_execution_environment};
use crate::utils::environment::fetch_node_environment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_message_primitives::schemas::indexable_version::IndexableVersion;
use shinkai_message_primitives::schemas::invoices::{Invoice, InvoiceStatusEnum};
use shinkai_message_primitives::schemas::job::JobLike;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tool_offering::{
    AssetPayment, ToolPrice, UsageType, UsageTypeInquiry,
};
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::wallet_mixed::{Asset, NetworkIdentifier};
use shinkai_message_primitives::schemas::ws_types::{PaymentMetadata, WSMessageType, WidgetMetadata};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::files::prompts_data;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::network_tool::NetworkTool;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::rust_tools::RustTool;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use shinkai_tools_runner::built_in_tools;

#[derive(Clone)]
pub struct ToolRouter {
    pub sqlite_manager: Arc<SqliteManager>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallFunctionResponse {
    pub response: String,
    pub function_call: FunctionCall,
}

impl ToolRouter {
    pub fn new(sqlite_manager: Arc<SqliteManager>) -> Self {
        ToolRouter { sqlite_manager }
    }

    pub async fn initialization(&self, generator: Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        let is_empty;
        let has_any_js_tools;
        {
            is_empty = self
                .sqlite_manager
                .is_empty()
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

            has_any_js_tools = self
                .sqlite_manager
                .has_any_js_tools()
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        if let Err(e) = Self::import_tools_from_directory(self.sqlite_manager.clone()).await {
            eprintln!("Error importing tools from directory: {}", e);
        }

        if is_empty {
            if let Err(e) = self.add_testing_network_tools().await {
                eprintln!("Error adding testing network tools: {}", e);
            }
            if let Err(e) = self.add_rust_tools().await {
                eprintln!("Error adding rust tools: {}", e);
            }
            if let Err(e) = self.add_static_prompts(&generator).await {
                eprintln!("Error adding static prompts: {}", e);
            }
        } else if !has_any_js_tools {
            if let Err(e) = self.add_testing_network_tools().await {
                eprintln!("Error adding testing network tools: {}", e);
            }
            if let Err(e) = self.add_rust_tools().await {
                eprintln!("Error adding rust tools: {}", e);
            }
        }

        Ok(())
    }

    pub async fn force_reinstall_all(&self, generator: &Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        if let Err(e) = self.add_testing_network_tools().await {
            eprintln!("Error adding testing network tools: {}", e);
        }
        if let Err(e) = self.add_rust_tools().await {
            eprintln!("Error adding rust tools: {}", e);
        }
        if let Err(e) = self.add_static_prompts(generator).await {
            eprintln!("Error adding static prompts: {}", e);
        }
        if let Err(e) = Self::import_tools_from_directory(self.sqlite_manager.clone()).await {
            eprintln!("Error importing tools from directory: {}", e);
        }
        Ok(())
    }

    async fn import_tools_from_directory(db: Arc<SqliteManager>) -> Result<(), ToolError> {
        if env::var("SKIP_IMPORT_FROM_DIRECTORY")
            .unwrap_or("false".to_string())
            .to_lowercase()
            .eq("true")
        {
            return Ok(());
        }

        // Start timing before the HTTP request
        let start_time = Instant::now();

        let url = env::var("SHINKAI_TOOLS_DIRECTORY_URL")
            .map_err(|_| ToolError::MissingConfigError("SHINKAI_TOOLS_DIRECTORY_URL not set".to_string()))?;

        let response = reqwest::get(url).await.map_err(|e| ToolError::RequestError(e))?;

        if response.status() != 200 {
            return Err(ToolError::ExecutionError(format!(
                "Import tools request returned a non OK status: {}",
                response.status()
            )));
        }

        let tools: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| ToolError::ParseError(format!("Failed to parse tools directory: {}", e)))?;

        let tool_urls = tools
            .iter()
            .map(|tool| {
                (
                    tool["name"].as_str(),
                    tool["file"].as_str(),
                    tool["router_key"].as_str(),
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
            .filter(|(name, url, router_key)| url.is_some() && name.is_some() && router_key.is_some())
            .map(|(name, url, router_key)| (name.unwrap(), url.unwrap(), router_key.unwrap()))
            .collect::<Vec<_>>();

        let futures = tool_urls.into_iter().map(|(tool_name, tool_url, router_key)| {
            let db = db.clone();
            let node_env = fetch_node_environment();
            let tool_url = tool_url.to_string();
            async move {
                let tool = db.get_tool_by_key(router_key);
                let _ = match tool {
                    Ok(_) => {
                        println!("Tool already exists: {}", router_key);
                        return Ok::<(), ToolError>(());
                    }
                    Err(SqliteManagerError::ToolNotFound(_)) => {
                        ();
                    }
                    Err(e) => {
                        eprintln!("Failed to get tool: {:#?}", e);
                        return Ok::<(), ToolError>(());
                    }
                };

                match Node::v2_api_import_tool_internal(db, node_env, tool_url).await {
                    Ok(_) => {
                        println!("Successfully imported tool {}", tool_name);
                        return Ok::<(), ToolError>(());
                    }
                    Err(e) => {
                        eprintln!("Failed to import tool {}: {:#?}", tool_name, e);
                        return Ok::<(), ToolError>(()); // Continue on error
                    }
                }
            }
        });

        futures::future::join_all(futures).await;

        // Calculate and print the duration
        let duration = start_time.elapsed();
        println!("Total time taken to import tools: {:?}", duration);

        Ok(())
    }

    pub async fn add_static_prompts(&self, _generator: &Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        // Check if ONLY_TESTING_PROMPTS is set
        if env::var("ONLY_TESTING_PROMPTS").unwrap_or_default() == "1"
            || env::var("ONLY_TESTING_PROMPTS").unwrap_or_default().to_lowercase() == "true"
        {
            return Ok(()); // Return right away and don't add anything
        }

        let start_time = Instant::now();

        // Determine which set of prompts to use
        let prompts_data = if env::var("IS_TESTING").unwrap_or_default() == "1" {
            prompts_data::PROMPTS_JSON_TESTING
        } else {
            prompts_data::PROMPTS_JSON
        };

        // Parse the JSON string into a Vec<Value>
        let json_array: Vec<Value> = serde_json::from_str(prompts_data).expect("Failed to parse prompts JSON data");

        println!("Number of static prompts to add: {}", json_array.len());

        // Use the add_prompts_from_json_values method
        {
            self.sqlite_manager
                .add_prompts_from_json_values(json_array)
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        let duration = start_time.elapsed();
        if env::var("LOG_ALL").unwrap_or_default() == "1" {
            println!("Time taken to add static prompts: {:?}", duration);
        }
        Ok(())
    }

    pub async fn add_network_tool(&self, network_tool: NetworkTool) -> Result<(), ToolError> {
        self.sqlite_manager
            .add_tool(ShinkaiTool::Network(network_tool, true))
            .await
            .map(|_| ())
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }

    async fn add_rust_tools(&self) -> Result<(), ToolError> {
        let rust_tools = get_rust_tools();
        for tool in rust_tools {
            let rust_tool = RustTool::new(
                tool.name,
                tool.description,
                tool.input_args,
                tool.output_arg,
                None,
                tool.tool_router_key,
            );
            self.sqlite_manager
                .add_tool(ShinkaiTool::Rust(rust_tool, true))
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }
        Ok(())
    }

    async fn add_testing_network_tools(&self) -> Result<(), ToolError> {
        // Check if ADD_TESTING_EXTERNAL_NETWORK_ECHO is set
        if std::env::var("ADD_TESTING_EXTERNAL_NETWORK_ECHO").unwrap_or_else(|_| "false".to_string()) == "true" {
            let usage_type = UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                asset: Asset {
                    network_id: NetworkIdentifier::BaseSepolia,
                    asset_id: "USDC".to_string(),
                    decimals: Some(6),
                    contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
                },
                amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
            }]));

            // Manually create NetworkTool
            let network_tool = NetworkTool {
                name: "network__echo".to_string(),
                toolkit_name: "shinkai-tool-echo".to_string(),
                description: "Echoes the input message".to_string(),
                version: "0.1".to_string(),
                provider: ShinkaiName::new("@@agent_provider.arb-sep-shinkai".to_string()).unwrap(),
                usage_type: usage_type.clone(),
                activated: true,
                config: vec![],
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property(
                        "message".to_string(),
                        "string".to_string(),
                        "The message to echo".to_string(),
                        true,
                    );
                    params
                },
                output_arg: ToolOutputArg { json: "".to_string() },
                embedding: None,
                restrictions: None,
            };
            {
                let shinkai_tool = ShinkaiTool::Network(network_tool, true);

                self.sqlite_manager
                    .add_tool(shinkai_tool)
                    .await
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            }

            // Manually create another NetworkTool
            let youtube_tool = NetworkTool {
                name: "youtube_transcript_with_timestamps".to_string(),
                toolkit_name: "shinkai-tool-youtube-transcript".to_string(),
                description: "Takes a YouTube link and summarizes the content by creating multiple sections with a summary and a timestamp.".to_string(),
                version: "0.1".to_string(),
                provider: ShinkaiName::new("@@agent_provider.arb-sep-shinkai".to_string()).unwrap(),
                usage_type: usage_type.clone(),
                activated: true,
                config: vec![],
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("url".to_string(), "string".to_string(), "The YouTube link to summarize".to_string(), true);
                    params
                },
                output_arg: ToolOutputArg { json: "".to_string() },
                embedding: None,
                restrictions: None,
            };

            {
                let shinkai_tool = ShinkaiTool::Network(youtube_tool, true);
                self.sqlite_manager
                    .add_tool(shinkai_tool)
                    .await
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            }
        }

        // Check if ADD_TESTING_NETWORK_ECHO is set
        if std::env::var("ADD_TESTING_NETWORK_ECHO").unwrap_or_else(|_| "false".to_string()) == "true" {
            match self
                .sqlite_manager
                .get_tool_by_key("local:::shinkai-tool-echo:::shinkai__echo")
            {
                Ok(shinkai_tool) => {
                    if let ShinkaiTool::Deno(mut js_tool, _) = shinkai_tool {
                        js_tool.name = "network__echo".to_string();
                        let modified_tool = ShinkaiTool::Deno(js_tool, true);
                        self.sqlite_manager
                            .add_tool(modified_tool)
                            .await
                            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
                    }
                }
                Err(SqliteManagerError::ToolNotFound(_)) => {
                    eprintln!("Tool not found: local:::shinkai-tool-echo:::shinkai__echo");
                    // Handle the case where the tool is not found, if necessary
                }
                Err(e) => {
                    return Err(ToolError::DatabaseError(e.to_string()));
                }
            }

            match self
                .sqlite_manager
                .get_tool_by_key("local:::shinkai-tool-youtube-transcript:::shinkai__youtube_transcript")
            {
                Ok(shinkai_tool) => {
                    if let ShinkaiTool::Deno(mut js_tool, _) = shinkai_tool {
                        js_tool.name = "youtube_transcript_with_timestamps".to_string();
                        let modified_tool = ShinkaiTool::Deno(js_tool, true);
                        self.sqlite_manager
                            .add_tool(modified_tool)
                            .await
                            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
                    }
                }
                Err(SqliteManagerError::ToolNotFound(_)) => {
                    eprintln!("Tool not found: local:::shinkai-tool-youtube-transcript:::shinkai__youtube_transcript");
                    // Handle the case where the tool is not found, if necessary
                }
                Err(e) => {
                    return Err(ToolError::DatabaseError(e.to_string()));
                }
            }
        }

        Ok(())
    }

    pub async fn get_tool_by_name(&self, name: &str) -> Result<Option<ShinkaiTool>, ToolError> {
        match self.sqlite_manager.get_tool_by_key(name) {
            Ok(tool) => Ok(Some(tool)),
            Err(SqliteManagerError::ToolNotFound(_)) => Ok(None),
            Err(e) => Err(ToolError::DatabaseError(e.to_string())),
        }
    }

    pub async fn get_tool_by_name_and_version(
        &self,
        name: &str,
        version: Option<IndexableVersion>,
    ) -> Result<Option<ShinkaiTool>, ToolError> {
        match self.sqlite_manager.get_tool_by_key_and_version(name, version) {
            Ok(tool) => Ok(Some(tool)),
            Err(SqliteManagerError::ToolNotFound(_)) => Ok(None),
            Err(e) => Err(ToolError::DatabaseError(e.to_string())),
        }
    }

    pub async fn vector_search_enabled_tools(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let tool_headers = self
            .sqlite_manager
            .tool_vector_search(query, num_of_results, false, false)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Note: we can add more code here to filter out low confidence results
        let tool_headers = tool_headers.into_iter().map(|(tool, _)| tool).collect();
        Ok(tool_headers)
    }

    pub async fn vector_search_enabled_tools_with_network(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let tool_headers = self
            .sqlite_manager
            .tool_vector_search(query, num_of_results, false, true)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Note: we can add more code here to filter out low confidence results
        let tool_headers = tool_headers.into_iter().map(|(tool, _)| tool).collect();
        Ok(tool_headers)
    }

    pub async fn vector_search_all_tools(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let tool_headers = self
            .sqlite_manager
            .tool_vector_search(query, num_of_results, true, true)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        // Note: we can add more code here to filter out low confidence results
        let tool_headers = tool_headers.into_iter().map(|(tool, _)| tool).collect();
        Ok(tool_headers)
    }

    pub async fn call_function(
        &self,
        function_call: FunctionCall,
        context: &dyn InferenceChainContextTrait,
        shinkai_tool: &ShinkaiTool,
        node_name: ShinkaiName,
    ) -> Result<ToolCallFunctionResponse, LLMProviderError> {
        let _function_name = function_call.name.clone();
        let function_args = function_call.arguments.clone();

        match shinkai_tool {
            ShinkaiTool::Python(python_tool, _) => {
                let function_config = shinkai_tool.get_config_from_env();
                let function_config_vec: Vec<ToolConfig> = function_config.into_iter().collect();

                let node_env = fetch_node_environment();
                let node_storage_path = node_env
                    .node_storage_path
                    .clone()
                    .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
                let app_id = context.full_job().job_id().to_string();
                let tool_id = shinkai_tool.tool_router_key().to_string_without_version().clone();
                let tools = python_tool.tools.clone().unwrap_or_default();
                let support_files =
                    generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                        .await
                        .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;

                let envs = generate_execution_environment(
                    context.db(),
                    context.agent().clone().get_id().to_string(),
                    format!("jid-{}", tool_id),
                    format!("jid-{}", app_id),
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    format!("jid-{}", app_id),
                    &python_tool.oauth,
                )
                .await
                .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

                check_tool_config(
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    python_tool.config.clone(),
                )
                .await?;

                let folder = get_app_folder_path(node_env.clone(), context.full_job().job_id().to_string());
                let mounts = Node::v2_api_list_app_files_internal(folder.clone(), true);
                if let Err(e) = mounts {
                    eprintln!("Failed to list app files: {:?}", e);
                    return Err(LLMProviderError::FunctionExecutionError(format!("{:?}", e)));
                }
                let mounts = Some(mounts.unwrap_or_default());

                let result = python_tool
                    .run(
                        envs,
                        node_env.api_listen_address.ip().to_string(),
                        node_env.api_listen_address.port(),
                        support_files,
                        function_args,
                        function_config_vec,
                        node_storage_path,
                        app_id,
                        tool_id,
                        node_name,
                        false,
                        None,
                        mounts,
                    )
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(ToolCallFunctionResponse {
                    response: result_str,
                    function_call,
                });
            }
            ShinkaiTool::Rust(_, _) => {
                unimplemented!("Rust tool calls are not supported yet");
                // if let Some(rust_function) = RustToolFunctions::get_tool_function(&function_name) {
                //     let args: Vec<Box<dyn Any + Send>> = RustTool::convert_args_from_fn_call(function_args)?;
                //     let result = rust_function(context, args)
                //         .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                //     let result_str = result
                //         .downcast_ref::<String>()
                //         .ok_or_else(|| {
                //             LLMProviderError::InvalidFunctionResult(format!("Invalid result: {:?}", result))
                //         })?
                //         .clone();
                //     return Ok(ToolCallFunctionResponse {
                //         response: result_str,
                //         function_call,
                //     });
                // }
            }
            ShinkaiTool::Deno(deno_tool, _) => {
                let function_config = shinkai_tool.get_config_from_env();
                let function_config_vec: Vec<ToolConfig> = function_config.into_iter().collect();

                let node_env = fetch_node_environment();
                let node_storage_path = node_env
                    .node_storage_path
                    .clone()
                    .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
                let app_id = context.full_job().job_id().to_string();
                let tool_id = shinkai_tool.tool_router_key().to_string_without_version().clone();
                let tools = deno_tool.tools.clone().unwrap_or_default();
                let support_files =
                    generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                        .await
                        .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;

                let envs = generate_execution_environment(
                    context.db(),
                    context.agent().clone().get_id().to_string(),
                    format!("jid-{}", app_id),
                    format!("jid-{}", tool_id),
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    format!("jid-{}", app_id),
                    &deno_tool.oauth,
                )
                .await
                .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

                check_tool_config(
                    shinkai_tool.tool_router_key().to_string_without_version().clone(),
                    deno_tool.config.clone(),
                )
                .await?;

                let folder = get_app_folder_path(node_env.clone(), context.full_job().job_id().to_string());
                let mounts = Node::v2_api_list_app_files_internal(folder.clone(), true);
                if let Err(e) = mounts {
                    eprintln!("Failed to list app files: {:?}", e);
                    return Err(LLMProviderError::FunctionExecutionError(format!("{:?}", e)));
                }
                let mounts = Some(mounts.unwrap_or_default());

                let result = deno_tool
                    .run(
                        envs,
                        node_env.api_listen_address.ip().to_string(),
                        node_env.api_listen_address.port(),
                        support_files,
                        function_args,
                        function_config_vec,
                        node_storage_path,
                        app_id,
                        tool_id.clone(),
                        node_name,
                        false,
                        Some(tool_id),
                        mounts,
                    )
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(ToolCallFunctionResponse {
                    response: result_str,
                    function_call,
                });
            }
            ShinkaiTool::Network(network_tool, _) => {
                eprintln!("network tool with name {:?}", network_tool.name);

                let agent_payments_manager = context.my_agent_payments_manager();
                let (internal_invoice_request, wallet_balances) = {
                    // Start invoice request
                    let my_agent_payments_manager = match &agent_payments_manager {
                        Some(manager) => manager.lock().await,
                        None => {
                            eprintln!("call_function> Agent payments manager is not available");
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                "Agent payments manager is not available",
                            );
                            return Err(LLMProviderError::FunctionExecutionError(
                                "Agent payments manager is not available".to_string(),
                            ));
                        }
                    };

                    // Get wallet balances
                    let balances = match my_agent_payments_manager.get_balances(node_name.clone()).await {
                        Ok(balances) => balances,
                        Err(e) => {
                            eprintln!("Failed to get balances: {}", e);
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                format!("Failed to get balances: {}", e).as_str(),
                            );
                            return Err(LLMProviderError::FunctionExecutionError(format!(
                                "Failed to get balances: {}",
                                e
                            )));
                        }
                    };

                    // Send a Network Request Invoice
                    let invoice_request = match my_agent_payments_manager
                        .network_request_invoice(network_tool.clone(), UsageTypeInquiry::PerUse)
                        .await
                    {
                        Ok(request) => request,
                        Err(e) => {
                            eprintln!("Failed to request invoice: {}", e);
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                format!("Failed to request invoice: {}", e).as_str(),
                            );
                            return Err(LLMProviderError::FunctionExecutionError(format!(
                                "Failed to request invoice: {}",
                                e
                            )));
                        }
                    };
                    (invoice_request, balances)
                };

                eprintln!(
                    "call_function> internal_invoice_request: {:?}",
                    internal_invoice_request
                );

                // TODO: Send ws_message to the frontend saying requesting invoice to X and more context

                // Convert balances to Value
                let balances_value = match serde_json::to_value(&wallet_balances) {
                    Ok(value) => value,
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            format!("Failed to convert balances to Value: {}", e).as_str(),
                        );
                        return Err(LLMProviderError::FunctionExecutionError(format!(
                            "Failed to convert balances to Value: {}",
                            e
                        )));
                    }
                };

                // Note: there must be a better way to do this
                // Loop to check for the invoice unique_id
                let start_time = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(300); // 5 minutes
                let interval = std::time::Duration::from_millis(100); // 100ms
                let notification_content: Invoice;

                loop {
                    if start_time.elapsed() > timeout {
                        return Err(LLMProviderError::FunctionExecutionError(
                            "Timeout while waiting for invoice unique_id".to_string(),
                        ));
                    }

                    // Check if the invoice is paid
                    match context.db().get_invoice(&internal_invoice_request.unique_id.clone()) {
                        Ok(invoice) => {
                            eprintln!("invoice found: {:?}", invoice);

                            if invoice.status == InvoiceStatusEnum::Pending {
                                // Process the notification
                                notification_content = invoice;
                                break;
                            }
                        }
                        Err(_e) => {
                            // If invoice is not found, check for InvoiceNetworkError
                            match context
                                .db()
                                .get_invoice_network_error(&internal_invoice_request.unique_id.clone())
                            {
                                Ok(network_error) => {
                                    eprintln!("InvoiceNetworkError found: {:?}", network_error);
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!("InvoiceNetworkError details: {:?}", network_error),
                                    );
                                    // Return the user_error_message if available, otherwise a default message
                                    let error_message = network_error
                                        .user_error_message
                                        .unwrap_or_else(|| "Invoice network error encountered".to_string());
                                    return Err(LLMProviderError::FunctionExecutionError(error_message));
                                }
                                Err(_) => {
                                    // Continue waiting if neither invoice nor network error is found
                                }
                            }
                        }
                    }
                    tokio::time::sleep(interval).await;
                }

                // Convert notification_content to Value
                let notification_content_value = match serde_json::to_value(&notification_content) {
                    Ok(value) => value,
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            format!("Failed to convert notification_content to Value: {}", e).as_str(),
                        );
                        return Err(LLMProviderError::FunctionExecutionError(format!(
                            "Failed to convert notification_content to Value: {}",
                            e
                        )));
                    }
                };

                // Get the ws from the context
                {
                    let ws_manager = context.ws_manager_trait();

                    if let Some(ws_manager) = &ws_manager {
                        let ws_manager = ws_manager.lock().await;
                        let job = context.full_job();

                        let topic = WSTopic::Widget;
                        let subtopic = job.conversation_inbox_name.to_string();
                        let update = "".to_string();
                        let payment_metadata = PaymentMetadata {
                            tool_key: network_tool.name.clone(),
                            description: network_tool.description.clone(),
                            usage_type: network_tool.usage_type.clone(),
                            invoice_id: internal_invoice_request.unique_id.clone(),
                            invoice: notification_content_value.clone(),
                            function_args: function_args.clone(),
                            wallet_balances: balances_value.clone(),
                            error_message: None,
                        };

                        let widget = WSMessageType::Widget(WidgetMetadata::PaymentRequest(payment_metadata));
                        ws_manager.queue_message(topic, subtopic, update, widget, false).await;
                    } else {
                        return Err(LLMProviderError::FunctionExecutionError(
                            "WS manager is not available".to_string(),
                        ));
                    }
                }

                // Wait for the invoice to be paid for up to 5 minutes
                let start_time = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(300); // 5 minutes
                let interval = std::time::Duration::from_millis(100); // 100ms
                let invoice_result: Invoice;

                loop {
                    if start_time.elapsed() > timeout {
                        // Send a timeout notification via WebSocket
                        {
                            let ws_manager = context.ws_manager_trait();

                            if let Some(ws_manager) = &ws_manager {
                                let ws_manager = ws_manager.lock().await;
                                let job = context.full_job();

                                let topic = WSTopic::Widget;
                                let subtopic = job.conversation_inbox_name.to_string();
                                let update = "Timeout while waiting for invoice payment".to_string();
                                let payment_metadata = PaymentMetadata {
                                    tool_key: network_tool.name.clone(),
                                    description: network_tool.description.clone(),
                                    usage_type: network_tool.usage_type.clone(),
                                    invoice_id: internal_invoice_request.unique_id.clone(),
                                    invoice: notification_content_value.clone(),
                                    function_args: function_args.clone(),
                                    wallet_balances: balances_value.clone(),
                                    error_message: Some(update.clone()),
                                };

                                let widget = WSMessageType::Widget(WidgetMetadata::PaymentRequest(payment_metadata));
                                ws_manager.queue_message(topic, subtopic, update, widget, false).await;
                            }
                        }

                        return Err(LLMProviderError::FunctionExecutionError(
                            "Timeout while waiting for invoice payment".to_string(),
                        ));
                    }

                    // Check if the invoice is paid
                    match context.db().get_invoice(&internal_invoice_request.unique_id.clone()) {
                        Ok(invoice) => {
                            if invoice.status == InvoiceStatusEnum::Processed {
                                invoice_result = invoice;
                                break;
                            }
                        }
                        Err(e) => {
                            return Err(LLMProviderError::FunctionExecutionError(format!(
                                "Error while checking for invoice payment: {}",
                                e
                            )));
                        }
                    }

                    // Sleep for the interval before checking again
                    tokio::time::sleep(interval).await;
                }

                eprintln!("invoice_result: {:?}", invoice_result);

                // Try to parse the result_str and extract the "data" field
                let response = match serde_json::from_str::<serde_json::Value>(
                    &invoice_result.result_str.clone().unwrap_or_default(),
                ) {
                    Ok(parsed) => {
                        if let Some(data) = parsed.get("data") {
                            data.to_string()
                        } else {
                            invoice_result.result_str.clone().unwrap_or_default()
                        }
                    }
                    Err(_) => invoice_result.result_str.clone().unwrap_or_default(),
                };

                eprintln!("parsed response: {:?}", response);

                return Ok(ToolCallFunctionResponse {
                    response,
                    function_call,
                });
            }
        }
    }

    /// This function is used to call a JS function directly
    /// It's very handy for agent-to-agent communication
    pub async fn call_js_function(
        &self,
        function_args: serde_json::Map<String, Value>,
        requester_node_name: ShinkaiName,
        js_tool_name: &str,
    ) -> Result<String, LLMProviderError> {
        let shinkai_tool = self.get_tool_by_name(js_tool_name).await?;

        if shinkai_tool.is_none() {
            return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string()));
        }

        let shinkai_tool = shinkai_tool.unwrap();
        let function_config = shinkai_tool.get_config_from_env();
        let function_config_vec: Vec<ToolConfig> = function_config.into_iter().collect();

        let js_tool = match shinkai_tool.clone() {
            ShinkaiTool::Deno(js_tool, _) => js_tool,
            _ => return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string())),
        };

        let node_env = fetch_node_environment();
        let node_storage_path = node_env
            .node_storage_path
            .clone()
            .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
        let tools = js_tool.clone().tools.unwrap_or_default();
        let app_id = format!("external_{}", uuid::Uuid::new_v4());
        let tool_id = shinkai_tool.tool_router_key().clone().to_string_without_version();
        let support_files =
            generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;

        let oauth = match shinkai_tool.clone() {
            ShinkaiTool::Deno(deno_tool, _) => deno_tool.oauth.clone(),
            ShinkaiTool::Python(python_tool, _) => python_tool.oauth.clone(),
            _ => return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string())),
        };

        let env = generate_execution_environment(
            self.sqlite_manager.clone(),
            "".to_string(),
            format!("xid-{}", app_id),
            format!("xid-{}", tool_id),
            shinkai_tool.tool_router_key().clone().to_string_without_version(),
            // TODO: Pass data from the API
            "".to_string(),
            &oauth,
        )
        .await
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        check_tool_config(
            shinkai_tool.tool_router_key().clone().to_string_without_version(),
            function_config_vec.clone(),
        )
        .await?;

        let result = js_tool
            .run(
                env,
                node_env.api_listen_address.ip().to_string(),
                node_env.api_listen_address.port(),
                support_files,
                function_args,
                function_config_vec,
                node_storage_path,
                app_id,
                tool_id.clone(),
                // TODO Is this correct?
                requester_node_name,
                true,
                Some(tool_id),
                None,
            )
            .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
        let result_str =
            serde_json::to_string(&result).map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;

        return Ok(result_str);
    }

    pub async fn combined_tool_search(
        &self,
        query: &str,
        num_of_results: u64,
        include_disabled: bool,
        include_network: bool,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        // Sanitize the query to handle special characters
        let sanitized_query = query.replace(|c: char| !c.is_alphanumeric() && c != ' ', " ");

        // Start the timer for vector search
        let vector_start_time = Instant::now();
        let vector_search_result = self
            .sqlite_manager
            .tool_vector_search(&sanitized_query, num_of_results, include_disabled, include_network)
            .await;
        let vector_elapsed_time = vector_start_time.elapsed();
        println!("Time taken for vector search: {:?}", vector_elapsed_time);

        // Start the timer for FTS search
        let fts_start_time = Instant::now();
        let fts_search_result = self.sqlite_manager.search_tools_fts(&sanitized_query);
        let fts_elapsed_time = fts_start_time.elapsed();
        println!("Time taken for FTS search: {:?}", fts_elapsed_time);

        match (vector_search_result, fts_search_result) {
            (Ok(vector_tools), Ok(fts_tools)) => {
                let mut combined_tools = Vec::new();
                let mut seen_ids = std::collections::HashSet::new();

                // Always add the first FTS result if available
                if let Some(first_fts_tool) = fts_tools.first() {
                    if seen_ids.insert(first_fts_tool.tool_router_key.clone()) {
                        combined_tools.push(first_fts_tool.clone());
                    }
                }

                // Check if the top vector search result has a score under 0.2
                if let Some((tool, score)) = vector_tools.first() {
                    if *score < 0.2 {
                        if seen_ids.insert(tool.tool_router_key.clone()) {
                            combined_tools.push(tool.clone());
                        }
                    }
                }

                // Add remaining FTS results
                for tool in fts_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                // Add remaining vector search results
                for (tool, _) in vector_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                // Log the result count if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    println!("Number of combined tool results: {}", combined_tools.len());
                }

                Ok(combined_tools)
            }
            (Err(e), _) | (_, Err(e)) => Err(ToolError::DatabaseError(e.to_string())),
        }
    }
}
