use crate::resources::embeddings::*;
use crate::resources::kv_resource::KVVectorResource;
use crate::resources::resource_errors::*;
use crate::resources::vector_resource::*;
use serde_json;
use std::collections::HashMap;
use std::convert::From;
use std::convert::TryFrom;
use std::str::FromStr;

use super::data_tags::DataTag;

/// Type which holds reference data about a resource in the DB.
///
/// This hides away the implementation details of the current underlying DocumentVectorResource
/// and allows us to offer an equivalent interface in the future even if we swap to
/// a different underlying internal model of how the resource pointer data is stored.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorResourcePointer {
    pub db_key: String,
    pub resource_type: VectorResourceType,
    data_tag_names: Vec<String>,
    resource_embedding: Option<Embedding>,
}

impl VectorResourcePointer {
    /// Create a new VectorResourcePointer
    pub fn new(
        db_key: &str,
        resource_type: VectorResourceType,
        resource_embedding: Option<Embedding>,
        data_tag_names: Vec<String>,
    ) -> Self {
        Self {
            db_key: db_key.to_string(),
            resource_type,
            resource_embedding: resource_embedding.clone(),
            data_tag_names: data_tag_names,
        }
    }

    // Wraps the resource pointer's db_key into a hashmap ready to use for
    // the resource router's chunk metadata
    // pub fn _db_key_as_metadata_hashmap(&self) -> HashMap<String, String> {
    //     let mut hmap = HashMap::new();
    //     hmap.insert(VectorResourceRouter::router_chunk_metadata_key(), self.db_key.clone());
    //     hmap
    // }
}

impl From<Box<dyn VectorResource>> for VectorResourcePointer {
    fn from(resource: Box<dyn VectorResource>) -> Self {
        resource.get_resource_pointer()
    }
}

/// A top level struct which indexes a series of resource pointers.
/// This struct thus makes it possible to perform vector searches to find
/// relevant VectorResources for users or agents.
///
/// For now we just implement this on top of DocumentVectorResource for speed of
/// implementation, later on we can come around and design something
/// specifically for routing that is more effective if needed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorResourceRouter {
    routing_resource: KVVectorResource,
}

impl VectorResourceRouter {
    /// Create a new VectorResourceRouter instance from scratch.
    pub fn new() -> Self {
        let name = "VectorResource Router";
        let desc = Some("Enables performing vector searches to find relevant resources.");
        let source = None;
        let resource_id = "resource_router";
        VectorResourceRouter {
            routing_resource: KVVectorResource::new_empty(name, desc, source, resource_id),
        }
    }

    /// A hard-coded DB key for the Profile VectorResource router in Topic::VectorResources.
    /// No other resource is allowed to use this db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn db_key() -> String {
        "profile_resource_router".to_string()
    }

    /// Performs a syntactic vector search using a query embedding and list of data tag names.
    /// Returns a list of VectorResourcePointers of the most similar VectorResources.
    pub fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<VectorResourcePointer> {
        let chunks = self
            .routing_resource
            .syntactic_vector_search(query, num_of_results, data_tag_names);
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// Performs a vector search using a query embedding and returns
    /// a list of VectorResourcePointers of the most similar VectorResources.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<VectorResourcePointer> {
        let chunks = self.routing_resource.vector_search(query, num_of_results);
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// A hardcoded key string used for the metadata hashmap of data chunks
    /// in the router's internal resource
    fn router_chunk_metadata_key() -> String {
        "db_key".to_string()
    }

    /// Takes a list of RetrievedDataChunks and outputs a list of VectorResourcePointers
    /// that point to the real resource (not the resource router).
    ///
    /// Of note, if a chunk holds an invalid VectorResourceType string then the chunk
    /// is ignored.
    fn ret_data_chunks_to_pointers(&self, ret_chunks: &Vec<RetrievedDataChunk>) -> Vec<VectorResourcePointer> {
        let mut resource_pointers = vec![];
        for ret_chunk in ret_chunks {
            // Ignore resources added to the router with invalid resource types

            if let Ok(resource_type) = VectorResourceType::from_str(&ret_chunk.chunk.data)
                .map_err(|_| VectorResourceError::InvalidVectorResourceType)
            {
                let id = &ret_chunk.chunk.id;
                let embedding = self.routing_resource.get_chunk_embedding(id).ok();
                let resource_pointer =
                    VectorResourcePointer::new(&id, resource_type, embedding, ret_chunk.chunk.data_tag_names.clone());
                resource_pointers.push(resource_pointer);
            }
        }
        resource_pointers
    }

    /// Adds a resource pointer into the VectorResourceRouter instance.
    /// The pointer is expected to have a valid resource embedding
    /// and the matching resource having already been saved into the DB.
    ///
    /// If a resource pointer already exists with the same db_key, then
    /// the old pointer will be replaced.
    ///
    /// Of note, in this implementation we store the resource type in the `data`
    /// of the chunk and the db_key as the id of the data chunk.
    pub fn add_resource_pointer(
        &mut self,
        resource_pointer: &VectorResourcePointer,
    ) -> Result<(), VectorResourceError> {
        let data = resource_pointer.resource_type.to_str();
        let embedding = resource_pointer
            .resource_embedding
            .clone()
            .ok_or(VectorResourceError::NoEmbeddingProvided)?;
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

    /// Search through the resource pointers to find if one exists with
    /// a matching db_key.
    pub fn db_key_search(&self, db_key: &str) -> Result<VectorResourcePointer, VectorResourceError> {
        let ret_data = self
            .routing_resource
            .metadata_search(&VectorResourceRouter::router_chunk_metadata_key(), db_key)?;

        if let Some(res_pointer) = self.ret_data_chunks_to_pointers(&ret_data).get(0).cloned() {
            return Ok(res_pointer);
        } else {
            Err(VectorResourceError::NoChunkFound)
        }
    }

    /// Replaces an existing resource pointer with a new one.
    pub fn replace_resource_pointer(
        &mut self,
        old_pointer_id: &str,
        resource_pointer: &VectorResourcePointer,
    ) -> Result<(), VectorResourceError> {
        let data = resource_pointer.resource_type.to_str();
        let embedding = resource_pointer
            .resource_embedding
            .clone()
            .ok_or(VectorResourceError::NoEmbeddingProvided)?;
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

    /// Deletes the resource pointer inside of the VectorResourceRouter given a valid id
    pub fn delete_resource_pointer(&mut self, old_pointer_id: &str) -> Result<(), VectorResourceError> {
        self.routing_resource.delete_kv(old_pointer_id)?;
        Ok(())
    }

    /// Acquire the resource_embedding for a given VectorResourcePointer.
    /// If the pointer itself doesn't have the embedding attached to it,
    /// we use the id to fetch the embedding directly from the VectorResourceRouter.
    pub fn get_resource_embedding(
        &self,
        resource_pointer: &VectorResourcePointer,
    ) -> Result<Embedding, VectorResourceError> {
        if let Some(embedding) = resource_pointer.resource_embedding.clone() {
            Ok(embedding)
        } else {
            self.routing_resource.get_chunk_embedding(&resource_pointer.db_key)
        }
    }

    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        Ok(VectorResourceRouter {
            routing_resource: KVVectorResource::from_json(json)?,
        })
    }
    /// Convert to json
    pub fn to_json(&self) -> Result<String, VectorResourceError> {
        serde_json::to_string(self).map_err(|_| VectorResourceError::FailedJSONParsing)
    }
}
