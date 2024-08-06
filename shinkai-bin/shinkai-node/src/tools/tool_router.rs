use std::collections::HashMap;
use std::sync::{Arc, Weak};

use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChainContextTrait;
use crate::llm_provider::providers::shared::openai::{FunctionCall, FunctionCallResponse};
use crate::tools::error::ToolError;
use crate::tools::shinkai_tool::ShinkaiTool;
use crate::workflows::sm_executor::AsyncFunction;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource::{MapVectorResource, RetrievedNode};

use super::tool_router_dep::{function_call, tool_initialization, tool_management, tool_search};

#[derive(Debug, Clone, PartialEq)]
pub struct ToolRouter {
    pub routing_resources: HashMap<String, MapVectorResource>,
    pub started: bool,
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRouter {
    pub fn new() -> Self {
        ToolRouter {
            routing_resources: HashMap::new(),
            started: false,
        }
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    // Import methods from other modules
    pub async fn start(
        &mut self,
        generator: Box<dyn EmbeddingGenerator>,
        db: Weak<ShinkaiDB>,
        profile: ShinkaiName,
    ) -> Result<(), ToolError> {
        tool_initialization::start(self, generator, db, profile).await
    }

    pub fn add_shinkai_tool(
        &mut self,
        profile: &ShinkaiName,
        shinkai_tool: &ShinkaiTool,
        embedding: Embedding,
    ) -> Result<(), ToolError> {
        tool_management::add_shinkai_tool(self, profile, shinkai_tool, embedding)
    }

    pub fn delete_shinkai_tool(
        &mut self,
        profile: &ShinkaiName,
        tool_name: &str,
        toolkit_name: &str,
    ) -> Result<(), ToolError> {
        tool_management::delete_shinkai_tool(self, profile, tool_name, toolkit_name)
    }

    pub async fn add_js_toolkit(
        &mut self,
        profile: &ShinkaiName,
        toolkit: Vec<ShinkaiTool>,
        generator: Box<dyn EmbeddingGenerator>,
    ) -> Result<(), ToolError> {
        tool_management::add_js_toolkit(self, profile, toolkit, generator).await
    }

    pub fn remove_js_toolkit(
        &mut self,
        profile: &ShinkaiName,
        toolkit: Vec<ShinkaiTool>,
    ) -> Result<(), ToolError> {
        tool_management::remove_js_toolkit(self, profile, toolkit)
    }

    pub fn get_shinkai_tool(
        &self,
        profile: &ShinkaiName,
        tool_name: &str,
        toolkit_name: &str,
    ) -> Result<ShinkaiTool, ToolError> {
        tool_management::get_shinkai_tool(self, profile, tool_name, toolkit_name)
    }

    pub fn get_default_tools(&self, profile: &ShinkaiName) -> Result<Vec<ShinkaiTool>, ToolError> {
        tool_management::get_default_tools(self, profile)
    }

    pub fn syntactic_vector_search(
        &self,
        profile: &ShinkaiName,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Result<Vec<ShinkaiTool>, ToolError> {
        tool_search::syntactic_vector_search(self, profile, query, num_of_results, data_tag_names)
    }

    pub fn vector_search(
        &self,
        profile: &ShinkaiName,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiTool>, ToolError> {
        tool_search::vector_search(self, profile, query, num_of_results)
    }

    pub async fn workflow_search(
        &mut self,
        profile: ShinkaiName,
        embedding_generator: Box<dyn EmbeddingGenerator>,
        db: Arc<ShinkaiDB>,
        query: Embedding,
        name_query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiTool>, ToolError> {
        tool_search::workflow_search(self, profile, embedding_generator, db, query, name_query, num_of_results).await
    }

    pub async fn call_function(
        &self,
        function_call: FunctionCall,
        db: Arc<ShinkaiDB>,
        context: &dyn InferenceChainContextTrait,
        shinkai_tool: &ShinkaiTool,
        user_profile: &ShinkaiName,
    ) -> Result<FunctionCallResponse, LLMProviderError> {
        function_call::call_function(self, function_call, db, context, shinkai_tool, user_profile).await
    }

    pub fn ret_nodes_to_tools(&self, ret_nodes: &Vec<RetrievedNode>) -> Vec<ShinkaiTool> {
        let mut shinkai_tools = vec![];
        for ret_node in ret_nodes {
            if let Ok(data_string) = ret_node.node.get_text_content() {
                if let Ok(shinkai_tool) = ShinkaiTool::from_json(data_string) {
                    shinkai_tools.push(shinkai_tool);
                }
            }
        }
        shinkai_tools
    }
}


#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;

    use crate::tools::tool_router_dep::workflows_data;
    use crate::tools::workflow_tool::WorkflowTool;

    use super::*;
    use std::env;
    use std::io::Write;
    use std::{fs::File, time::Instant};

    #[test]
    fn test_parse_workflows_json() {
        let data = workflows_data::WORKFLOWS_JSON;

        // Start the timer
        let start_time = Instant::now();

        // Parse the JSON data into a generic JSON value
        let json_value: Value = serde_json::from_str(data).expect("Failed to parse JSON data");

        // Ensure the JSON value is an array
        let json_array = json_value.as_array().expect("Expected JSON data to be an array");

        // Iterate over the JSON array and manually parse each element
        for item in json_array {
            // Parse the shinkai_tool field
            let shinkai_tool_value = &item["shinkai_tool"];
            eprintln!("shinkai_tool_value: {:?}", shinkai_tool_value);
            let shinkai_tool: ShinkaiTool =
                serde_json::from_value(shinkai_tool_value.clone()).expect("Failed to parse shinkai_tool");

            // Parse the embedding field
            let embedding_value = &item["embedding"];
            let embedding: Embedding =
                serde_json::from_value(embedding_value.clone()).expect("Failed to parse embedding");

            // Check if embedding vector is not empty
            assert!(!embedding.vector.is_empty(), "Embedding vector is empty");

            // Check if tool name and description are not empty
            assert!(!shinkai_tool.name().is_empty(), "Tool name is empty");
            assert!(!shinkai_tool.description().is_empty(), "Tool description is empty");
        }

        // Stop the timer and calculate the duration
        let duration = start_time.elapsed();
        println!("Time taken to parse workflows JSON: {:?}", duration);
    }

    // #[tokio::test]
    /// Not really a test but rather a script. I should move it to a separate file soon (tm)
    /// It's just easier to have it here because it already has access to all the necessary dependencies
    async fn test_generate_static_workflows() {
        let generator = RemoteEmbeddingGenerator::new_default();

        let mut workflows_json_testing = Vec::new();
        let mut workflows_json = Vec::new();

        // Generate workflows for testing
        env::set_var("IS_TESTING", "1");
        let workflows_testing = WorkflowTool::static_tools();
        println!("Number of testing workflows: {}", workflows_testing.len());

        for workflow_tool in workflows_testing {
            let shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone());

            let embedding = if let Some(embedding) = workflow_tool.get_embedding() {
                embedding
            } else {
                generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap()
            };

            workflows_json_testing.push(json!({
                "embedding": embedding,
                "shinkai_tool": shinkai_tool
            }));
        }

        // Generate workflows for production
        env::set_var("IS_TESTING", "0");
        let workflows = WorkflowTool::static_tools();
        println!("Number of production workflows: {}", workflows.len());

        for workflow_tool in workflows {
            let shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone());

            let embedding = if let Some(embedding) = workflow_tool.get_embedding() {
                embedding
            } else {
                generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap()
            };

            workflows_json.push(json!({
                "embedding": embedding,
                "shinkai_tool": shinkai_tool
            }));
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
