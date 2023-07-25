use crate::resources::document::DocumentResource;
use crate::resources::embeddings::*;
use crate::resources::resource::*;
use crate::resources::resource_errors::*;
use serde_json;
use std::convert::TryFrom;
use std::str::FromStr;

/// Type which holds the data about how to fetch and parse a resource from the DB.
/// This hides away the implementation details of the current underlying DocumentResource
/// and allows us to offer an equivalent interface in the future even if we swap to
/// a different underlying internal model of how the resource pointer data is stored.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourcePointer {
    pub db_key: String,              // Key of the resource in the Topic::Resources
    pub resource_type: ResourceType, // Type of the resource
    pub id: String,                  // Id of the DataChunk in the Router
}

impl ResourcePointer {
    pub fn new(db_key: &str, resource_type: ResourceType, id: &str) -> Self {
        Self {
            db_key: db_key.to_string(),
            resource_type,
            id: id.to_string(),
        }
    }
}

impl TryFrom<RetrievedDataChunk> for ResourcePointer {
    type Error = ResourceError;

    fn try_from(ret_data: RetrievedDataChunk) -> Result<Self, Self::Error> {
        let resource_type =
            ResourceType::from_str(&ret_data.chunk.data).map_err(|_| ResourceError::InvalidResourceType)?;
        let db_key = ret_data.chunk.metadata.unwrap_or_default();
        let id = ret_data.chunk.id;

        Ok(ResourcePointer::new(&db_key, resource_type, &id))
    }
}

/// A top level struct which indexes all resources inside of a Shinkai node as
/// resource pointers. These are DataChunks which have a matching embedding that
/// is the Resource Embedding, and which hold metadata that points to the DB key
/// of the Resource.
///
/// This struct thus makes it possible to perform vector searches to find
/// relevant Resources for any vector search made by users or agents.
///
/// For now we just implement this on top of DocumentResource for speed of
/// implementation, later on we can come around and design something
/// specifically for routing that is more effective if needed.
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
    pub fn similarity_search(&self, query: Embedding, num_of_results: u64) -> Vec<ResourcePointer> {
        let chunks = self.routing_resource.similarity_search(query, num_of_results);
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// Takes a list of RetrievedDataChunks and outputs a list of ResourcePointers
    ///
    /// Of note, if a chunk holds an invalid ResourceType string then the chunk
    /// is ignored.
    fn ret_data_chunks_to_pointers(&self, chunks: &Vec<RetrievedDataChunk>) -> Vec<ResourcePointer> {
        let mut resource_pointers = vec![];
        for chunk in chunks {
            // Ignore resources added to the router with invalid resource types
            if let Ok(resource_pointer) = ResourcePointer::try_from(chunk.clone()) {
                resource_pointers.push(resource_pointer);
            }
        }
        resource_pointers
    }

    /// Adds a resource pointer to the ResourceRouter in memory.
    /// The pointed-to resource is expected to have a valid resource embedding
    /// and have already been saved into the DB.
    ///
    /// If a resource pointer already exists with the same db_key, then
    /// the old pointer will be replaced.
    ///
    /// Of note, in this implementation we store the resource type in the `data`
    /// of the chunk and the db_key in the `metadata` of the chunk.
    pub fn add_resource_pointer(&mut self, resource: &Box<dyn Resource>) -> Result<(), ResourceError> {
        let data = resource.resource_type();
        let embedding = resource.resource_embedding();
        let metadata = resource.db_key().clone();

        match self.db_key_search(&metadata) {
            Ok(res_pointer) => {
                // If a resource pointer with matching db_key is found,
                // replace the existing resource pointer with the new one.
                self.replace_resource_pointer(&res_pointer.id, resource)?;
            }
            Err(_) => {
                // If no resource pointer with matching db_key is found,
                // append the new data.
                self.routing_resource
                    .append_data(&data.to_str(), Some(&metadata), embedding);
            }
        }

        Ok(())
    }

    /// Search through the resource pointers to find if one exists with
    /// a matching db_key.
    pub fn db_key_search(&self, db_key: &str) -> Result<ResourcePointer, ResourceError> {
        let ret_data = self.routing_resource.metadata_search(db_key)?;

        if let Some(res_pointer) = self.ret_data_chunks_to_pointers(&ret_data).get(0).cloned() {
            return Ok(res_pointer);
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
    pub fn delete_resource_pointer(&mut self, id: u64) -> Result<(), ResourceError> {
        self.routing_resource.delete_data(id)?;
        Ok(())
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
