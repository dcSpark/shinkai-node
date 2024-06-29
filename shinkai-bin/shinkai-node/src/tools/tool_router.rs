use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::dsl_chain::generic_functions::RustToolFunctions;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChainContextTrait;
use crate::llm_provider::providers::shared::openai::{FunctionCall, FunctionCallResponse};
use crate::tools::error::ToolError;
use crate::tools::rust_tools::RustTool;
use keyphrases::KeyPhraseExtractor;
use serde_json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::source::VRSourceReference;
use shinkai_vector_resources::vector_resource::{
    MapVectorResource, NodeContent, RetrievedNode, VectorResourceCore, VectorResourceSearch,
};

use super::shinkai_tool::ShinkaiTool;

/// A top level struct which indexes Tools (Rust or JS or Workflows) installed in the Shinkai Node
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRouter {
    pub routing_resources: HashMap<String, MapVectorResource>,
    // We use started so we can defer the initialization of the routing_resources using a
    // generator that may not be available at the time of creation
    started: bool,
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRouter {
    /// Create a new ToolRouter instance with empty routing_resources.
    pub fn new() -> Self {
        ToolRouter {
            routing_resources: HashMap::new(),
            started: false,
        }
    }

    /// Check if the ToolRouter instance has already been started.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Start the ToolRouter instance.
    pub async fn start(
        &mut self,
        generator: Box<dyn EmbeddingGenerator>,
        db: Weak<ShinkaiDB>,
        profile: ShinkaiName,
    ) -> Result<(), ToolError> {
        if self.started {
            return Err(ToolError::AlreadyStarted);
        }

        let name = "Tool Router";
        let desc = Some("Enables performing vector searches to find relevant tools.");
        let source = VRSourceReference::None;

        // Extract profile
        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;

        // Initialize the MapVectorResource and add all of the rust tools by default
        let mut routing_resource = MapVectorResource::new_empty(name, desc, source, true);

        // Add Rust tools
        Self::add_rust_tools(&mut routing_resource, generator.box_clone()).await;

        // Add JS tools
        if let Some(db) = db.upgrade() {
            Self::add_js_tools(&mut routing_resource, generator, db, profile.clone()).await;
        }

        self.routing_resources.insert(profile.to_string(), routing_resource);
        self.started = true;
        Ok(())
    }

    async fn add_rust_tools(routing_resource: &mut MapVectorResource, generator: Box<dyn EmbeddingGenerator>) {
        // Generate the static Rust tools
        let rust_tools = RustTool::static_tools(generator).await;

        // Insert each Rust tool into the routing resource
        for tool in rust_tools {
            let shinkai_tool = ShinkaiTool::Rust(tool.clone());
            // let parsing_tags = Self::extract_keywords_from_text(&shinkai_tool.description(), 10); // Extract keywords

            let _ = routing_resource.insert_text_node(
                shinkai_tool.tool_router_key(),
                shinkai_tool.to_json().unwrap(), // This unwrap should be safe because Rust Tools are not dynamic
                None,
                tool.tool_embedding.clone(),
                &vec![],
            );
        }
    }

    async fn add_js_tools(
        routing_resource: &mut MapVectorResource,
        generator: Box<dyn EmbeddingGenerator>,
        db: Arc<ShinkaiDB>,
        profile: ShinkaiName,
    ) {
        match db.all_tools_for_user(&profile) {
            Ok(tools) => {
                for tool in tools {
                    if let ShinkaiTool::JS(mut js_tool) = tool {
                        let js_lite_tool = js_tool.to_without_code();
                        let shinkai_tool = ShinkaiTool::JSLite(js_lite_tool);

                        let embedding = if let Some(embedding) = js_tool.embedding.clone() {
                            embedding
                        } else {
                            let new_embedding = generator
                                .generate_embedding_default(&shinkai_tool.format_embedding_string())
                                .await
                                .unwrap();
                            js_tool.embedding = Some(new_embedding.clone());
                            // Update the JS tool in the database
                            if let Err(e) = db.add_shinkai_tool(ShinkaiTool::JS(js_tool.clone()), profile.clone()) {
                                eprintln!("Error updating JS tool in DB: {:?}", e);
                            }
                            new_embedding
                        };

                        let _ = routing_resource.insert_text_node(
                            shinkai_tool.tool_router_key(),
                            shinkai_tool.to_json().unwrap(),
                            None,
                            embedding,
                            &vec![],
                        );
                    }
                }
            }
            Err(e) => eprintln!("Error fetching JS tools: {:?}", e),
        }
    }

    /// Adds a tool into the ToolRouter instance.
    pub fn add_shinkai_tool(
        &mut self,
        profile: &ShinkaiName,
        shinkai_tool: &ShinkaiTool,
        embedding: Embedding,
    ) -> Result<(), ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        let routing_resource = self
            .routing_resources
            .get_mut(&profile.to_string())
            .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
        let data = shinkai_tool.to_json()?;
        let router_key = shinkai_tool.tool_router_key();
        let metadata = None;

        // Setup the metadata based on tool type
        match routing_resource.get_root_node(router_key.clone()) {
            Ok(_) => {
                // If a Shinkai tool with same key is already found, error
                return Err(ToolError::ToolAlreadyInstalled(data.to_string()));
            }
            Err(_) => {
                // If no tool is found, insert new tool
                routing_resource._insert_kv_without_tag_validation(
                    &router_key,
                    NodeContent::Text(data),
                    metadata,
                    &embedding,
                    &vec![],
                );
            }
        }

        Ok(())
    }

    /// Deletes the tool inside of the ToolRouter given a valid id
    pub fn delete_shinkai_tool(
        &mut self,
        profile: &ShinkaiName,
        tool_name: &str,
        toolkit_name: &str,
    ) -> Result<(), ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        let routing_resource = self
            .routing_resources
            .get_mut(&profile.to_string())
            .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        routing_resource.print_all_nodes_exhaustive(None, false, false);
        routing_resource.remove_node_dt_specified(key, None, true)?;
        Ok(())
    }

    /// Adds a JSToolkit into the ToolRouter instance.
    pub async fn add_js_toolkit(
        &mut self,
        profile: &ShinkaiName,
        toolkit: Vec<ShinkaiTool>,
        generator: Box<dyn EmbeddingGenerator>,
    ) -> Result<(), ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        for tool in toolkit {
            if let ShinkaiTool::JS(mut js_tool) = tool {
                let js_lite_tool = js_tool.to_without_code();
                let shinkai_tool = ShinkaiTool::JSLite(js_lite_tool);

                let embedding = if let Some(embedding) = js_tool.embedding.clone() {
                    embedding
                } else {
                    let new_embedding = generator
                        .generate_embedding_default(&shinkai_tool.format_embedding_string())
                        .await
                        .unwrap();
                    js_tool.embedding = Some(new_embedding.clone());
                    new_embedding
                };

                self.add_shinkai_tool(&profile, &shinkai_tool, embedding)?;
            }
        }
        Ok(())
    }

    /// Removes a JSToolkit from the ToolRouter instance.
    pub fn remove_js_toolkit(&mut self, profile: &ShinkaiName, toolkit: Vec<ShinkaiTool>) -> Result<(), ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        for tool in toolkit {
            if let ShinkaiTool::JS(js_tool) = tool {
                let js_lite_tool = js_tool.to_without_code();
                let shinkai_tool = ShinkaiTool::JSLite(js_lite_tool);
                self.delete_shinkai_tool(&profile, &shinkai_tool.name(), &shinkai_tool.toolkit_name())?;
            }
        }
        Ok(())
    }

    /// Fetches the ShinkaiTool from the ToolRouter by parsing the internal Node
    /// within the ToolRouter.
    pub fn get_shinkai_tool(
        &self,
        profile: &ShinkaiName,
        tool_name: &str,
        toolkit_name: &str,
    ) -> Result<ShinkaiTool, ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        let routing_resource = self
            .routing_resources
            .get(&profile.to_string())
            .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        let node = routing_resource.get_root_node(key)?;
        ShinkaiTool::from_json(node.get_text_content()?)
    }

    /// Returns a list of ShinkaiTools of the most similar that
    /// have matching data tag names.
    pub fn syntactic_vector_search(
        &self,
        profile: &ShinkaiName,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Result<Vec<ShinkaiTool>, ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        let routing_resource = self
            .routing_resources
            .get(&profile.to_string())
            .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
        let nodes = routing_resource.syntactic_vector_search(query, num_of_results, data_tag_names);
        Ok(self.ret_nodes_to_tools(&nodes))
    }

    /// Returns a list of ShinkaiTools of the most similar.
    pub fn vector_search(
        &self,
        profile: &ShinkaiName,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiTool>, ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        let routing_resource = self
            .routing_resources
            .get(&profile.to_string())
            .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
        let nodes = routing_resource.vector_search(query, num_of_results);

        // Print out the score and toolkit name for each node
        for node in &nodes {
            if let Ok(shinkai_tool) = ShinkaiTool::from_json(node.node.get_text_content()?) {
                eprintln!(
                    "Node Score: {}, Toolkit Name: {}",
                    node.score,
                    shinkai_tool.toolkit_name()
                );
            }
        }
        Ok(self.ret_nodes_to_tools(&nodes))
    }

    /// Takes a list of RetrievedNodes and outputs a list of ShinkaiTools
    fn ret_nodes_to_tools(&self, ret_nodes: &Vec<RetrievedNode>) -> Vec<ShinkaiTool> {
        let mut shinkai_tools = vec![];
        for ret_node in ret_nodes {
            // Ignores tools added to the router which are invalid by matching on the Ok()
            if let Ok(data_string) = ret_node.node.get_text_content() {
                if let Ok(shinkai_tool) = ShinkaiTool::from_json(data_string) {
                    shinkai_tools.push(shinkai_tool);
                }
            }
        }
        shinkai_tools
    }

    /// Acquire the tool embedding for a given ShinkaiTool.
    pub fn get_tool_embedding(
        &self,
        profile: &ShinkaiName,
        shinkai_tool: &ShinkaiTool,
    ) -> Result<Embedding, ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        let routing_resource = self
            .routing_resources
            .get(&profile.to_string())
            .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;
        Ok(routing_resource.get_root_embedding(shinkai_tool.tool_router_key().to_string())?)
    }

    /// Extracts top N keywords from the given text.
    fn extract_keywords_from_text(text: &str, num_keywords: usize) -> Vec<String> {
        // Create a new KeyPhraseExtractor with a maximum of num_keywords keywords
        let extractor = KeyPhraseExtractor::new(text, num_keywords);

        // Get the keywords and their scores
        let keywords = extractor.get_keywords();

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        Ok(ToolRouter {
            routing_resources: serde_json::from_str(json).map_err(|_| ToolError::FailedJSONParsing)?,
            started: false,
        })
    }

    /// Calls a function given a function call, context, and ShinkaiTool.
    pub async fn call_function(
        &self,
        function_call: FunctionCall,
        context: &dyn InferenceChainContextTrait,
        shinkai_tool: &ShinkaiTool,
        user_profile: &ShinkaiName,
    ) -> Result<FunctionCallResponse, LLMProviderError> {
        let function_name = function_call.name.clone();
        let function_args = function_call.arguments.clone();

        match shinkai_tool {
            ShinkaiTool::Rust(_) => {
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
            ShinkaiTool::JS(js_tool) => {
                let result = js_tool
                    .run(function_args)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                let result_str = serde_json::to_string(&result)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                return Ok(FunctionCallResponse {
                    response: result_str,
                    function_call,
                });
            }
            ShinkaiTool::JSLite(js_lite_tool) => {
                // Fetch the full ShinkaiTool::JS
                let full_js_tool = self
                    .get_shinkai_tool(&user_profile, &js_lite_tool.name, &js_lite_tool.toolkit_name)
                    .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                if let ShinkaiTool::JS(js_tool) = full_js_tool {
                    let result = js_tool
                        .run(function_args)
                        .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                    let result_str = serde_json::to_string(&result)
                        .map_err(|e| LLMProviderError::FunctionExecutionError(e.to_string()))?;
                    return Ok(FunctionCallResponse {
                        response: result_str,
                        function_call,
                    });
                } else {
                    return Err(LLMProviderError::FunctionNotFound(function_name));
                }
            }
        }

        Err(LLMProviderError::FunctionNotFound(function_name))
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }
}
