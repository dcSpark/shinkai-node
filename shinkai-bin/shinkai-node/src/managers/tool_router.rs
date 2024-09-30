use std::any::Any;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Instant;

use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::dsl_chain::dsl_inference_chain::DslChain;
use crate::llm_provider::execution::chains::dsl_chain::generic_functions::RustToolFunctions;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, InferenceChainContextTrait};
use crate::workflows::sm_executor::AsyncFunction;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_db::schemas::ws_types::{PaymentMetadata, WSMessageType, WidgetMetadata};
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_lancedb::lance_db::prompts::prompts_data;
use shinkai_lancedb::lance_db::shinkai_lance_db::{LanceShinkaiDb, LATEST_ROUTER_DB_VERSION};
use shinkai_message_primitives::schemas::custom_prompt::CustomPrompt;
use shinkai_message_primitives::schemas::invoices::{Invoice, InvoiceStatusEnum};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tool_offering::{
    AssetPayment, ToolPrice, UsageType, UsageTypeInquiry,
};
use shinkai_message_primitives::schemas::wallet_mixed::{Asset, NetworkIdentifier};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_tools_primitives::tools::argument::ToolArgument;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::js_toolkit::JSToolkit;
use shinkai_tools_primitives::tools::network_tool::NetworkTool;
use shinkai_tools_primitives::tools::rust_tools::RustTool;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use shinkai_tools_primitives::tools::tool_router_dep::workflows_data;
use shinkai_tools_primitives::tools::workflow_tool::WorkflowTool;
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use tokio::sync::RwLock;

use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChain;

#[derive(Clone)]
pub struct ToolRouter {
    pub lance_db: Arc<RwLock<LanceShinkaiDb>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallFunctionResponse {
    pub response: String,
    pub function_call: FunctionCall,
}

impl ToolRouter {
    pub fn new(lance_db: Arc<RwLock<LanceShinkaiDb>>) -> Self {
        ToolRouter { lance_db }
    }

    pub async fn initialization(&self, generator: Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        let is_empty;
        let has_any_js_tools;
        {
            let lance_db = self.lance_db.read().await;
            is_empty = lance_db.is_empty().await?;
            has_any_js_tools = lance_db.has_any_js_tools().await?;
        }

        if is_empty {
            // Add workflows
            let _ = self.add_static_workflows(&generator).await;

            // Add JS tools
            let _ = self.add_js_tools().await;

            // Add static prompts
            let _ = self.add_static_prompts(&generator).await;

            // Set the latest version in the database
            self.set_lancedb_version(LATEST_ROUTER_DB_VERSION).await?;
        } else if !has_any_js_tools {
            // Add JS tools
            let _ = self.add_js_tools().await;
        }

        self.lance_db.write().await.create_tool_indices_if_needed().await?;
        self.lance_db.write().await.create_prompt_indices_if_needed().await?;

        Ok(())
    }

    pub async fn force_reinstall_all(&self, generator: &Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        // Add workflows
        let _ = self.add_static_workflows(generator).await;

        // Add JS tools
        let _ = self.add_js_tools().await;

        let _ = self.add_static_prompts(generator).await;

        // Set the latest version in the database
        self.set_lancedb_version(LATEST_ROUTER_DB_VERSION).await?;

        self.lance_db.write().await.create_tool_indices_if_needed().await?;
        self.lance_db.write().await.create_prompt_indices_if_needed().await?;

        Ok(())
    }

    pub async fn add_static_prompts(&self, generator: &Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
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

        let json_value: Value = serde_json::from_str(prompts_data).expect("Failed to parse prompts JSON data");
        let json_array = json_value
            .as_array()
            .expect("Expected prompts JSON data to be an array");

        println!("Number of static prompts to add: {}", json_array.len());

        for item in json_array {
            let custom_prompt: Result<CustomPrompt, _> = serde_json::from_value(item.clone());
            let mut custom_prompt = match custom_prompt {
                Ok(prompt) => prompt,
                Err(e) => {
                    eprintln!("Failed to parse custom_prompt: {}. JSON: {:?}", e, item);
                    continue; // Skip this item and continue with the next one
                }
            };

            // Generate embedding if not present
            if custom_prompt.embedding.is_none() {
                let embedding = generator
                    .generate_embedding_default(&custom_prompt.text_for_embedding())
                    .await
                    .map_err(|e| ToolError::EmbeddingGenerationError(e.to_string()))?;
                custom_prompt.embedding = Some(embedding.vector);
            }

            let lance_db = self.lance_db.write().await;
            lance_db.set_prompt(custom_prompt).await?;
        }

        let duration = start_time.elapsed();
        if env::var("LOG_ALL").unwrap_or_default() == "1" {
            println!("Time taken to add static prompts: {:?}", duration);
        }
        Ok(())
    }

    async fn add_static_workflows(&self, generator: &Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        // Check if ONLY_TESTING_WORKFLOWS is set
        if env::var("ONLY_TESTING_WORKFLOWS").unwrap_or_default() == "1"
            || env::var("ONLY_TESTING_WORKFLOWS").unwrap_or_default().to_lowercase() == "true"
        {
            return Ok(()); // Return right away and don't add anything
        }

        let model_type = generator.model_type();
        let start_time = Instant::now();

        if let EmbeddingModelType::OllamaTextEmbeddingsInference(
            OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
        ) = model_type
        {
            let data = workflows_data::WORKFLOWS_JSON;
            let json_value: Value = serde_json::from_str(data).expect("Failed to parse JSON data");
            let json_array = json_value.as_array().expect("Expected JSON data to be an array");

            for item in json_array {
                let shinkai_tool: Result<ShinkaiTool, _> = serde_json::from_value(item.clone());

                let shinkai_tool = match shinkai_tool {
                    Ok(tool) => {
                        eprintln!("adding shinkai_tool (workflow): {:?}", tool.name());
                        tool
                    }
                    Err(e) => {
                        eprintln!("Failed to parse shinkai_tool: {}. JSON: {:?}", e, item);
                        continue; // Skip this item and continue with the next one
                    }
                };

                let lance_db = self.lance_db.write().await;
                lance_db.set_tool(&shinkai_tool).await?;
            }
        } else {
            let workflows = WorkflowTool::static_tools();
            println!("Number of static workflows: {}", workflows.len());

            for (workflow_tool, is_enabled) in workflows {
                let shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), is_enabled);
                let lance_db = self.lance_db.write().await;
                lance_db.set_tool(&shinkai_tool).await?;
            }
        }

        let duration = start_time.elapsed();
        if env::var("LOG_ALL").unwrap_or_default() == "1" {
            println!("Time taken to generate static workflows: {:?}", duration);
        }
        Ok(())
    }

    pub async fn add_network_tool(&self, network_tool: NetworkTool) -> Result<(), ToolError> {
        let lance_db = self.lance_db.write().await;
        lance_db.set_tool(&ShinkaiTool::Network(network_tool, true)).await?;
        Ok(())
    }

    async fn add_js_tools(&self) -> Result<(), ToolError> {
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

        for (name, definition) in tools {
            if only_testing_js_tools && !allowed_tools.contains(&name.as_str()) {
                continue; // Skip tools that are not in the allowed list
            }
            println!("Adding JS tool: {}", name);

            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
                let lance_db = self.lance_db.write().await;
                lance_db.set_tool(&shinkai_tool).await?;
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
                input_args: vec![ToolArgument {
                    name: "message".to_string(),
                    arg_type: "string".to_string(),
                    description: "".to_string(),
                    is_required: true,
                }],
                embedding: None,
                restrictions: None,
            };

            let shinkai_tool = ShinkaiTool::Network(network_tool, true);
            {
                let lance_db = self.lance_db.write().await;
                lance_db.set_tool(&shinkai_tool).await?;
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
                input_args: vec![ToolArgument {
                    name: "url".to_string(),
                    arg_type: "string".to_string(),
                    description: "The URL of the YouTube video".to_string(),
                    is_required: true,
                }],
                embedding: None,
                restrictions: None,
            };

            let shinkai_tool = ShinkaiTool::Network(youtube_tool, true);
            let lance_db = self.lance_db.write().await;
            lance_db.set_tool(&shinkai_tool).await?;
        }

        // Check if ADD_TESTING_NETWORK_ECHO is set
        if std::env::var("ADD_TESTING_NETWORK_ECHO").unwrap_or_else(|_| "false".to_string()) == "true" {
            let lance_db = self.lance_db.read().await;
            if let Some(shinkai_tool) = lance_db.get_tool("local:::shinkai-tool-echo:::shinkai__echo").await? {
                if let ShinkaiTool::JS(mut js_tool, _) = shinkai_tool {
                    js_tool.name = "network__echo".to_string();
                    let modified_tool = ShinkaiTool::JS(js_tool, true);
                    let lance_db = self.lance_db.write().await;
                    lance_db.set_tool(&modified_tool).await?;
                }
            }

            let lance_db = self.lance_db.read().await;
            if let Some(shinkai_tool) = lance_db
                .get_tool("local:::shinkai-tool-youtube-transcript:::shinkai__youtube_transcript")
                .await?
            {
                if let ShinkaiTool::JS(mut js_tool, _) = shinkai_tool {
                    js_tool.name = "youtube_transcript_with_timestamps".to_string();
                    let modified_tool = ShinkaiTool::JS(js_tool, true);
                    let lance_db = self.lance_db.write().await;
                    lance_db.set_tool(&modified_tool).await?;
                }
            }
        }

        let duration = start_time.elapsed(); // Calculate the duration
        println!("Time taken to add JS tools: {:?}", duration); // Print the duration

        Ok(())
    }

    pub async fn get_tool_by_name(&self, name: &str) -> Result<Option<ShinkaiTool>, ToolError> {
        let lance_db = self.lance_db.read().await;
        lance_db
            .get_tool(name)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }

    pub async fn get_tools_by_names_with_smart_retry(&self, names: Vec<String>) -> Result<Vec<ShinkaiTool>, ToolError> {
        let lance_db = self.lance_db.read().await;
        let mut tools = Vec::new();

        for name in names {
            match lance_db.get_tool(&name).await {
                Ok(Some(tool)) => tools.push(tool),
                Ok(None) => {
                    // Perform a vector search if the tool is not found
                    let search_results = lance_db
                        .vector_search_all_tools(&name, 10, true)
                        .await
                        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

                    // Search for the result that has the same name
                    if let Some(matching_result) = search_results.iter().find(|result| result.name == name) {
                        match lance_db.get_tool(&matching_result.tool_router_key).await {
                            Ok(Some(tool)) => tools.push(tool),
                            Ok(None) => {
                                eprintln!("Tool not found: {}", name);
                                continue; // Skip this tool and continue with the next one
                            }
                            Err(e) => {
                                eprintln!("Database error: {}", e);
                                continue; // Skip this tool and continue with the next one
                            }
                        }
                    } else {
                        eprintln!("Tool not found: {}", name);
                        continue; // Skip this tool and continue with the next one
                    }
                }
                Err(e) => {
                    eprintln!("Database error: {}", e);
                    continue; // Skip this tool and continue with the next one
                }
            }
        }

        Ok(tools)
    }

    pub async fn get_workflow(&self, name: &str) -> Result<Option<Workflow>, ToolError> {
        if let Some(tool) = self.get_tool_by_name(name).await? {
            if let ShinkaiTool::Workflow(workflow, _) = tool {
                return Ok(Some(workflow.workflow));
            }
        }
        Ok(None)
    }

    pub async fn vector_search_enabled_tools(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let lance_db = self.lance_db.read().await;
        let tool_headers = lance_db
            .vector_search_enabled_tools(query, num_of_results, false)
            .await?;
        Ok(tool_headers)
    }

    pub async fn vector_search_enabled_tools_with_network(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let lance_db = self.lance_db.read().await;
        let tool_headers = lance_db
            .vector_search_enabled_tools(query, num_of_results, true)
            .await?;
        Ok(tool_headers)
    }

    pub async fn vector_search_all_tools(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let lance_db = self.lance_db.read().await;
        let tool_headers = lance_db.vector_search_all_tools(query, num_of_results, false).await?;
        Ok(tool_headers)
    }

    pub async fn workflow_search(
        &self,
        name_query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        if name_query.is_empty() {
            return Ok(Vec::new());
        }

        let lance_db = self.lance_db.read().await;
        let tool_headers = lance_db.workflow_vector_search(name_query, num_of_results).await?;
        Ok(tool_headers)
    }

    pub async fn call_function(
        &self,
        function_call: FunctionCall,
        context: &dyn InferenceChainContextTrait,
        shinkai_tool: &ShinkaiTool,
    ) -> Result<ToolCallFunctionResponse, LLMProviderError> {
        let function_name = function_call.name.clone();
        let function_args = function_call.arguments.clone();

        match shinkai_tool {
            ShinkaiTool::Rust(_, _) => {
                if let Some(rust_function) = RustToolFunctions::get_tool_function(&function_name) {
                    let args: Vec<Box<dyn Any + Send>> = RustTool::convert_args_from_fn_call(function_args)?;
                    let result = rust_function(context, args)
                        .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                    let result_str = result
                        .downcast_ref::<String>()
                        .ok_or_else(|| {
                            LLMProviderError::InvalidFunctionResult(format!("Invalid result: {:?}", result))
                        })?
                        .clone();
                    return Ok(ToolCallFunctionResponse {
                        response: result_str,
                        function_call,
                    });
                }
            }
            ShinkaiTool::JS(js_tool, _) => {
                let function_config = shinkai_tool.get_config_from_env();
                let result = js_tool
                    .run(function_args, function_config)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(ToolCallFunctionResponse {
                    response: result_str,
                    function_call,
                });
            }
            ShinkaiTool::Workflow(workflow_tool, _) => {
                let functions: HashMap<String, Box<dyn AsyncFunction>> = HashMap::new();

                let mut dsl_inference =
                    DslChain::new(Box::new(context.clone_box()), workflow_tool.workflow.clone(), functions);

                let functions_used = workflow_tool
                    .workflow
                    .extract_function_names()
                    .into_iter()
                    .filter(|name| name.starts_with("shinkai__"))
                    .collect::<Vec<_>>();
                let tools = self.get_tools_by_names_with_smart_retry(functions_used).await?;

                dsl_inference.add_inference_function();
                dsl_inference.add_inference_no_ws_function();
                dsl_inference.add_baml_inference_function();
                dsl_inference.add_opinionated_inference_function();
                dsl_inference.add_opinionated_inference_no_ws_function();
                dsl_inference.add_multi_inference_function();
                dsl_inference.add_all_generic_functions();
                dsl_inference.add_tools_from_router(tools).await?;

                let inference_result = dsl_inference.run_chain().await?;

                return Ok(ToolCallFunctionResponse {
                    response: inference_result.response,
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
                            return Err(LLMProviderError::FunctionExecutionError(
                                "Agent payments manager is not available".to_string(),
                            ));
                        }
                    };

                    // Get wallet balances
                    let balances = my_agent_payments_manager.get_balances().await.map_err(|e| {
                        LLMProviderError::FunctionExecutionError(format!("Failed to get balances: {}", e))
                    })?;

                    // Send a Network Request Invoice
                    let invoice_request = match my_agent_payments_manager
                        .request_invoice(network_tool.clone(), UsageTypeInquiry::PerUse)
                        .await
                    {
                        Ok(request) => request,
                        Err(e) => {
                            return Err(LLMProviderError::FunctionExecutionError(format!(
                                "Failed to request invoice: {}",
                                e
                            )));
                        }
                    };
                    (invoice_request, balances)
                };

                // Convert balances to Value
                let balances_value = serde_json::to_value(&wallet_balances).map_err(|e| {
                    LLMProviderError::FunctionExecutionError(format!("Failed to convert balances to Value: {}", e))
                })?;

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
                            // Nothing to do here
                        }
                    }
                    tokio::time::sleep(interval).await;
                }

                // Convert notification_content to Value
                let notification_content_value = serde_json::to_value(&notification_content).map_err(|e| {
                    LLMProviderError::FunctionExecutionError(format!(
                        "Failed to convert notification_content to Value: {}",
                        e
                    ))
                })?;

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
                            invoice: notification_content_value,
                            function_args: function_args.clone(),
                            wallet_balances: balances_value,
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

        Err(LLMProviderError::FunctionNotFound(function_name))
    }

    /// This function is used to call a JS function directly
    /// It's very handy for agent-to-agent communication
    pub async fn call_js_function(
        &self,
        function_args: serde_json::Map<String, Value>,
        js_tool_name: &str,
    ) -> Result<String, LLMProviderError> {
        let shinkai_tool = self.get_tool_by_name(js_tool_name).await?;

        if shinkai_tool.is_none() {
            return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string()));
        }

        let shinkai_tool = shinkai_tool.unwrap();
        let function_config = shinkai_tool.get_config_from_env();

        let js_tool = match shinkai_tool {
            ShinkaiTool::JS(js_tool, _) => js_tool,
            _ => return Err(LLMProviderError::FunctionNotFound(js_tool_name.to_string())),
        };

        let result = js_tool
            .run(function_args, function_config)
            .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
        let result_str =
            serde_json::to_string(&result).map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;

        return Ok(result_str);
    }

    pub async fn get_current_lancedb_version(&self) -> Result<Option<String>, ToolError> {
        let lance_db = self.lance_db.read().await;
        lance_db
            .get_current_version()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }

    pub async fn set_lancedb_version(&self, version: &str) -> Result<(), ToolError> {
        let lance_db = self.lance_db.write().await;
        lance_db
            .set_version(version)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use shinkai_baml::baml_builder::BamlConfig;
    use shinkai_baml::baml_builder::ClientConfig;
    use shinkai_baml::baml_builder::GeneratorConfig;
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
    use tokio::task;
    use regex::Regex;

    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;

    /// Not really a test but rather a script. I should move it to a separate file soon (tm)
    /// It's just easier to have it here because it already has access to all the necessary dependencies
    // #[tokio::test]
    #[allow(dead_code)]
    async fn test_generate_static_workflows() {
        let generator = RemoteEmbeddingGenerator::new_default_local();

        let mut workflows_json_testing = Vec::new();
        let mut workflows_json = Vec::new();

        // Generate workflows for testing
        env::set_var("IS_TESTING", "1");
        let workflows_testing = WorkflowTool::static_tools();
        println!("Number of testing workflows: {}", workflows_testing.len());

        for (workflow_tool, is_enabled) in workflows_testing {
            let mut shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), is_enabled);

            let embedding = if let Some(embedding) = workflow_tool.get_embedding() {
                embedding
            } else {
                generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap()
            };

            shinkai_tool.set_embedding(embedding);
            workflows_json_testing.push(json!(shinkai_tool));
        }

        // Generate workflows for production
        env::set_var("IS_TESTING", "0");
        let workflows = WorkflowTool::static_tools();
        println!("Number of production workflows: {}", workflows.len());

        for (workflow_tool, is_enabled) in workflows {
            let mut shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), is_enabled);

            let embedding = if let Some(embedding) = workflow_tool.get_embedding() {
                embedding
            } else {
                generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap()
            };

            shinkai_tool.set_embedding(embedding);
            workflows_json.push(json!(shinkai_tool));
        }

        let json_data_testing =
            serde_json::to_string(&workflows_json_testing).expect("Failed to serialize testing workflows");
        let json_data = serde_json::to_string(&workflows_json).expect("Failed to serialize production workflows");

        // Print the current directory
        let current_dir = env::current_dir().expect("Failed to get current directory");
        println!("Current directory: {:?}", current_dir);

        let mut file = File::create("../../tmp/workflows_data.rs").expect("Failed to create file");
        writeln!(
            file,
            "pub static WORKFLOWS_JSON_TESTING: &str = r###\"{}\"###;",
            json_data_testing
        )
        .expect("Failed to write to file");
        writeln!(file, "pub static WORKFLOWS_JSON: &str = r###\"{}\"###;", json_data).expect("Failed to write to file");
    }

    /// Not really a test but rather a script. I should move it to a separate file soon (tm)
    /// It's just easier to have it here because it already has access to all the necessary dependencies
    // #[tokio::test]
    #[allow(dead_code)]
    async fn test_generate_workflow_playground_documentation() {
        /*
        - read all the workflows
        - read all the JS tools
        - read all the Rust tools
        - (eventually) read all the Python tools
        - read the baml file
        - (do we need prompts here? is there a way to read them from the playground?)
        - generate the documentation
        - save it to a file
         */

        let mut tools_json = Vec::new();
        let mut serialized_tools = Vec::new();
        let mut documentation_results = Vec::new();

        let workflows = WorkflowTool::static_tools();
        println!("Number of production workflows: {}", workflows.len());

        for (workflow_tool, is_enabled) in workflows {
            let shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), is_enabled);
            tools_json.push(json!(shinkai_tool));

            let tool_content = serde_json::to_string(&shinkai_tool)
                .expect("Failed to serialize workflow")
                .trim_start_matches('{')
                .trim_end_matches('}')
                .to_string();

            serialized_tools.push(tool_content);
        }

        // Process JS tools
        let tools = built_in_tools::get_tools();
        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let mut mut_tool = tool.clone();
                mut_tool.embedding = None;
                mut_tool.js_code = "".to_string();
                let shinkai_tool = ShinkaiTool::JS(mut_tool, true);
                
                tools_json.push(json!(shinkai_tool));

                let tool_content = serde_json::to_string(&shinkai_tool)
                    .expect("Failed to serialize JS tool")
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .to_string();

                serialized_tools.push(tool_content);
            }
        }

        for workflow_content in serialized_tools {
            let mut attempts = 0;
            let max_attempts = 3;
            let mut success = false;

            while attempts < max_attempts && !success {
                attempts += 1;

                // Process the workflow using BAML
                let generator_config = GeneratorConfig {
                    output_type: "typescript".to_string(),
                    output_dir: "../src/".to_string(),
                    version: "0.55.3".to_string(),
                    default_client_mode: "async".to_string(),
                };

                let client_config = ClientConfig {
                    provider: "ollama".to_string(),
                    base_url: "http://localhost:11434/v1".to_string(),
                    model: "mistral-small:22b".to_string(),
                    default_role: "user".to_string(),
                };

                eprintln!("\n\nworkflow_content: {:?}", workflow_content);

                let baml_config = BamlConfig::builder(generator_config, client_config)
                    .dsl_class_file(
                        r##"
                        class InputArg {
                            name string @description(#"The name of the input argument"#)
                            arg_type string @description(#"The type of the input argument"#)
                            description string @description(#"The description of the input argument"#)
                            is_required bool @description(#"Whether the input argument is required"#)
                        }

                        class Answer {
                            name string @description(#"The name of the function. Don't include special characters like _ or - instead create an space between words."#)
                            fn_name string @description(#"The name of the function. It's obtained from tool_router_key by removing the prefix e.g., \"local:::@@official.shinkai:::extensive_summary\" -> \"extensive_summary\""#)
                            description string @description(#"The description of the function"#)
                            tool_type string @description(#"The type of the tool E.g. Workflow, Prompt, JS Tool"#)
                            author string @description(#"The author of the function"#)
                            version string @description(#"The version of the function"#)
                            input_args InputArg[] @description(#"The input arguments of the function"#)
                            config string | null @description(#"The config of the function"#)
                        }

                        function DocumentFunction(function_string: string) -> Answer {
                            client ShinkaiProvider

                            prompt #"
                            Parse the following json and return a structured representation of the data in the schema below. Don't include comments in the JSON.

                            Resume:
                            ---
                            {{ function_string }}
                            ---
                                
                            {{ ctx.output_format }}

                            JSON:
                            {{ _.role("user") }}
                            "#
                        }
                        "##,
                    )
                    .input(&workflow_content)
                    .function_name("DocumentFunction")
                    .param_name("function_string")
                    .build();

                let env_vars = HashMap::new();
                let runtime = baml_config.initialize_runtime(env_vars).unwrap();
                // Spawn a blocking task to run the blocking code
                let result = task::spawn_blocking(move || baml_config.execute(&runtime, true))
                    .await
                    .unwrap()
                    .unwrap();

                eprintln!("result: {:?}", result);

                // Remove comments from the result string using regex
                let re = Regex::new(r",\s*//.*").unwrap();
                let cleaned_result = re.replace_all(&result, "");

                // Deserialize the cleaned result string into a JSON object
                match serde_json::from_str::<serde_json::Value>(&cleaned_result) {
                    Ok(result_json) => {
                        documentation_results.push(result_json);
                        success = true;
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize result (attempt {}): {}", attempts, e);
                        if attempts >= max_attempts {
                            panic!("Failed to deserialize result after {} attempts: {}", max_attempts, e);
                        }
                    }
                }
            }
        }

        println!("Documentation results: {:?}", documentation_results);

        // Serialize documentation results to JSON
        let json_data =
            serde_json::to_string_pretty(&documentation_results).expect("Failed to serialize documentation results");

        // Print the current directory
        let current_dir = env::current_dir().expect("Failed to get current directory");
        println!("Current directory: {:?}", current_dir);

        // Write the documentation results to a file
        let mut file = File::create("../../tmp/documentation_results.json").expect("Failed to create file");
        writeln!(file, "{}", json_data).expect("Failed to write to file");
    }
}