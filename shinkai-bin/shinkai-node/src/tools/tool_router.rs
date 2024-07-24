use std::any::Any;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Weak};
use std::time::Instant;

use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::dsl_chain::dsl_inference_chain::DslChain;
use crate::llm_provider::execution::chains::dsl_chain::generic_functions::RustToolFunctions;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChain;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChainContextTrait;
use crate::llm_provider::providers::shared::openai::{FunctionCall, FunctionCallResponse};
use crate::tools::error::ToolError;
use crate::tools::rust_tools::RustTool;
use crate::tools::workflows_data;
use keyphrases::KeyPhraseExtractor;
use serde_json::{self, Value};
use shinkai_dsl::sm_executor::AsyncFunction;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::source::VRSourceReference;
use shinkai_vector_resources::vector_resource::{
    MapVectorResource, NodeContent, RetrievedNode, VectorResourceCore, VectorResourceSearch,
};

use super::js_toolkit::JSToolkit;
use super::shinkai_tool::ShinkaiTool;
use super::workflow_tool::WorkflowTool;

/// A top level struct which indexes Tools (Rust or JS or Workflows) installed in the Shinkai Node
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRouter {
    pub routing_resources: HashMap<String, MapVectorResource>,
    // pub workflows_for_search: RwLock<HashMap<ShinkaiName, ShinkaiTool>>,
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
            // workflows: RwLock::new(HashMap::new()),
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

        // Add JS tools and workflows
        if let Some(db) = db.upgrade() {
            // Add static workflows
            Self::add_static_workflows(
                &mut routing_resource,
                generator.box_clone(),
                db.clone(),
                profile.clone(),
            )
            .await;
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

    async fn add_static_workflows(
        routing_resource: &mut MapVectorResource,
        generator: Box<dyn EmbeddingGenerator>,
        db: Arc<ShinkaiDB>,
        profile: ShinkaiName,
    ) {
        // Measure the time it takes to generate static workflows
        let start_time = Instant::now();

        // New Approach
        // // Parse the JSON data into a generic JSON value
        // let data = workflows_data::WORKFLOWS_JSON;
        // let json_value: Value = serde_json::from_str(data).expect("Failed to parse JSON data");
        // let json_array = json_value.as_array().expect("Expected JSON data to be an array");

        // // Insert each workflow into the routing resource and save to the database
        // for item in json_array {
        //     // Parse the shinkai_tool field
        //     let shinkai_tool_value = &item["shinkai_tool"];
        //     let shinkai_tool: ShinkaiTool =
        //         serde_json::from_value(shinkai_tool_value.clone()).expect("Failed to parse shinkai_tool");

        //     // Parse the embedding field
        //     let embedding_value = &item["embedding"];
        //     let embedding: Embedding =
        //         serde_json::from_value(embedding_value.clone()).expect("Failed to parse embedding");

        //     let _ = routing_resource.insert_text_node(
        //         shinkai_tool.tool_router_key(),
        //         shinkai_tool.to_json().unwrap(),
        //         None,
        //         embedding,
        //         &vec![],
        //     );

        //     // Save the workflow to the database
        //     if let ShinkaiTool::Workflow(workflow_tool) = &shinkai_tool {
        //         if let Err(e) = db.save_workflow(workflow_tool.workflow.clone(), profile.clone()) {
        //             eprintln!("Error saving workflow to DB: {:?}", e);
        //         }
        //     }
        // }

        // old Approach
        // let duration = start_time.elapsed();
        // println!("Time taken to generate static workflows: {:?}", duration);

        // // Generate the static workflows
        // let workflows = WorkflowTool::static_tools();
        // println!("Number of static workflows: {}", workflows.len());

        // let duration = start_time.elapsed();
        // println!("Time taken to generate static workflows: {:?}", duration);

        // // Insert each workflow into the routing resource and save to the database
        // for workflow_tool in workflows {
        //     let shinkai_tool = ShinkaiTool::Workflow(workflow_tool.clone());

        //     let embedding = if let Some(embedding) = workflow_tool.get_embedding() {
        //         embedding
        //     } else {
        //         generator
        //             .generate_embedding_default(&shinkai_tool.format_embedding_string())
        //             .await
        //             .unwrap()
        //     };

        //     let _ = routing_resource.insert_text_node(
        //         shinkai_tool.tool_router_key(),
        //         shinkai_tool.to_json().unwrap(),
        //         None,
        //         embedding,
        //         &vec![],
        //     );

        //     // Save the workflow to the database
        //     if let Err(e) = db.save_workflow(workflow_tool.workflow.clone(), profile.clone()) {
        //         eprintln!("Error saving workflow to DB: {:?}", e);
        //     }
        // }
        // let duration = start_time.elapsed();
        // println!("Time taken to generate static workflows: {:?}", duration);
    }

    async fn add_js_tools(
        routing_resource: &mut MapVectorResource,
        generator: Box<dyn EmbeddingGenerator>,
        db: Arc<ShinkaiDB>,
        profile: ShinkaiName,
    ) {
        // Add static JS tools
        let tools = built_in_tools::get_tools();
        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition]);
            db.add_jstoolkit(toolkit.clone(), profile.clone()).unwrap();
        }

        // Add user-specific JS tools and static tools
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
            if let ShinkaiTool::JS(mut js_tool) = tool.clone() {
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

                // We save tool instead of shinkai_tool so it also includes the code
                self.add_shinkai_tool(&profile, &tool, embedding)?;
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

    /// Searches for workflows using both embeddings and text similarity by name matching.
    pub async fn workflow_search(
        &mut self,
        profile: ShinkaiName,
        embedding_generator: Box<dyn EmbeddingGenerator>,
        db: Arc<ShinkaiDB>,
        query: Embedding,
        name_query: &str,
        num_of_results: u64,
    ) -> Result<Vec<ShinkaiTool>, ToolError> {
        if !self.started {
            let _ = self
                .start(embedding_generator, Arc::downgrade(&db), profile.clone())
                .await;
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;
        let routing_resource = self
            .routing_resources
            .get(&profile.to_string())
            .ok_or_else(|| ToolError::InvalidProfile("Profile not found".to_string()))?;

        // Perform vector search
        let vector_nodes = routing_resource.vector_search(query, num_of_results);

        // Perform name similarity search
        let mut name_similarity_results = vec![];
        for node in routing_resource.get_root_nodes() {
            if let Ok(shinkai_tool) = ShinkaiTool::from_json(node.get_text_content()?) {
                if let ShinkaiTool::Workflow(_) = shinkai_tool {
                    let name = shinkai_tool.name().to_lowercase();
                    let query = name_query.to_lowercase();
                    if name.contains(&query) {
                        let similarity_score = (query.len() as f64 / name.len() as f64) as f32;
                        name_similarity_results.push((shinkai_tool, similarity_score));
                    }
                }
            }
        }

        // Combine results from vector search and name similarity search, avoiding duplicates
        let mut combined_results = vec![];
        let mut seen_keys = std::collections::HashSet::new();

        for node in vector_nodes {
            if let Ok(shinkai_tool) = ShinkaiTool::from_json(node.node.get_text_content()?) {
                if let ShinkaiTool::Workflow(_) = shinkai_tool {
                    let key = shinkai_tool.tool_router_key();
                    if seen_keys.insert(key.clone()) {
                        combined_results.push((shinkai_tool, node.score));
                    }
                }
            }
        }

        for (shinkai_tool, similarity_score) in name_similarity_results {
            let key = shinkai_tool.tool_router_key();
            if seen_keys.insert(key.clone()) {
                combined_results.push((shinkai_tool, similarity_score));
            }
        }

        // Sort by combined score (vector score + name similarity score)
        combined_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return the top results
        Ok(combined_results
            .into_iter()
            .map(|(tool, _)| tool)
            .take(num_of_results as usize)
            .collect())
    }

    /// Returns a list of default ShinkaiTools that should always be included.
    pub fn get_default_tools(&self, profile: &ShinkaiName) -> Result<Vec<ShinkaiTool>, ToolError> {
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

        let mut default_tools = Vec::new();

        // Always include shinkai__math_expression_evaluator if it exists
        let math_tool_key = ShinkaiTool::gen_router_key(
            "shinkai__math_expression_evaluator".to_string(),
            "shinkai-tool-math-exp".to_string(),
        );
        if let Ok(node) = routing_resource.get_root_node(math_tool_key) {
            if let Ok(tool) = ShinkaiTool::from_json(node.get_text_content()?) {
                default_tools.push(tool);
            }
        }

        // Add more default tools here if needed in the future

        Ok(default_tools)
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

    /// Returns all available JS tools for a given user profile
    pub fn all_available_js_tools(
        &self,
        profile: &ShinkaiName,
        db: Arc<ShinkaiDB>,
    ) -> Result<Vec<ShinkaiTool>, ToolError> {
        if !self.started {
            return Err(ToolError::NotStarted);
        }

        let profile = profile
            .extract_profile()
            .map_err(|e| ToolError::InvalidProfile(e.to_string()))?;

        match db.all_tools_for_user(&profile) {
            Ok(tools) => {
                let js_tools: Vec<ShinkaiTool> = tools
                    .into_iter()
                    .filter_map(|tool| match tool {
                        ShinkaiTool::JS(_) | ShinkaiTool::JSLite(_) => Some(tool),
                        _ => None,
                    })
                    .collect();
                Ok(js_tools)
            }
            Err(e) => Err(ToolError::DatabaseError(e.to_string())),
        }
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
        db: Arc<ShinkaiDB>,
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
                // Fetch the full ShinkaiTool::JS (so it has the code that needs to be executed)
                let tool_key =
                    ShinkaiTool::gen_router_key(js_lite_tool.name.clone(), js_lite_tool.toolkit_name.clone());
                let full_js_tool = db.get_shinkai_tool(&tool_key, user_profile).map_err(|e| {
                    LLMProviderError::FunctionExecutionError(format!("Failed to fetch tool from DB: {}", e))
                })?;

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
            ShinkaiTool::Workflow(workflow_tool) => {
                // Available functions for the workflow
                let functions: HashMap<String, Box<dyn AsyncFunction>> = HashMap::new();

                // Call the inference chain router to choose which chain to use, and call it
                let mut dsl_inference =
                    DslChain::new(Box::new(context.clone_box()), workflow_tool.workflow.clone(), functions);

                // Add the inference function to the functions map
                dsl_inference.add_inference_function();
                dsl_inference.add_inference_no_ws_function();
                dsl_inference.add_opinionated_inference_function();
                dsl_inference.add_opinionated_inference_no_ws_function();
                dsl_inference.add_multi_inference_function();
                dsl_inference.add_all_generic_functions();
                dsl_inference.add_tools_from_router().await?;

                // TODO: we may need to inject other workflows as well?

                let inference_result = dsl_inference.run_chain().await?;

                return Ok(FunctionCallResponse {
                    response: inference_result.response,
                    function_call,
                });
            }
        }

        Err(LLMProviderError::FunctionNotFound(function_name))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::tools::workflows_data;

    use super::*;
    use std::time::Instant;

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
}
