use crate::resources::map_resource::MapVectorResource;
use crate::resources::vector_resource::*;
use crate::resources::{embeddings::*, router};
use crate::tools::error::ToolError;
use crate::tools::js_tools::JSTool;
use crate::tools::rust_tools::{RustTool, RUST_TOOLKIT};
use serde_json;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ShinkaiTool {
    Rust(RustTool),
    JS(JSTool),
}

impl ShinkaiTool {
    /// The key that this tool will be stored under in the tool router
    pub fn tool_router_key(&self) -> String {
        let (name, toolkit_name) = match self {
            ShinkaiTool::Rust(r) => (r.name.clone(), r.toolkit_name()),
            ShinkaiTool::JS(j) => (j.name.clone(), j.toolkit_name.to_string()),
        };

        Self::gen_router_key(name, toolkit_name)
    }

    /// Generate the key that this tool will be stored under in the tool router
    pub fn gen_router_key(name: String, toolkit_name: String) -> String {
        // We include `tool_type` to prevent attackers trying to overwrite
        // the internal Rust tools with JS tools that have the same name
        format!("{}:{}", toolkit_name, name)
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

/// A top level struct which indexes JSTools installed in the Shinkai Node
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolRouter {
    routing_resource: MapVectorResource,
}

impl ToolRouter {
    /// Create a new ToolRouter instance from scratch.
    pub fn new() -> Self {
        let name = "Tool Router";
        let desc = Some("Enables performing vector searches to find relevant tools.");
        let source = None;
        let resource_id = "tool_router";

        // Initialize the MapVectorResource and add all of the rust tools by default
        let mut routing_resource = MapVectorResource::new_empty(name, desc, source, resource_id);
        let mut metadata = HashMap::new();
        metadata.insert(Self::tool_type_metadata_key(), Self::tool_type_rust_value());
        RUST_TOOLKIT.rust_tool_map.values().into_iter().map(|t| {
            routing_resource.insert_kv(
                &ShinkaiTool::Rust(t.clone()).tool_router_key(),
                &t.to_json().unwrap(), // This unwrap should be safe because Rust Tools are not dynamic
                Some(metadata.clone()),
                &t.tool_embedding,
                &vec![],
            );
        });

        ToolRouter {
            routing_resource: routing_resource,
        }
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

    /// Fetches the ShinkaiTool from the ToolRouter by parsing the internal DataChunk
    /// within the ToolRouter.
    pub fn get_shinkai_tool(&self, tool_name: &str, toolkit_name: &str) -> Result<ShinkaiTool, ToolError> {
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        let data_chunk = self.routing_resource.get_data_chunk(key)?;
        self.parse_shinkai_tool_from_data_chunk(data_chunk)
    }

    /// Parses a fetched internal DataChunk from within the ToolRouter into a ShinkaiTool
    fn parse_shinkai_tool_from_data_chunk(&self, data_chunk: DataChunk) -> Result<ShinkaiTool, ToolError> {
        if let Some(metadata) = data_chunk.metadata {
            if let Some(tool_type) = metadata.get(&Self::tool_type_metadata_key()) {
                // If a rust tool, read the name and fetch the rust tool from that global static rust toolkit
                if tool_type.to_string() == Self::tool_type_rust_value() {
                    let tool = RustTool::from_json(&data_chunk.data)?;
                    return Ok(ShinkaiTool::Rust(tool));
                }
                // Else a JSTool
                else {
                    let tool = JSTool::from_json(&data_chunk.data)?;
                    return Ok(ShinkaiTool::JS(tool));
                }
            }
        }
        Err(ToolError::ToolNotFound(data_chunk.id))
    }

    /// A hard-coded DB key for the profile-wide Tool Router in Topic::Tools.
    /// No other resource is allowed to use this db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn profile_router_db_key() -> String {
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
        let chunks = self
            .routing_resource
            .syntactic_vector_search(query, num_of_results, data_tag_names);
        self.ret_data_chunks_to_tools(&chunks)
    }

    /// Returns a list of ShinkaiTools of the most similar.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<ShinkaiTool> {
        let chunks = self.routing_resource.vector_search(query, num_of_results);
        self.ret_data_chunks_to_tools(&chunks)
    }

    /// Takes a list of RetrievedDataChunks and outputs a list of ShinkaiTools
    fn ret_data_chunks_to_tools(&self, ret_chunks: &Vec<RetrievedDataChunk>) -> Vec<ShinkaiTool> {
        let mut shinkai_tools = vec![];
        for ret_chunk in ret_chunks {
            // Ignores tools added to the router which are invalid by matching on the Ok()
            if let Ok(shinkai_tool) = Self::parse_shinkai_tool_from_data_chunk(&self, ret_chunk.chunk.clone()) {
                shinkai_tools.push(shinkai_tool);
            }
        }
        shinkai_tools
    }

    /// Adds a tool into the ToolRouter instance.
    pub fn add_shinkai_tool(&mut self, shinkai_tool: &ShinkaiTool, embedding: Embedding) -> Result<(), ToolError> {
        let data = shinkai_tool.to_json()?;
        let metadata = None;
        let router_key = shinkai_tool.tool_router_key();

        match self.routing_resource.get_data_chunk(router_key.clone()) {
            Ok(_) => {
                // If a Shinkai tool with same key is already found, error
                return Err(ToolError::ToolAlreadyInstalled(data.to_string()));
            }
            Err(_) => {
                // If no tool is found, insert new tool
                self.routing_resource._insert_kv_without_tag_validation(
                    &router_key,
                    &data,
                    metadata,
                    &embedding,
                    &vec![],
                );
            }
        }

        Ok(())
    }

    /// Deletes the resource pointer inside of the ToolRouter given a valid id
    pub fn delete_shinkai_tool(&mut self, old_pointer_id: &str) -> Result<(), ToolError> {
        self.routing_resource.delete_kv(old_pointer_id)?;
        Ok(())
    }

    /// Acquire the resource_embedding for a given ShinkaiTool.
    /// If the pointer itself doesn't have the embedding attached to it,
    /// we use the id to fetch the embedding directly from the ToolRouter.
    pub fn get_tool_embedding(&self, shinkai_tool: &ShinkaiTool) -> Result<Embedding, ToolError> {
        Ok(self
            .routing_resource
            .get_chunk_embedding(&shinkai_tool.tool_router_key())?)
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
