use crate::resources::document::DocumentResource;
use crate::resources::embeddings::*;
use crate::resources::resource::*;
use crate::resources::resource_errors::*;
use serde_json;

/// A top level struct which indexes all resources inside of a Shinkai node as
/// resource pointers. These are DataChunks which have a matching embedding that
/// is the Resource Embedding, and which hold metadata that points to the DB key
/// of the Resource.
///
/// This struct thus makes it possible to perform vector searches to find
/// relevant Resources for any vector search made by users or agents.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceRouter {
    routing_resource: DocumentResource,
}

impl ResourceRouter {
    /// Create a new ResourceRouter from scratch
    pub fn new() -> Self {
        let name = "Resource Router";
        let desc = Some("Enables performing vector searches to find relevant resources.");
        let source = None;
        let resource_id = "resource_router";
        ResourceRouter {
            routing_resource: DocumentResource::new_empty(name, desc, source, resource_id),
        }
    }

    /// A hard-coded DB key for the resource router in Topic::Resources
    /// No other resource is allowed to use this db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn db_key() -> String {
        "resource_router".to_string()
    }

    /// Adds a resource pointer to the ResourceRouter in memory.
    /// The pointed to resource is expected to have a valid resource embedding
    /// and have already been saved into the DB.
    pub fn add_resource_pointer(&mut self, resource: Box<dyn Resource>) {
        let data = resource.name();
        let embedding = resource.resource_embedding();
        let metadata = resource.db_key().clone();
        self.routing_resource.append_data(&data, Some(&metadata), embedding);
    }

    /// Search through all metadata of all resource pointers (stored as
    /// DataChunks)
    pub fn metadata_search(&self, query_metadata: &str) -> Result<Vec<DataChunk>, ResourceError> {
        self.routing_resource.metadata_search(query_metadata)
    }

    /// Replaces an existing resource pointer with a new one.
    ///
    /// Returns the old resource pointer as a DataChunk
    pub fn replace_resource_pointer(
        &mut self,
        id: u64,
        resource: Box<dyn Resource>,
    ) -> Result<DataChunk, ResourceError> {
        let data = resource.name();
        let embedding = resource.resource_embedding();
        let metadata = resource.db_key().clone();
        self.routing_resource.replace_data(id, data, Some(&metadata), embedding)
    }

    /// Deletes a resource pointer given the DataChunk id
    pub fn delete_resource_pointer(&mut self, id: u64) -> Result<(DataChunk, Embedding), ResourceError> {
        self.routing_resource.delete_data(id)
    }

    pub fn from_json(json: &str) -> Result<Self, ResourceError> {
        Ok(ResourceRouter {
            routing_resource: DocumentResource::from_json(json)?,
        })
    }
    /// Convert to json
    pub fn to_json(&self) -> Result<String, ResourceError> {
        serde_json::to_string(self).map_err(|_| ResourceError::FailedJSONParsing)
    }
}
