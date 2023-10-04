use super::base_vector_resources::BaseVectorResource;
use crate::base_vector_resources::VectorResourceBaseType;
use crate::embeddings::Embedding;
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::VectorResource;
use ordered_float::NotNan;
use std::collections::HashMap;

/// Contents of a DataChunk. Either the String data itself, or
/// another VectorResource
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DataContent {
    Data(String),
    Resource(BaseVectorResource),
}

/// A data chunk that was retrieved from a search.
/// Includes extra data like the resource_pointer of the resource it was from
/// and the similarity score from the vector search. Resource pointer is especially
/// helpful when you have multiple layers of VectorResources inside of each other.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetrievedDataChunk {
    pub chunk: DataChunk,
    pub score: f32,
    pub resource_pointer: VectorResourcePointer,
    pub retrieval_depth: u64,
}

impl RetrievedDataChunk {
    /// Sorts the list of RetrievedDataChunks based on their scores.
    /// Uses a binary heap for efficiency, returns num_results of highest scored.
    pub fn sort_by_score(retrieved_data: &Vec<RetrievedDataChunk>, num_results: u64) -> Vec<RetrievedDataChunk> {
        // Create a HashMap to store the RetrievedDataChunk instances for post-scoring retrieval
        let mut data_chunks: HashMap<String, RetrievedDataChunk> = HashMap::new();

        // Map the retrieved_data to a vector of tuples (NotNan<f32>, id_ref_key)
        // We create id_ref_key to support sorting RetrievedDataChunks from
        // different Resources together and avoid chunk id collision problems.
        let scores: Vec<(NotNan<f32>, String)> = retrieved_data
            .into_iter()
            .map(|data_chunk| {
                let ref_key = data_chunk.resource_pointer.reference.clone();
                let id_ref_key = format!("{}-{}", data_chunk.chunk.id.clone(), ref_key);
                data_chunks.insert(id_ref_key.clone(), data_chunk.clone());
                (NotNan::new(data_chunks[&id_ref_key].score).unwrap(), id_ref_key)
            })
            .collect();

        // Use the bin_heap_order_scores function to sort the scores
        let sorted_scores = Embedding::bin_heap_order_scores(scores, num_results as usize);

        // Map the sorted_scores back to a vector of RetrievedDataChunk
        let sorted_data: Vec<RetrievedDataChunk> = sorted_scores
            .into_iter()
            .map(|(_, id_db_key)| data_chunks[&id_db_key].clone())
            .collect();

        sorted_data
    }
}

/// Represents a data chunk with an id, data, and optional metadata.
/// Note: `DataTag` type is excessively heavy when we convert to JSON, thus we just use the
/// data tag names instead in the DataChunk.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DataChunk {
    pub id: String,
    pub data: DataContent,
    pub metadata: Option<HashMap<String, String>>,
    pub data_tag_names: Vec<String>,
}

impl DataChunk {
    /// Create a new String-holding DataChunk with a provided String id
    pub fn new(
        id: String,
        data: &str,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        Self {
            id,
            data: DataContent::Data(data.to_string()),
            metadata,
            data_tag_names: data_tag_names.clone(),
        }
    }

    /// Create a new String-holding DataChunk with a provided u64 id, which gets converted to string internally
    pub fn new_with_integer_id(
        id: u64,
        data: &str,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        Self::new(id.to_string(), data, metadata, data_tag_names)
    }

    /// Create a new BaseVectorResource-holding DataChunk with a provided String id
    pub fn new_vector_resource(
        id: String,
        vector_resource: &BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        DataChunk {
            id: id,
            data: DataContent::Resource(vector_resource.clone()),
            metadata: metadata,
            data_tag_names: vector_resource.as_trait_object().data_tag_index().data_tag_names(),
        }
    }

    /// Create a new BaseVectorResource-holding DataChunk with a provided u64 id, which gets converted to string internally
    pub fn new_vector_resource_with_integer_id(
        id: u64,
        vector_resource: &BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        Self::new_vector_resource(id.to_string(), vector_resource, metadata)
    }

    /// Attempts to read the data String from the DataChunk. Errors if data is a VectorResource
    pub fn get_data_string(&self) -> Result<String, VectorResourceError> {
        match &self.data {
            DataContent::Data(s) => Ok(s.clone()),
            DataContent::Resource(_) => Err(VectorResourceError::DataIsNonMatchingType),
        }
    }

    /// Attempts to read the BaseVectorResource from the DataChunk. Errors if data is an actual String
    pub fn get_data_vector_resource(&self) -> Result<BaseVectorResource, VectorResourceError> {
        match &self.data {
            DataContent::Data(_) => Err(VectorResourceError::DataIsNonMatchingType),
            DataContent::Resource(resource) => Ok(resource.clone()),
        }
    }
}

/// Type which holds referential data about a given resource.
/// `reference` holds a string which points back to the original resource that
/// the pointer was created out of.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorResourcePointer {
    pub reference: String,
    pub resource_base_type: VectorResourceBaseType,
    pub resource_source: VRSource,
    pub data_tag_names: Vec<String>,
    pub resource_embedding: Option<Embedding>,
    // pub metadata: HashMap<String, String>,
}

impl VectorResourcePointer {
    /// Create a new VectorResourcePointer
    pub fn new(
        reference: &str,
        resource_base_type: VectorResourceBaseType,
        resource_embedding: Option<Embedding>,
        data_tag_names: Vec<String>,
        resource_source: VRSource,
    ) -> Self {
        Self {
            reference: reference.to_string(),
            resource_base_type,
            resource_embedding: resource_embedding.clone(),
            data_tag_names: data_tag_names,
            resource_source,
        }
    }

    /// Returns the name of the referenced resource, which is the part of the reference before the first ':'.
    /// If no ':' is found, the whole reference is returned.
    pub fn name(&self) -> String {
        match self.reference.find(':') {
            Some(index) => self.reference[..index].to_string(),
            None => self.reference.clone(),
        }
    }
}

impl From<Box<dyn VectorResource>> for VectorResourcePointer {
    fn from(resource: Box<dyn VectorResource>) -> Self {
        resource.get_resource_pointer()
    }
}
