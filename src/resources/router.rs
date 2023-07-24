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
    /// Performs a vector similarity search using a query embedding and returns
    /// a list of the db_keys (as Strings) of the most similar Resources.
    ///
    /// # Arguments
    ///
    /// * `query` - An embedding that is the basis for the similarity search.
    /// * `num_of_results` - The number of top results to return (top-k)
    ///
    /// # Returns
    ///
    /// A `Result` that contains a vector of db_keys sorted by similarity
    /// score in descending order, or an error if something goes wrong.
    fn similarity_search(&self, query: Embedding, num_of_results: u64) -> Vec<String> {
        let chunks = self.routing_resource.similarity_search(query, num_of_results);
        chunks.iter().map(|c| c.id.to_string()).collect()
    }

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
    /// The pointed-to resource is expected to have a valid resource embedding
    /// and have already been saved into the DB.
    ///
    /// If a resource pointer already exists with the same db_key, then
    /// the old pointer will be replaced.
    pub fn add_resource_pointer(&mut self, resource: &Box<dyn Resource>) -> Result<(), ResourceError> {
        let data = resource.name();
        let embedding = resource.resource_embedding();
        let metadata = resource.db_key().clone();

        match self.db_key_search(&metadata) {
            Ok(id) => {
                // If a resource pointer with matching db_key is found,
                // replace the existing resource pointer with the new one.
                self.replace_resource_pointer(&id, resource)?;
            }
            Err(_) => {
                // If no resource pointer with matching db_key is found,
                // append the new data.
                self.routing_resource.append_data(&data, Some(&metadata), embedding);
            }
        }

        Ok(())
    }

    /// Search through the resource pointers to find if one exists with
    /// a matching db_key.
    ///
    /// Returns the id of the first resource pointer in the Router.
    pub fn db_key_search(&self, query_metadata: &str) -> Result<String, ResourceError> {
        let data_chunks = self.routing_resource.metadata_search(query_metadata)?;

        if let Some(chunk) = data_chunks.get(0).cloned() {
            return Ok(chunk.id);
        } else {
            Err(ResourceError::NoChunkFound)
        }
    }

    /// Replaces an existing resource pointer with a new one.
    pub fn replace_resource_pointer(&mut self, id: &str, resource: &Box<dyn Resource>) -> Result<(), ResourceError> {
        let data = resource.name();
        let embedding = resource.resource_embedding();
        let metadata = resource.db_key().clone();
        let id = id.parse::<u64>().map_err(|_| ResourceError::InvalidChunkId)?;
        self.routing_resource
            .replace_data(id, &data, Some(&metadata), embedding)?;
        Ok(())
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
