use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use crate::tools::js_tools::JSTool;
use crate::tools::rust_tools::RustTool;
use serde_json;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::source::VRSourceReference;
use shinkai_vector_resources::vector_resource::{
    MapVectorResource, NodeContent, RetrievedNode, VectorResourceCore, VectorResourceSearch,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ShinkaiTool {
    Rust(RustTool),
    JS(JSTool),
}

impl ShinkaiTool {
    /// The key that this tool will be stored under in the tool router
    pub fn tool_router_key(&self) -> String {
        let (name, toolkit_name) = (
            self.name(),
            match self {
                ShinkaiTool::Rust(r) => r.toolkit_type_name(),
                ShinkaiTool::JS(j) => j.toolkit_name.to_string(),
            },
        );

        Self::gen_router_key(name, toolkit_name)
    }

    /// Tool name
    pub fn name(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.name.clone(),
            ShinkaiTool::JS(j) => j.name.clone(),
        }
    }
    /// Tool description
    pub fn description(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.description.clone(),
            ShinkaiTool::JS(j) => j.description.clone(),
        }
    }

    /// Toolkit name the tool is from
    pub fn toolkit_name(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.name.clone(),
            ShinkaiTool::JS(j) => j.name.clone(),
        }
    }

    /// Toolkit name the tool is from
    pub fn toolkit_type_name(&self) -> String {
        match self {
            ShinkaiTool::Rust(r) => r.toolkit_type_name().clone(),
            ShinkaiTool::JS(j) => j.toolkit_name.clone(),
        }
    }

    /// Returns the input arguments of the tool
    pub fn input_args(&self) -> Vec<ToolArgument> {
        match self {
            ShinkaiTool::Rust(r) => r.input_args.clone(),
            ShinkaiTool::JS(j) => j.input_args.clone(),
        }
    }

    /// Returns a formatted summary of the tool
    pub fn formatted_tool_summary(&self) -> String {
        format!(
            "Tool Name: {}\nToolkit Name: {}\nDescription: {}",
            self.name(),
            self.toolkit_type_name(),
            self.description(),
        )
    }

    pub fn json_value_tool_summary(&self) -> Result<serde_json::Value, ToolError> {
        let mut properties = serde_json::Map::new();

        for arg in self.input_args() {
            properties.insert(
                arg.name.clone(),
                serde_json::json!({
                    "type": "string",
                    "description": arg.description.clone(),
                }),
            );
        }

        let summary = serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": self.input_args().iter().map(|arg| arg.name.clone()).collect::<Vec<String>>(),
                },
            },
        });

        Ok(summary)
    }

    pub fn json_formatted_tool_summary(&self) -> Result<String, ToolError> {
        let summary_value = self.json_value_tool_summary()?;
        serde_json::to_string(&summary_value).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Formats the tool's info into a String to be used for generating the tool's embedding.
    pub fn format_embedding_string(&self) -> String {
        let mut embedding_string = format!("{}:{}\n", self.name(), self.description());

        embedding_string.push_str("Input Args:\n");

        for arg in self.input_args() {
            embedding_string.push_str(&format!("-{}:{}\n", arg.name, arg.description));
        }

        embedding_string
    }

    /// Generate the key that this tool will be stored under in the tool router
    pub fn gen_router_key(name: String, toolkit_name: String) -> String {
        // We replace any `/` in order to not have the names break VRPaths
        format!("{}:::{}", toolkit_name, name).replace('/', "|")
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json).map_err(|e| ToolError::ParseError(e.to_string()))?;
        Ok(deserialized)
    }
}

impl From<RustTool> for ShinkaiTool {
    fn from(tool: RustTool) -> Self {
        ShinkaiTool::Rust(tool)
    }
}

impl From<JSTool> for ShinkaiTool {
    fn from(tool: JSTool) -> Self {
        ShinkaiTool::JS(tool)
    }
}

/// A top level struct which indexes JSTools installed in the Shinkai Node
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRouter {
    pub routing_resource: MapVectorResource,
}

impl ToolRouter {
    /// Create a new ToolRouter instance from scratch.
    pub async fn new(generator: Box<dyn EmbeddingGenerator>) -> Self {
        let name = "Tool Router";
        let desc = Some("Enables performing vector searches to find relevant tools.");
        let source = VRSourceReference::None;

        // Initialize the MapVectorResource and add all of the rust tools by default
        let mut routing_resource = MapVectorResource::new_empty(name, desc, source, true);

        // Generate the static Rust tools
        let rust_tools = RustTool::static_tools(generator).await;

        // Insert each Rust tool into the routing resource
        for tool in rust_tools {
            let shinkai_tool = ShinkaiTool::Rust(tool.clone());
            let _ = routing_resource.insert_text_node(
                shinkai_tool.tool_router_key(),
                shinkai_tool.to_json().unwrap(), // This unwrap should be safe because Rust Tools are not dynamic
                None,
                tool.tool_embedding.clone(),
                &vec![],
            );
        }

        ToolRouter { routing_resource }
    }

    fn tool_type_metadata_key() -> String {
        "tool_type".to_string()
    }

    fn tool_type_rust_value() -> String {
        "rust".to_string()
    }

    fn tool_type_js_value() -> String {
        "js".to_string()
    }

    /// Fetches the ShinkaiTool from the ToolRouter by parsing the internal Node
    /// within the ToolRouter.
    pub fn get_shinkai_tool(&self, tool_name: &str, toolkit_name: &str) -> Result<ShinkaiTool, ToolError> {
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        let node = self.routing_resource.get_root_node(key)?;
        ShinkaiTool::from_json(node.get_text_content()?)
    }

    /// A hard-coded DB key for the profile-wide Tool Router in Topic::Tools.
    /// No other resource is allowed to use this shinkai_db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn profile_router_shinkai_db_key() -> String {
        "profile_tool_router".to_string()
    }

    /// Returns a list of ShinkaiTools of the most similar that
    /// have matching data tag names.
    pub fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<ShinkaiTool> {
        let nodes = self
            .routing_resource
            .syntactic_vector_search(query, num_of_results, data_tag_names);
        self.ret_nodes_to_tools(&nodes)
    }

    /// Returns a list of ShinkaiTools of the most similar.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<ShinkaiTool> {
        let nodes = self.routing_resource.vector_search(query, num_of_results);
        self.ret_nodes_to_tools(&nodes)
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

    /// Adds a tool into the ToolRouter instance.
    pub fn add_shinkai_tool(&mut self, shinkai_tool: &ShinkaiTool, embedding: Embedding) -> Result<(), ToolError> {
        let data = shinkai_tool.to_json()?;
        let router_key = shinkai_tool.tool_router_key();
        let metadata = None;

        // Setup the metadata based on tool type

        match self.routing_resource.get_root_node(router_key.clone()) {
            Ok(_) => {
                // If a Shinkai tool with same key is already found, error
                return Err(ToolError::ToolAlreadyInstalled(data.to_string()));
            }
            Err(_) => {
                // If no tool is found, insert new tool
                self.routing_resource._insert_kv_without_tag_validation(
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
    pub fn delete_shinkai_tool(&mut self, tool_name: &str, toolkit_name: &str) -> Result<(), ToolError> {
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        self.routing_resource.print_all_nodes_exhaustive(None, false, false);
        println!("Tool key: {}", key);
        self.routing_resource.remove_node_dt_specified(key, None, true)?;
        Ok(())
    }

    /// Acquire the tool embedding for a given ShinkaiTool.
    pub fn get_tool_embedding(&self, shinkai_tool: &ShinkaiTool) -> Result<Embedding, ToolError> {
        Ok(self
            .routing_resource
            .get_root_embedding(shinkai_tool.tool_router_key().to_string())?)
    }

    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        Ok(ToolRouter {
            routing_resource: MapVectorResource::from_json(json)?,
        })
    }
    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }
}
