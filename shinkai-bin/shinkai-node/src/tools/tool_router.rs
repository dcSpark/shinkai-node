use std::any::Any;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Instant;

use crate::lance_db::shinkai_lance_db::{LanceShinkaiDb, LATEST_ROUTER_DB_VERSION};
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::dsl_chain::dsl_inference_chain::DslChain;
use crate::llm_provider::execution::chains::dsl_chain::generic_functions::RustToolFunctions;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChainContextTrait;
use crate::llm_provider::providers::shared::openai::{FunctionCall, FunctionCallResponse};
use crate::tools::error::ToolError;
use crate::tools::shinkai_tool::ShinkaiTool;
use crate::tools::workflow_tool::WorkflowTool;
use crate::workflows::sm_executor::AsyncFunction;
use serde_json::Value;
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use tokio::sync::Mutex;

use super::js_toolkit::JSToolkit;
use super::rust_tools::RustTool;
use super::shinkai_tool::ShinkaiToolHeader;
use super::tool_router_dep::workflows_data;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChain;

#[derive(Clone)]
pub struct ToolRouter {
    pub lance_db: Arc<Mutex<LanceShinkaiDb>>,
}

impl ToolRouter {
    pub fn new(lance_db: Arc<Mutex<LanceShinkaiDb>>) -> Self {
        ToolRouter { lance_db }
    }

    pub async fn initialization(&self, generator: Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        let is_empty;
        let has_any_js_tools;
        {
            let lance_db = self.lance_db.lock().await;
            is_empty = lance_db.is_empty().await?;
            has_any_js_tools = lance_db.has_any_js_tools().await?;
        }

        if is_empty {
            // Add workflows
            let _ = self.add_static_workflows(generator).await;

            // Add JS tools
            let _ = self.add_js_tools().await;

            // Set the latest version in the database
            self.set_lancedb_version(LATEST_ROUTER_DB_VERSION).await?;
        } else if !has_any_js_tools {
            // Add JS tools
            let _ = self.add_js_tools().await;
        }

        Ok(())
    }

    pub async fn force_reinstall_all(&self, generator: Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        // Add workflows
        let _ = self.add_static_workflows(generator).await;

        // Add JS tools
        let _ = self.add_js_tools().await;

        Ok(())
    }

    async fn add_static_workflows(&self, generator: Box<dyn EmbeddingGenerator>) -> Result<(), ToolError> {
        let lance_db = self.lance_db.lock().await;
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
                    Ok(tool) => tool,
                    Err(e) => {
                        eprintln!("Failed to parse shinkai_tool: {}. JSON: {:?}", e, item);
                        continue; // Skip this item and continue with the next one
                    }
                };

                lance_db.set_tool(&shinkai_tool).await?;
            }
        } else {
            let workflows = WorkflowTool::static_tools();
            println!("Number of static workflows: {}", workflows.len());

            for workflow_tool in workflows {
                let shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), true);
                lance_db.set_tool(&shinkai_tool).await?;
            }
        }

        let duration = start_time.elapsed();
        if env::var("LOG_ALL").unwrap_or_default() == "1" {
            println!("Time taken to generate static workflows: {:?}", duration);
        }
        Ok(())
    }

    async fn add_js_tools(&self) -> Result<(), ToolError> {
        let start_time = Instant::now(); // Start the timer

        let tools = built_in_tools::get_tools();
        let lance_db = self.lance_db.lock().await;

        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
                lance_db.set_tool(&shinkai_tool).await?;
            }
        }

        let duration = start_time.elapsed(); // Calculate the duration
        println!("Time taken to add JS tools: {:?}", duration); // Print the duration

        Ok(())
    }

    pub async fn get_tool_by_name(&self, name: &str) -> Result<Option<ShinkaiTool>, ToolError> {
        let lance_db = self.lance_db.lock().await;
        lance_db
            .get_tool(name)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }

    pub async fn get_tools_by_names(&self, names: Vec<String>) -> Result<Vec<ShinkaiTool>, ToolError> {
        let lance_db = self.lance_db.lock().await;
        let mut tools = Vec::new();

        for name in names {
            match lance_db.get_tool(&name).await {
                Ok(Some(tool)) => tools.push(tool),
                Ok(None) => return Err(ToolError::ToolNotFound(name)),
                Err(e) => return Err(ToolError::DatabaseError(e.to_string())),
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
        let lance_db = self.lance_db.lock().await;
        let tool_headers = lance_db.vector_search_enabled_tools(query, num_of_results).await?;
        Ok(tool_headers)
    }

    pub async fn vector_search_all_tools(
        &self,
        query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        let lance_db = self.lance_db.lock().await;
        let tool_headers = lance_db.vector_search_all_tools(query, num_of_results).await?;
        Ok(tool_headers)
    }

    pub async fn workflow_search(
        &mut self,
        name_query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiToolHeader>, ToolError> {
        if name_query.is_empty() {
            return Ok(Vec::new());
        }

        let lance_db = self.lance_db.lock().await;
        let tool_headers = lance_db.workflow_vector_search(name_query, num_of_results).await?;
        Ok(tool_headers)
    }

    pub async fn call_function(
        &self,
        function_call: FunctionCall,
        context: &dyn InferenceChainContextTrait,
        shinkai_tool: &ShinkaiTool,
    ) -> Result<FunctionCallResponse, LLMProviderError> {
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
                    return Ok(FunctionCallResponse {
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
                return Ok(FunctionCallResponse {
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
                let tools = self.get_tools_by_names(functions_used).await?;

                dsl_inference.add_inference_function();
                dsl_inference.add_inference_no_ws_function();
                dsl_inference.add_opinionated_inference_function();
                dsl_inference.add_opinionated_inference_no_ws_function();
                dsl_inference.add_multi_inference_function();
                dsl_inference.add_all_generic_functions();
                dsl_inference.add_tools_from_router(tools).await?;

                let inference_result = dsl_inference.run_chain().await?;

                return Ok(FunctionCallResponse {
                    response: inference_result.response,
                    function_call,
                });
            }
        }

        Err(LLMProviderError::FunctionNotFound(function_name))
    }

    pub async fn get_current_lancedb_version(&self) -> Result<Option<String>, ToolError> {
        let lance_db = self.lance_db.lock().await;
        lance_db
            .get_current_version()
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }

    pub async fn set_lancedb_version(&self, version: &str) -> Result<(), ToolError> {
        let lance_db = self.lance_db.lock().await;
        lance_db
            .set_version(version)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;

    use crate::tools::workflow_tool::WorkflowTool;

    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;

    // #[tokio::test]
    /// Not really a test but rather a script. I should move it to a separate file soon (tm)
    /// It's just easier to have it here because it already has access to all the necessary dependencies
    #[allow(dead_code)]
    async fn test_generate_static_workflows() {
        let generator = RemoteEmbeddingGenerator::new_default_local();

        let mut workflows_json_testing = Vec::new();
        let mut workflows_json = Vec::new();

        // Generate workflows for testing
        env::set_var("IS_TESTING", "1");
        let workflows_testing = WorkflowTool::static_tools();
        println!("Number of testing workflows: {}", workflows_testing.len());

        for workflow_tool in workflows_testing {
            let mut shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), true);

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

        for workflow_tool in workflows {
            let mut shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone(), true);

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
            "pub static WORKFLOWS_JSON_TESTING: &str = r#\"{}\"#;",
            json_data_testing
        )
        .expect("Failed to write to file");
        writeln!(file, "pub static WORKFLOWS_JSON: &str = r#\"{}\"#;", json_data).expect("Failed to write to file");
    }
}
