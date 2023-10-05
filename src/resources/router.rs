use serde_json;
use shinkai_vector_resources::base_vector_resources::VectorResourceBaseType;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::map_resource::MapVectorResource;
use shinkai_vector_resources::resource_errors::VectorResourceError;
use shinkai_vector_resources::source::VRSource;
use shinkai_vector_resources::vector_resource::{
    DataContent, RetrievedDataChunk, VectorResource, VectorResourcePointer,
};
use shinkai_vector_resources::vector_resource_types::VRPath;
use std::collections::HashMap;
use std::convert::From;
use std::str::FromStr;

/// A top level struct which indexes a series of resource pointers
/// using a MapVectorResource
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorResourceRouter {
    routing_resource: MapVectorResource,
}

impl VectorResourceRouter {
    /// Create a new VectorResourceRouter instance from scratch.
    pub fn new() -> Self {
        let name = "VectorResource Router";
        let desc = Some("Enables performing vector searches to find relevant resources.");
        let source = VRSource::None;
        let resource_id = "resource_router";
        VectorResourceRouter {
            routing_resource: MapVectorResource::new_empty(name, desc, source, resource_id),
        }
    }

    /// A hard-coded DB key for the profile-wide VectorResource Router in Topic::VectorResources.
    /// No other resource is allowed to use this shinkai_db_key (this is enforced
    /// automatically because all resources have a two-part key)
    pub fn profile_router_shinkai_db_key() -> String {
        "profile_resource_router".to_string()
    }

    /// Returns a list of VectorResourcePointers of the most similar resources that
    /// have matching data tag names.
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

    /// Returns a list of VectorResourcePointers of the most similar resources.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<VectorResourcePointer> {
        let chunks = self.routing_resource.vector_search(query, num_of_results);
        self.ret_data_chunks_to_pointers(&chunks)
    }

    /// Returns all VectorResourcePointers in the Resource Router
    pub fn get_all_resource_pointers(&self) -> Vec<VectorResourcePointer> {
        let data_chunks = self.routing_resource.get_all_data_chunks();
        let map_resource_pointer = self.routing_resource.get_resource_pointer();
        let mut resource_pointers = vec![];

        for chunk in data_chunks {
            let retrieved_data_chunk = RetrievedDataChunk {
                chunk: chunk.clone(),
                score: 0.0,
                resource_pointer: map_resource_pointer.clone(),
                retrieval_path: VRPath::new(),
            };

            let pointers = self.ret_data_chunks_to_pointers(&vec![retrieved_data_chunk]);
            resource_pointers.extend(pointers);
        }

        resource_pointers
    }

    /// Takes a list of RetrievedDataChunks and outputs a list of VectorResourcePointers
    /// that point to the real resource (not the resource router).
    ///
    /// Of note, if a chunk holds an invalid VectorResourceBaseType string then the chunk
    /// is ignored.
    fn ret_data_chunks_to_pointers(&self, ret_chunks: &Vec<RetrievedDataChunk>) -> Vec<VectorResourcePointer> {
        let mut resource_pointers = vec![];
        for ret_chunk in ret_chunks {
            // Ignore resources added to the router with invalid resource types
            if let DataContent::Data(data) = &ret_chunk.chunk.data {
                if let Ok(resource_base_type) = VectorResourceBaseType::from_str(data)
                    .map_err(|_| VectorResourceError::InvalidVectorResourceBaseType)
                {
                    let id = &ret_chunk.chunk.id;
                    let embedding = self.routing_resource.get_chunk_embedding(id.to_string()).ok();

                    // Extract the "source" field from the metadata
                    let source = ret_chunk
                        .chunk
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.get("source"))
                        .and_then(|source_json| VRSource::from_json(source_json).ok())
                        .unwrap_or(VRSource::None);

                    let resource_pointer = VectorResourcePointer::new(
                        &id,
                        resource_base_type,
                        embedding,
                        ret_chunk.chunk.data_tag_names.clone(),
                        source,
                    );
                    resource_pointers.push(resource_pointer);
                }
            }
        }
        resource_pointers
    }

    /// Extracts necessary data from a VectorResourcePointer to create a DataChunk
    fn extract_pointer_data(
        &self,
        resource_pointer: &VectorResourcePointer,
    ) -> Result<(String, String, Embedding, Option<HashMap<String, String>>), VectorResourceError> {
        let data = resource_pointer.resource_base_type.to_str().to_string();
        let embedding = resource_pointer
            .resource_embedding
            .clone()
            .ok_or(VectorResourceError::NoEmbeddingProvided)?;
        let shinkai_db_key = resource_pointer.reference.to_string();
        let metadata = match resource_pointer.resource_source.to_json() {
            Ok(source_json) => {
                let mut metadata_map = HashMap::new();
                metadata_map.insert("source".to_string(), source_json);
                Some(metadata_map)
            }
            Err(_) => None,
        };

        Ok((shinkai_db_key, data, embedding, metadata))
    }

    /// Adds a resource pointer into the VectorResourceRouter instance.
    /// The pointer is expected to have a valid resource embedding
    /// and the matching resource having already been saved into the DB.
    ///
    /// If a resource pointer already exists with the same shinkai_db_key, then
    /// the old pointer will be replaced.
    ///
    /// Of note, in this implementation we store the resource type in the `data`
    /// of the chunk and the shinkai db key (pointer reference) as the id of the data chunk.
    pub fn add_resource_pointer(
        &mut self,
        resource_pointer: &VectorResourcePointer,
    ) -> Result<(), VectorResourceError> {
        let (shinkai_db_key, data, embedding, metadata) = self.extract_pointer_data(resource_pointer)?;
        let shinkai_db_key_clone = shinkai_db_key.clone();

        match self.routing_resource.get_data_chunk(shinkai_db_key_clone) {
            Ok(old_chunk) => {
                // If a resource pointer with matching shinkai_db_key is found,
                // replace the existing resource pointer with the new one.
                self.replace_resource_pointer(&old_chunk.id, resource_pointer)?;
            }
            Err(_) => {
                // If no resource pointer with matching shinkai_db_key is found,
                // insert the new kv pair. We skip tag validation because the tags
                // have already been previously validated when adding into the
                // original resource.
                self.routing_resource._insert_kv_without_tag_validation(
                    &shinkai_db_key,
                    DataContent::Data(data.to_string()),
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
        resource_pointer: &VectorResourcePointer,
    ) -> Result<(), VectorResourceError> {
        let (_, data, embedding, metadata) = self.extract_pointer_data(resource_pointer)?;

        self.routing_resource._replace_kv_without_tag_validation(
            old_pointer_id,
            DataContent::Data(data.to_string()),
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
            self.routing_resource
                .get_chunk_embedding(resource_pointer.reference.to_string())
        }
    }

    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        Ok(VectorResourceRouter {
            routing_resource: MapVectorResource::from_json(json)?,
        })
    }
    /// Convert to json
    pub fn to_json(&self) -> Result<String, VectorResourceError> {
        serde_json::to_string(self).map_err(|_| VectorResourceError::FailedJSONParsing)
    }
}
