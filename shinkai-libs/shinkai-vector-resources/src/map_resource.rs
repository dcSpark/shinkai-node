use crate::base_vector_resources::{BaseVectorResource, VectorResourceBaseType};
use crate::data_tags::{DataTag, DataTagIndex};
use crate::embeddings::Embedding;
use crate::model_type::{EmbeddingModelType, RemoteModel};
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::{DataChunk, DataContent, RetrievedDataChunk, VRPath, VectorResource};
use serde_json;
use std::collections::HashMap;

/// A VectorResource which uses an internal HashMap data model, thus providing a
/// native key-value interface.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapVectorResource {
    name: String,
    description: Option<String>,
    source: VRSource,
    resource_id: String,
    resource_embedding: Embedding,
    resource_base_type: VectorResourceBaseType,
    embedding_model_used: EmbeddingModelType,
    chunk_embeddings: HashMap<String, Embedding>,
    chunk_count: u64,
    data_chunks: HashMap<String, DataChunk>,
    data_tag_index: DataTagIndex,
}

impl VectorResource for MapVectorResource {
    fn data_tag_index(&self) -> &DataTagIndex {
        &self.data_tag_index
    }

    fn embedding_model_used(&self) -> EmbeddingModelType {
        self.embedding_model_used.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn source(&self) -> VRSource {
        self.source.clone()
    }

    fn resource_id(&self) -> &str {
        &self.resource_id
    }

    fn resource_embedding(&self) -> &Embedding {
        &self.resource_embedding
    }

    fn resource_base_type(&self) -> VectorResourceBaseType {
        self.resource_base_type.clone()
    }

    fn chunk_embeddings(&self) -> Vec<Embedding> {
        self.chunk_embeddings.values().cloned().collect()
    }

    fn to_json(&self) -> Result<String, VectorResourceError> {
        serde_json::to_string(self).map_err(|_| VectorResourceError::FailedJSONParsing)
    }

    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType) {
        self.embedding_model_used = model_type;
    }

    fn set_resource_embedding(&mut self, embedding: Embedding) {
        self.resource_embedding = embedding;
    }

    /// Retrieves a data chunk's embedding given its key (id)
    fn get_chunk_embedding(&self, key: String) -> Result<Embedding, VectorResourceError> {
        Ok(self
            .chunk_embeddings
            .get(&key)
            .ok_or(VectorResourceError::InvalidChunkId)?
            .clone())
    }

    /// Retrieves a data chunk given its key (id)
    fn get_data_chunk(&self, key: String) -> Result<DataChunk, VectorResourceError> {
        Ok(self
            .data_chunks
            .get(&key)
            .ok_or(VectorResourceError::InvalidChunkId)?
            .clone())
    }

    /// Returns all data chunks in the MapVectorResource
    fn get_all_data_chunks(&self) -> Vec<DataChunk> {
        self.data_chunks.values().cloned().collect()
    }
}

impl MapVectorResource {
    /// * `resource_id` - This can be the Sha256 hash as a String from the bytes of the original data
    /// or anything that is deterministic to ensure duplicates are not possible.
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        resource_id: &str,
        resource_embedding: Embedding,
        chunk_embeddings: HashMap<String, Embedding>,
        data_chunks: HashMap<String, DataChunk>,
        embedding_model_used: EmbeddingModelType,
    ) -> Self {
        MapVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source,
            resource_id: String::from(resource_id),
            resource_embedding,
            chunk_embeddings,
            chunk_count: data_chunks.len() as u64,
            resource_base_type: VectorResourceBaseType::Map,
            data_chunks,
            embedding_model_used,
            data_tag_index: DataTagIndex::new(),
        }
    }

    /// Initializes an empty `MapVectorResource` with empty defaults.
    pub fn new_empty(name: &str, desc: Option<&str>, source: VRSource, resource_id: &str) -> Self {
        MapVectorResource::new(
            name,
            desc,
            source,
            resource_id,
            Embedding::new(&String::new(), vec![]),
            HashMap::new(),
            HashMap::new(),
            EmbeddingModelType::RemoteModel(RemoteModel::AllMiniLML12v2),
        )
    }

    /// Returns all DataChunks with a matching key/value pair in the metadata hashmap
    /// Does not perform any traversal.
    pub fn metadata_search(
        &self,
        metadata_key: &str,
        metadata_value: &str,
    ) -> Result<Vec<RetrievedDataChunk>, VectorResourceError> {
        let mut matching_chunks = Vec::new();

        for chunk in self.data_chunks.values() {
            match &chunk.metadata {
                Some(metadata) if metadata.get(metadata_key) == Some(&metadata_value.to_string()) => matching_chunks
                    .push(RetrievedDataChunk {
                        chunk: chunk.clone(),
                        score: 0.00,
                        resource_pointer: self.get_resource_pointer(),
                        retrieval_depth: 0,
                        retrieval_path: VRPath::new(),
                    }),
                _ => (),
            }
        }

        if matching_chunks.is_empty() {
            return Err(VectorResourceError::NoChunkFound);
        }

        Ok(matching_chunks)
    }

    /// Inserts a new data chunk (with a BaseVectorResource) and associated embeddings to the Map resource
    /// and updates the data tags index.
    pub fn insert_vector_resource(
        &mut self,
        key: &str,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) {
        let embedding = resource.as_trait_object().resource_embedding().clone();
        let tag_names = resource.as_trait_object().data_tag_index().data_tag_names();
        self._insert_kv_without_tag_validation(key, DataContent::Resource(resource), metadata, &embedding, &tag_names)
    }

    /// Inserts a new data chunk (with a String value) and associated embeddings to the Map resource
    /// and updates the data tags index.
    pub fn insert_kv(
        &mut self,
        key: &str,
        value: &str,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) {
        let validated_data_tags = DataTag::validate_tag_list(value, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._insert_kv_without_tag_validation(
            key,
            DataContent::Data(value.to_string()),
            metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Insert a new data chunk and associated embeddings to the Map resource
    /// without checking if tags are valid. Also used by resource router.
    pub fn _insert_kv_without_tag_validation(
        &mut self,
        key: &str,
        data: DataContent,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        tag_names: &Vec<String>,
    ) {
        let data_chunk = match data {
            DataContent::Data(data_string) => {
                DataChunk::new(key.to_string(), &data_string, metadata.clone(), tag_names)
            }
            DataContent::Resource(resource) => {
                DataChunk::new_vector_resource(key.to_string(), &resource, metadata.clone())
            }
        };
        self.data_tag_index.add_chunk(&data_chunk);

        // Embedding details
        let mut embedding = embedding.clone();
        embedding.set_id(key.to_string());
        self.insert_data_chunk(data_chunk.clone());
        self.chunk_embeddings.insert(data_chunk.id.clone(), embedding);
    }

    /// Replaces an existing data chunk (based on key) and associated embeddings in the Map resource
    /// with a BaseVectorResource in the new DataChunk, and updates the data tags index.
    pub fn replace_vector_resource(
        &mut self,
        key: &str,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
    ) -> Result<DataChunk, VectorResourceError> {
        let embedding = new_resource.as_trait_object().resource_embedding().clone();
        let tag_names = new_resource.as_trait_object().data_tag_index().data_tag_names();
        self._replace_kv_without_tag_validation(
            key,
            DataContent::Resource(new_resource),
            new_metadata,
            &embedding,
            &tag_names,
        )
    }

    /// Replaces an existing data chunk & associated embedding and updates the data tags index.
    /// * `id` - The id of the data chunk to be replaced.
    pub fn replace_kv(
        &mut self,
        key: &str,
        new_value: &str,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the new data with
    ) -> Result<DataChunk, VectorResourceError> {
        // Validate which tags will be saved with the new data
        let validated_data_tags = DataTag::validate_tag_list(new_value, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._replace_kv_without_tag_validation(
            key,
            DataContent::Data(new_value.to_string()),
            new_metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Replaces an existing data chunk & associated embeddings in the Map resource
    /// without checking if tags are valid. Used for resource router.
    pub fn _replace_kv_without_tag_validation(
        &mut self,
        key: &str,
        new_data: DataContent,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        new_tag_names: &Vec<String>,
    ) -> Result<DataChunk, VectorResourceError> {
        // Next create the new chunk, and replace the old chunk in the data_chunks list
        let new_chunk = match new_data {
            DataContent::Data(data_string) => {
                DataChunk::new(key.to_string(), &data_string, new_metadata.clone(), new_tag_names)
            }
            DataContent::Resource(resource) => {
                DataChunk::new_vector_resource(key.to_string(), &resource, new_metadata.clone())
            }
        };
        let old_chunk = self
            .data_chunks
            .insert(key.to_string(), new_chunk.clone())
            .ok_or(VectorResourceError::InvalidChunkId)?;

        // Then deletion of old chunk from index and addition of new chunk
        self.data_tag_index.remove_chunk(&old_chunk);
        self.data_tag_index.add_chunk(&new_chunk);

        // Finally replacing the embedding
        let mut embedding = embedding.clone();
        embedding.set_id(key.to_string());
        self.chunk_embeddings.insert(key.to_string(), embedding);

        Ok(old_chunk)
    }

    /// Deletes a data chunk and associated embedding from the resource
    /// and updates the data tags index.
    pub fn delete_kv(&mut self, key: &str) -> Result<(DataChunk, Embedding), VectorResourceError> {
        let deleted_chunk = self.delete_data_chunk(key)?;
        self.data_tag_index.remove_chunk(&deleted_chunk);
        let deleted_embedding = self
            .chunk_embeddings
            .remove(key)
            .ok_or(VectorResourceError::InvalidChunkId)?;

        Ok((deleted_chunk, deleted_embedding))
    }

    /// Internal data chunk deletion from the hashmap
    fn delete_data_chunk(&mut self, key: &str) -> Result<DataChunk, VectorResourceError> {
        self.chunk_count -= 1;
        let removed_chunk = self
            .data_chunks
            .remove(key)
            .ok_or(VectorResourceError::InvalidChunkId)?;
        Ok(removed_chunk)
    }

    // Inserts a data chunk into the data_chunks hashmap
    fn insert_data_chunk(&mut self, data_chunk: DataChunk) {
        self.chunk_count += 1;
        self.data_chunks.insert(data_chunk.id.clone(), data_chunk);
    }

    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
    }
}
