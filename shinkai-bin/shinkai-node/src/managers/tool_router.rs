use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Instant;

use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, InferenceChainContextTrait};
use crate::tools::tool_definitions::definition_generation::{generate_tool_definitions, get_rust_tools};
use crate::utils::environment::fetch_node_environment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
use shinkai_tools_primitives::tools::deno_tools::ToolResult;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::js_toolkit::JSToolkit;
use shinkai_tools_primitives::tools::network_tool::NetworkTool;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::python_tools::PythonTool;
use shinkai_tools_primitives::tools::rust_tools::RustTool;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ToolRouter {
    pub sqlite_manager: Arc<RwLock<SqliteManager>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallFunctionResponse {
    pub response: String,
    pub function_call: FunctionCall,
}

impl ToolRouter {
    pub fn new(sqlite_manager: Arc<RwLock<SqliteManager>>) -> Self {
        ToolRouter { sqlite_manager }
    }

    pub async fn initialization(&self, generator: Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        let is_empty;
        let has_any_js_tools;
        {
            let sqlite_manager = self.sqlite_manager.read().await;
            is_empty = sqlite_manager
                .is_empty()
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

            has_any_js_tools = sqlite_manager
                .has_any_js_tools()
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        if is_empty {
            // Add JS tools
            let _ = self.add_deno_tools().await;
            let _ = self.add_rust_tools().await;
            let _ = self.add_python_tools().await;
            // Add static prompts
            let _ = self.add_static_prompts(&generator).await;
        } else if !has_any_js_tools {
            // Add JS tools
            let _ = self.add_deno_tools().await;
            let _ = self.add_rust_tools().await;
            let _ = self.add_python_tools().await;
        }

        Ok(())
    }

    pub async fn force_reinstall_all(&self, generator: &Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        // Add JS tools
        let _ = self.add_deno_tools().await;
        let _ = self.add_rust_tools().await;
        let _ = self.add_python_tools().await;
        let _ = self.add_static_prompts(generator).await;

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
            let sqlite_manager = self.sqlite_manager.write().await;
            sqlite_manager
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
        let mut sqlite_manager = self.sqlite_manager.write().await;
        sqlite_manager
            .add_tool(ShinkaiTool::Network(network_tool, true))
            .await
            .map(|_| ())
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }

    async fn add_rust_tools(&self) -> Result<(), ToolError> {
        let rust_tools = get_rust_tools();
        let mut sqlite_manager = self.sqlite_manager.write().await;
        for tool in rust_tools {
            let rust_tool = RustTool::new(
                tool.name,
                tool.description,
                tool.input_args,
                tool.output_arg,
                None,
                tool.tool_router_key,
            );
            sqlite_manager
                .add_tool(ShinkaiTool::Rust(rust_tool, true))
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }
        Ok(())
    }

    async fn add_deno_tools(&self) -> Result<(), ToolError> {
        let start_time = Instant::now(); // Start the timer

        let tools = built_in_tools::get_tools();

        let only_testing_js_tools =
            std::env::var("ONLY_TESTING_JS_TOOLS").unwrap_or_else(|_| "false".to_string()) == "true";
        let allowed_tools = vec![
            "shinkai-tool-echo",
            "shinkai-tool-coinbase-create-wallet",
            "shinkai-tool-coinbase-get-my-address",
            "shinkai-tool-coinbase-get-balance",
            "shinkai-tool-coinbase-get-transactions",
            "shinkai-tool-coinbase-send-tx",
            "shinkai-tool-coinbase-call-faucet",
        ];

        {
            let mut sqlite_manager = self.sqlite_manager.write().await;
            for (name, definition) in tools {
                if only_testing_js_tools && !allowed_tools.contains(&name.as_str()) {
                    continue; // Skip tools that are not in the allowed list
                }
                println!("Adding JS tool: {}", name);

                let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
                for tool in toolkit.tools {
                    let shinkai_tool = ShinkaiTool::Deno(tool.clone(), true);
                    sqlite_manager
                        .add_tool(shinkai_tool)
                        .await
                        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
                }
            }
        }

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
                version: "v0.1".to_string(),
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
                let mut sqlite_manager = self.sqlite_manager.write().await;
                let shinkai_tool = ShinkaiTool::Network(network_tool, true);

                sqlite_manager
                    .add_tool(shinkai_tool)
                    .await
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            }

            // Manually create another NetworkTool
            let youtube_tool = NetworkTool {
                name: "youtube_transcript_with_timestamps".to_string(),
                toolkit_name: "shinkai-tool-youtube-transcript".to_string(),
                description: "Takes a YouTube link and summarizes the content by creating multiple sections with a summary and a timestamp.".to_string(),
                version: "v0.1".to_string(),
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
                let mut sqlite_manager = self.sqlite_manager.write().await;
                sqlite_manager
                    .add_tool(shinkai_tool)
                    .await
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            }
        }

        // Check if ADD_TESTING_NETWORK_ECHO is set
        if std::env::var("ADD_TESTING_NETWORK_ECHO").unwrap_or_else(|_| "false".to_string()) == "true" {
            let sqlite_manager_read = self.sqlite_manager.read().await;
            match sqlite_manager_read.get_tool_by_key("local:::shinkai-tool-echo:::shinkai__echo") {
                Ok(shinkai_tool) => {
                    if let ShinkaiTool::Deno(mut js_tool, _) = shinkai_tool {
                        std::mem::drop(sqlite_manager_read);
                        js_tool.name = "network__echo".to_string();
                        let modified_tool = ShinkaiTool::Deno(js_tool, true);
                        let mut sqlite_manager = self.sqlite_manager.write().await;
                        sqlite_manager
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

            let sqlite_manager_read = self.sqlite_manager.read().await;
            match sqlite_manager_read
                .get_tool_by_key("local:::shinkai-tool-youtube-transcript:::shinkai__youtube_transcript")
            {
                Ok(shinkai_tool) => {
                    if let ShinkaiTool::Deno(mut js_tool, _) = shinkai_tool {
                        std::mem::drop(sqlite_manager_read);
                        js_tool.name = "youtube_transcript_with_timestamps".to_string();
                        let modified_tool = ShinkaiTool::Deno(js_tool, true);
                        let mut sqlite_manager = self.sqlite_manager.write().await;
                        sqlite_manager
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

        let duration = start_time.elapsed(); // Calculate the duration
        println!("Time taken to add JS tools: {:?}", duration); // Print the duration

        Ok(())
    }

    fn generate_google_search_tool() -> PythonTool {
        // Create parameters for Google search
        let mut params = Parameters::new();
        params.add_property(
            "query".to_string(),
            "string".to_string(),
            "The search query to look up".to_string(),
            true,
        );
        params.add_property(
            "num_results".to_string(),
            "number".to_string(),
            "Number of search results to return".to_string(),
            false,
        );

        let mut output_arg = ToolOutputArg::empty();
        output_arg.json = r#"{
        "query": "string",
        "results": [
            {
                "title": "string",
                "url": "string", 
                "description": "string"
            }
        ]
    }"#
        .to_string();

        let python_tool = PythonTool {
            toolkit_name: "google_search_shinkai".to_string(),
            embedding: None,
            name: "Google Search".to_string(),
            author: "Shinkai".to_string(),
            py_code: r#"
# /// script
# dependencies = [
# "googlesearch-python"
# ]
# ///
from googlesearch import search, SearchResult
from typing import List
from dataclasses import dataclass
import json

class CONFIG:
    pass

class INPUTS:
    query: str
    num_results: int = 10

class OUTPUT:
    results: List[SearchResult]
    query: str

async def run(c: CONFIG, p: INPUTS) -> OUTPUT:
    query = p.query
    if not query:
        raise ValueError("No search query provided")

    results = []
    try:
        results = search(query, num_results=p.num_results, advanced=True)
    except Exception as e:
        raise RuntimeError(f"Search failed: {str(e)}")

    output = OUTPUT()
    output.results = results
    output.query = query
    return output
"#
            .to_string(),
            tools: None,
            config: vec![],
            description: "Search the web using Google".to_string(),
            keywords: vec![
                "web search".to_string(),
                "google search".to_string(),
                "internet search".to_string(),
            ],
            input_args: params,
            output_arg,
            activated: true,
            result: ToolResult {
                r#type: "object".to_string(),
                properties: serde_json::json!({
                    "query": {"type": "string"},
                    "results": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": {"type": "string"},
                                "url": {"type": "string"},
                                "description": {"type": "string"}
                            },
                            "required": ["title", "url", "description"]
                        }
                    }
                }),
                required: vec!["query".to_string(), "results".to_string()],
            },
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
        };
        python_tool
    }

    async fn add_python_tools(&self) -> Result<(), ToolError> {
        let python_tools = vec![Self::generate_google_search_tool()];
        let mut sqlite_manager = self.sqlite_manager.write().await;
        for python_tool in python_tools {
            sqlite_manager
                .add_tool(ShinkaiTool::Python(python_tool, true))
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    pub async fn get_tool_by_name(&self, name: &str) -> Result<Option<ShinkaiTool>, ToolError> {
        match self.sqlite_manager.read().await.get_tool_by_key(name) {
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
            .read()
            .await
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
            .read()
            .await
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
            .read()
            .await
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
                let tool_id = shinkai_tool.tool_router_key().clone();
                let tools = python_tool.tools.clone().unwrap_or_default();
                let support_files =
                    generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                        .await
                        .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
                let mut envs = HashMap::new();

                let bearer = context
                    .db()
                    .read()
                    .await
                    .read_api_v2_key()
                    .unwrap_or_default()
                    .unwrap_or_default();
                let llm_provider = context.agent().clone().get_id().to_string();
                envs.insert("BEARER".to_string(), bearer);
                envs.insert(
                    "X_SHINKAI_TOOL_ID".to_string(),
                    format!("jid-{}", context.full_job().job_id()),
                );
                envs.insert(
                    "X_SHINKAI_APP_ID".to_string(),
                    format!("jid-{}", context.full_job().job_id()),
                );
                envs.insert(
                    "X_SHINKAI_INSTANCE_ID".to_string(),
                    format!("jid-{}", context.full_job().job_id()),
                );
                envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), llm_provider);
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
                        None,
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
                let tool_id = shinkai_tool.tool_router_key().clone();
                let tools = deno_tool.tools.clone().unwrap_or_default();
                let support_files =
                    generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                        .await
                        .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
                let mut envs = HashMap::new();
                let bearer = context
                    .db()
                    .read()
                    .await
                    .read_api_v2_key()
                    .unwrap_or_default()
                    .unwrap_or_default();
                let llm_provider = context.agent().clone().get_id().to_string();
                envs.insert("BEARER".to_string(), bearer);
                envs.insert(
                    "X_SHINKAI_TOOL_ID".to_string(),
                    format!("jid-{}", context.full_job().job_id()),
                );
                envs.insert(
                    "X_SHINKAI_APP_ID".to_string(),
                    format!("jid-{}", context.full_job().job_id()),
                );
                envs.insert(
                    "X_SHINKAI_INSTANCE_ID".to_string(),
                    format!("jid-{}", context.full_job().job_id()),
                );
                envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), llm_provider);
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
                        None,
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
                    match context
                        .db()
                        .read()
                        .await
                        .get_invoice(&internal_invoice_request.unique_id.clone())
                    {
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
                                .read()
                                .await
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
                    match context
                        .db()
                        .read()
                        .await
                        .get_invoice(&internal_invoice_request.unique_id.clone())
                    {
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
        let tool_id = shinkai_tool.tool_router_key().clone();
        let support_files =
            generate_tool_definitions(tools, CodeLanguage::Typescript, self.sqlite_manager.clone(), false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
        let mut envs = HashMap::new();
        envs.insert("BEARER".to_string(), "".to_string()); // TODO (How do we get the bearer?)
        envs.insert("X_SHINKAI_TOOL_ID".to_string(), "".to_string()); // TODO Pass data from the API
        envs.insert("X_SHINKAI_APP_ID".to_string(), "".to_string()); // TODO Pass data from the API
        envs.insert("X_SHINKAI_INSTANCE_ID".to_string(), "".to_string()); // TODO Pass data from the API
        envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), "".to_string()); // TODO Pass data from the API

        let result = js_tool
            .run(
                HashMap::new(),
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
            .read()
            .await
            .tool_vector_search(&sanitized_query, num_of_results, include_disabled, include_network)
            .await;
        let vector_elapsed_time = vector_start_time.elapsed();
        println!("Time taken for vector search: {:?}", vector_elapsed_time);

        // Start the timer for FTS search
        let fts_start_time = Instant::now();
        let fts_search_result = self.sqlite_manager.read().await.search_tools_fts(&sanitized_query);
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
