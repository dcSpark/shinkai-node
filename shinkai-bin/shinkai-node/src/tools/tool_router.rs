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