use crate::resources::document::DocumentResource;
use crate::resources::embeddings::*;
use crate::resources::resource::*;
use crate::resources::resource_errors::*;
use serde_json;
use std::convert::From;
use std::convert::TryFrom;
use std::str::FromStr;

/// Type which holds reference data about a resource in the DB.
///
/// This hides away the implementation details of the current underlying DocumentResource
/// and allows us to offer an equivalent interface in the future even if we swap to
/// a different underlying internal model of how the resource pointer data is stored.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourcePointer {
    pub id: String,     // Id of the ResourcePointer in the ResourceRouter (currently DataChunk id)
    pub db_key: String, // Key of the resource in the Topic::Resources in the db
    pub resource_type: ResourceType,
    resource_embedding: Option<Embedding>,
}

impl ResourcePointer {
    /// Create a new ResourcePointer
    pub fn new(id: &str, db_key: &str, resource_type: ResourceType, resource_embedding: Option<Embedding>) -> Self {
        Self {
            id: id.to_string(),
            db_key: db_key.to_string(),
            resource_type,
            resource_embedding: resource_embedding.clone(),
        }
    }
}

impl From<&Box<dyn Resource>> for ResourcePointer {
    fn from(resource: &Box<dyn Resource>) -> Self {
        let db_key = resource.db_key();
        let resource_type = resource.resource_type();
        let id = "1"; // This will be replaced when the ResourcePointer is added into a ResourceRouter instance
        let embedding = resource.resource_embedding().clone();
        ResourcePointer::new(id, &db_key, resource_type, Some(embedding))
    }
}

impl TryFrom<RetrievedDataChunk> for ResourcePointer {
    type Error = ResourceError;

    fn try_from(ret_data: RetrievedDataChunk) -> Result<Self, Self::Error> {
        let resource_type =
            ResourceType::from_str(&ret_data.chunk.data).map_err(|_| ResourceError::InvalidResourceType)?;
        let db_key = ret_data.chunk.metadata.unwrap_or_default();
        let id = ret_data.chunk.id;

        Ok(ResourcePointer::new(&id, &db_key, resource_type, None))
    }
}

/// A top level struct which indexes a series of resource pointers.
/// This struct thus makes it possible to perform vector searches to find
/// relevant Resources for users or agents.
///
/// For now we just implement this on top of DocumentResource for speed of
/// implementation, later on we can come around and design something
/// specifically for routing that is more effective if needed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceRouter {
    routing_resource: DocumentResource,
}

impl ResourceRouter {
    /// Create a new ResourceRouter instance from scratch.
    pub fn new() -> Self {
        let name = "Resource Router";
        let desc = Some("Enables performing vector searches to find relevant resources.");
        let source = None;
        let resource_id = "resource_router";
        ResourceRouter {
            routing_resource: DocumentResource::new_empty(name, desc, source, resource_id),
        }
    }

    /// A hard-coded DB key for the global resource router in Topic::Resources.
    /// No other resource is allowed to use this db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn global_router_db_key() -> String {
        "global_resource_router".to_string()
    }

    /// Performs a vector similarity search using a query embedding and returns
    /// a list of ResourcePointers of the most similar Resources.
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

    /// Adds a resource pointer into the ResourceRouter instance.
    /// The pointer is expected to have a valid resource embedding
    /// and the matching resource having already been saved into the DB.
    ///
    /// If a resource pointer already exists with the same db_key, then
    /// the old pointer will be replaced.
    ///
    /// Of note, in this implementation we store the resource type in the `data`
    /// of the chunk and the db_key in the `metadata` of the chunk.
    pub fn add_resource_pointer(&mut self, resource_pointer: &ResourcePointer) -> Result<(), ResourceError> {
        let data = resource_pointer.resource_type.to_str();
        let embedding = resource_pointer
            .resource_embedding
            .clone()
            .ok_or(ResourceError::NoEmbeddingProvided)?;
        let metadata = resource_pointer.db_key.clone();

        match self.db_key_search(&metadata) {
            Ok(old_pointer) => {
                // If a resource pointer with matching db_key is found,
                // replace the existing resource pointer with the new one.
                self.replace_resource_pointer(&old_pointer.id, resource_pointer)?;
            }
            Err(_) => {
                // If no resource pointer with matching db_key is found,
                // append the new data.
                self.routing_resource.append_data(&data, Some(&metadata), &embedding);
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
    pub fn replace_resource_pointer(
        &mut self,
        old_pointer_id: &str,
        resource_pointer: &ResourcePointer,
    ) -> Result<(), ResourceError> {
        let data = resource_pointer.resource_type.to_str();
        let embedding = resource_pointer
            .resource_embedding
            .clone()
            .ok_or(ResourceError::NoEmbeddingProvided)?;
        let metadata = resource_pointer.db_key.clone();
        let old_pointer_id = old_pointer_id
            .parse::<u64>()
            .map_err(|_| ResourceError::InvalidChunkId)?;

        self.routing_resource
            .replace_data(old_pointer_id, &data, Some(&metadata), &embedding)?;
        Ok(())
    }

    /// Deletes the resource pointer inside of the ResourceRouter given a valid id
    pub fn delete_resource_pointer(&mut self, old_pointer_id: String) -> Result<(), ResourceError> {
        let id: u64 = old_pointer_id.parse().map_err(|_| ResourceError::InvalidChunkId)?;
        self.routing_resource.delete_data(id)?;
        Ok(())
    }

    /// Acquire the resource_embedding for a given ResourcePointer.
    /// If the pointer itself doesn't have the embedding attached to it,
    /// we use the id to fetch the embedding directly from the ResourceRouter.
    pub fn get_resource_embedding(&self, resource_pointer: &ResourcePointer) -> Result<Embedding, ResourceError> {
        if let Some(embedding) = resource_pointer.resource_embedding.clone() {
            Ok(embedding)
        } else {
            let id: usize = resource_pointer.id.parse().map_err(|_| ResourceError::InvalidChunkId)?;
            match self.routing_resource.chunk_embeddings().get(id - 1) {
                Some(embedding) => Ok(embedding.clone()),
                None => Err(ResourceError::InvalidChunkId),
            }
        }
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
