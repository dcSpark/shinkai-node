use crate::resources::document::DocumentResource;
use crate::resources::embeddings::*;
use crate::resources::resource::*;
use crate::resources::resource_errors::*;
use serde_json;
use std::collections::HashMap;
use std::convert::From;
use std::convert::TryFrom;
use std::str::FromStr;

use super::data_tags::DataTag;

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
    data_tag_names: Vec<String>,
    resource_embedding: Option<Embedding>,
}

impl ResourcePointer {
    /// Create a new ResourcePointer
    pub fn new(
        id: &str,
        db_key: &str,
        resource_type: ResourceType,
        resource_embedding: Option<Embedding>,
        data_tag_names: Vec<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            db_key: db_key.to_string(),
            resource_type,
            resource_embedding: resource_embedding.clone(),
            data_tag_names: data_tag_names,
        }
    }

    /// Wraps the resource pointer's db_key into a hashmap ready to use for
    /// the resource router's chunk metadata
    pub fn _db_key_as_metadata_hashmap(&self) -> HashMap<String, String> {
        let mut hmap = HashMap::new();
        hmap.insert(ResourceRouter::router_chunk_metadata_key(), self.db_key.clone());
        hmap
    }
}

impl From<Box<dyn Resource>> for ResourcePointer {
    fn from(resource: Box<dyn Resource>) -> Self {
        resource.get_resource_pointer()
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

    /// Performs a syntactic vector search using a query embedding and list of data tag names.
    /// Returns a list of ResourcePointers of the most similar Resources.
    pub fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<ResourcePointer> {
        let chunks = self
            .routing_resource
            .syntactic_vector_search(query, num_of_results, data_tag_names);
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// Performs a vector search using a query embedding and returns
    /// a list of ResourcePointers of the most similar Resources.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<ResourcePointer> {
        let chunks = self.routing_resource.vector_search(query, num_of_results);
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// A hardcoded key string used for the metadata hashmap of data chunks
    /// in the router's internal resource
    fn router_chunk_metadata_key() -> String {
        "db_key".to_string()
    }

    /// Takes a list of RetrievedDataChunks and outputs a list of ResourcePointers
    /// that point to the real resource (not the resource router).
    ///
    /// Of note, if a chunk holds an invalid ResourceType string then the chunk
    /// is ignored.
    fn ret_data_chunks_to_pointers(&self, ret_chunks: &Vec<RetrievedDataChunk>) -> Vec<ResourcePointer> {
        let mut resource_pointers = vec![];
        for ret_chunk in ret_chunks {
            // Ignore resources added to the router with invalid resource types

            if let Ok(resource_type) =
                ResourceType::from_str(&ret_chunk.chunk.data).map_err(|_| ResourceError::InvalidResourceType)
            {
                let metadata = &ret_chunk.chunk.metadata.clone().unwrap_or_default();
                let db_key: String = metadata
                    .get(&ResourceRouter::router_chunk_metadata_key())
                    .cloned()
                    .unwrap_or_default();
                let id = &ret_chunk.chunk.id;
                let embedding = self.routing_resource.get_chunk_embedding(id).ok();
                let resource_pointer = ResourcePointer::new(
                    &id,
                    &db_key,
                    resource_type,
                    embedding,
                    ret_chunk.chunk.data_tag_names.clone(),
                );
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
        let db_key = resource_pointer.db_key.to_string();
        let metadata = Some(resource_pointer._db_key_as_metadata_hashmap());

        match self.db_key_search(&db_key) {
            Ok(old_pointer) => {
                // If a resource pointer with matching db_key is found,
                // replace the existing resource pointer with the new one.
                self.replace_resource_pointer(&old_pointer.id, resource_pointer)?;
            }
            Err(_) => {
                // If no resource pointer with matching db_key is found,
                // append the new data. We skip tag validation because the tags
                // have already been previously validated when adding into the
                // original resource.
                self.routing_resource._append_data_without_tag_validation(
                    &data,
                    metadata,
                    &embedding,
                    &resource_pointer.data_tag_names,
                );
            }
        }

        Ok(())
    }

    /// Search through the resource pointers to find if one exists with
    /// a matching db_key.
    pub fn db_key_search(&self, db_key: &str) -> Result<ResourcePointer, ResourceError> {
        let ret_data = self
            .routing_resource
            .metadata_search(&ResourceRouter::router_chunk_metadata_key(), db_key)?;

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
        let metadata = Some(resource_pointer._db_key_as_metadata_hashmap());
        let old_pointer_id = old_pointer_id
            .parse::<u64>()
            .map_err(|_| ResourceError::InvalidChunkId)?;

        self.routing_resource._replace_data_without_tag_validation(
            old_pointer_id,
            &data,
            metadata,
            &embedding,
            &resource_pointer.data_tag_names,
        )?;
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
