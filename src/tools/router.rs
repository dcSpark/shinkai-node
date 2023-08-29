use crate::resources::embeddings::*;
use crate::resources::map_resource::MapVectorResource;
use crate::resources::vector_resource::*;
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
            ShinkaiTool::JS(j) => (j.name.clone(), j.toolkit_name),
        };

        Self::gen_router_key(name, toolkit_name)
    }

    /// Generate the key that this tool will be stored under in the tool router
    pub fn gen_router_key(name: String, toolkit_name: String) -> String {
        // We include `tool_type` to prevent attackers trying to overwrite
        // the internal Rust tools with JS tools that have the same name
        format!("{}:{}", toolkit_name, name)
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
                Some(metadata),
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
    /// within the ToolRouter. By default Rust tools just have their name stored in the metadata,
    /// while JS tools are fully serialized into JSON and stored in the DataChunk.
    pub fn get_shinkai_tool(&self, tool_name: &str, toolkit_name: &str) -> Result<ShinkaiTool, ToolError> {
        let key = ShinkaiTool::gen_router_key(tool_name.to_string(), toolkit_name.to_string());
        let data_chunk = self.routing_resource.get_data_chunk(key)?;

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
        Err(ToolError::ToolNotFound(tool_name.to_string()))
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
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// Returns a list of ShinkaiTools of the most similar.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<ShinkaiTool> {
        let chunks = self.routing_resource.vector_search(query, num_of_results);
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// Takes a list of RetrievedDataChunks and outputs a list of ShinkaiTools
    /// that point to the real resource (not the resource router).
    ///
    /// Of note, if a chunk holds an invalid ToolType string then the chunk
    /// is ignored.
    fn ret_data_chunks_to_pointers(&self, ret_chunks: &Vec<RetrievedDataChunk>) -> Vec<ShinkaiTool> {
        let mut resource_pointers = vec![];
        // for ret_chunk in ret_chunks {
        //     // Ignore resources added to the router with invalid resource types
        //     if let Ok(resource_type) = ToolType::from_str(&ret_chunk.chunk.data).map_err(|_| ToolError::InvalidToolType)
        //     {
        //         let id = &ret_chunk.chunk.id;
        //         let embedding = self.routing_resource.get_chunk_embedding(id).ok();
        //         let resource_pointer =
        //             ShinkaiTool::new(&id, resource_type, embedding, ret_chunk.chunk.data_tag_names.clone());
        //         resource_pointers.push(resource_pointer);
        //     }
        // }
        resource_pointers
    }

    /// Adds a resource pointer into the ToolRouter instance.
    /// The pointer is expected to have a valid resource embedding
    /// and the matching resource having already been saved into the DB.
    ///
    /// If a resource pointer already exists with the same db_key, then
    /// the old pointer will be replaced.
    ///
    /// Of note, in this implementation we store the resource type in the `data`
    /// of the chunk and the db_key as the id of the data chunk.
    pub fn add_resource_pointer(&mut self, resource_pointer: &ShinkaiTool) -> Result<(), ToolError> {
        let data = resource_pointer.resource_type.to_str();
        let embedding = resource_pointer
            .resource_embedding
            .clone()
            .ok_or(ToolError::NoEmbeddingProvided)?;
        let db_key = resource_pointer.db_key.to_string();
        let db_key_clone = db_key.clone();
        let metadata = None;

        match self.routing_resource.get_data_chunk(db_key_clone) {
            Ok(old_chunk) => {
                // If a resource pointer with matching db_key is found,
                // replace the existing resource pointer with the new one.
                self.replace_resource_pointer(&old_chunk.id, resource_pointer)?;
            }
            Err(_) => {
                // If no resource pointer with matching db_key is found,
                // insert the new kv pair. We skip tag validation because the tags
                // have already been previously validated when adding into the
                // original resource.
                self.routing_resource._insert_kv_without_tag_validation(
                    &db_key,
                    &data,
                    metadata,
                    &embedding,
                    &resource_pointer.data_tag_names,
                );
            }
        }

        Ok(())
    }

    /// Replaces an existing resource pointer with a new one
    pub fn replace_resource_pointer(
        &mut self,
        old_pointer_id: &str,
        resource_pointer: &ShinkaiTool,
    ) -> Result<(), ToolError> {
        let data = resource_pointer.resource_type.to_str();
        let embedding = resource_pointer
            .resource_embedding
            .clone()
            .ok_or(ToolError::NoEmbeddingProvided)?;
        let metadata = None;

        self.routing_resource._replace_kv_without_tag_validation(
            old_pointer_id,
            &data,
            metadata,
            &embedding,
            &resource_pointer.data_tag_names,
        )?;
        Ok(())
    }

    /// Deletes the resource pointer inside of the ToolRouter given a valid id
    pub fn delete_resource_pointer(&mut self, old_pointer_id: &str) -> Result<(), ToolError> {
        self.routing_resource.delete_kv(old_pointer_id)?;
        Ok(())
    }

    /// Acquire the resource_embedding for a given ShinkaiTool.
    /// If the pointer itself doesn't have the embedding attached to it,
    /// we use the id to fetch the embedding directly from the ToolRouter.
    pub fn get_resource_embedding(&self, resource_pointer: &ShinkaiTool) -> Result<Embedding, ToolError> {
        if let Some(embedding) = resource_pointer.resource_embedding.clone() {
            Ok(embedding)
        } else {
            self.routing_resource.get_chunk_embedding(&resource_pointer.db_key)
        }
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
