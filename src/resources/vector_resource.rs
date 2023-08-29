use crate::resources::data_tags::{DataTag, DataTagIndex};
use crate::resources::embedding_generator::*;
use crate::resources::embeddings::*;
use crate::resources::model_type::*;
use crate::resources::resource_errors::*;
use ordered_float::NotNan;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::str::FromStr;

use super::router::VectorResourcePointer;

/// Enum used for all VectorResources to specify their type
/// when dealing with Trait objects.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VectorResourceType {
    Document,
    KeyValue,
}

impl VectorResourceType {
    pub fn to_str(&self) -> &str {
        match self {
            VectorResourceType::Document => "Document",
            VectorResourceType::KeyValue => "KeyValue",
        }
    }
}

impl FromStr for VectorResourceType {
    type Err = VectorResourceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Document" => Ok(VectorResourceType::Document),
            "KeyValue" => Ok(VectorResourceType::KeyValue),
            _ => Err(VectorResourceError::InvalidVectorResourceType),
        }
    }
}

/// A data chunk that was retrieved from a vector search.
/// Includes extra data like the resource_id of the resource it was from
/// and the vector search score.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetrievedDataChunk {
    pub chunk: DataChunk,
    pub score: f32,
    pub resource_pointer: VectorResourcePointer,
}

impl RetrievedDataChunk {
    /// Sorts the list of RetrievedDataChunks based on their scores.
    /// Uses a binary heap for efficiency, returns num_results of highest scored.
    pub fn sort_by_score(retrieved_data: &Vec<RetrievedDataChunk>, num_results: u64) -> Vec<RetrievedDataChunk> {
        // Create a HashMap to store the RetrievedDataChunk instances by their id
        let mut data_chunks: HashMap<String, RetrievedDataChunk> = HashMap::new();

        // Map the retrieved_data to a vector of tuples (NotNan<f32>, id)
        let scores: Vec<(NotNan<f32>, String)> = retrieved_data
            .into_iter()
            .map(|data_chunk| {
                let id = data_chunk.chunk.id.clone();
                data_chunks.insert(id.clone(), data_chunk.clone());
                (NotNan::new(data_chunks[&id].score).unwrap(), id)
            })
            .collect();

        // Use the bin_heap_order_scores function to sort the scores
        let sorted_scores = Embedding::bin_heap_order_scores(scores, num_results as usize);

        // Map the sorted_scores back to a vector of RetrievedDataChunk
        let sorted_data: Vec<RetrievedDataChunk> = sorted_scores
            .into_iter()
            .map(|(_, id)| data_chunks[&id].clone())
            .collect();

        sorted_data
    }
}

/// Represents a data chunk with an id, data, and optional metadata.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DataChunk {
    pub id: String,
    pub data: String,
    pub metadata: Option<HashMap<String, String>>,
    pub data_tag_names: Vec<String>, // `DataTag` type is excessively heavy when we convert to JSON, thus we just use the names here
}

impl DataChunk {
    pub fn new(
        id: String,
        data: &str,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        Self {
            id,
            data: data.to_string(),
            metadata,
            data_tag_names: data_tag_names.clone(),
        }
    }

    pub fn new_with_integer_id(
        id: u64,
        data: &str,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        Self::new(id.to_string(), data, metadata, data_tag_names)
    }
}

/// Represents a VectorResource which includes properties and operations related to
/// data chunks and embeddings.
pub trait VectorResource {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn source(&self) -> Option<&str>;
    fn resource_id(&self) -> &str;
    fn resource_embedding(&self) -> &Embedding;
    fn set_resource_embedding(&mut self, embedding: Embedding);
    fn resource_type(&self) -> VectorResourceType;
    fn embedding_model_used(&self) -> EmbeddingModelType;
    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType);
    fn chunk_embeddings(&self) -> Vec<Embedding>;
    fn data_tag_index(&self) -> &DataTagIndex;

    // Note we cannot add from_json in the trait due to trait object limitations
    // with &self.
    fn to_json(&self) -> Result<String, VectorResourceError>;

    /// Retrieves a data chunk given its id.
    fn get_data_chunk(&self, id: String) -> Result<DataChunk, VectorResourceError>;

    /// Naively searches through all chunk embeddings in the resource
    /// to find one with a matching id
    fn get_chunk_embedding(&self, id: &str) -> Result<Embedding, VectorResourceError> {
        for embedding in self.chunk_embeddings() {
            if embedding.id == id {
                return Ok(embedding.clone());
            }
        }
        Err(VectorResourceError::InvalidChunkId)
    }

    /// Returns a String representing the Key that this VectorResource
    /// will be/is saved to in the Topic::VectorResources in the DB.
    /// The db key is: `{name}.{resource_id}`
    fn db_key(&self) -> String {
        let name = self.name().replace(" ", "_");
        let resource_id = self.resource_id().replace(" ", "_");
        format!("{}.{}", name, resource_id)
    }

    /// Regenerates and updates the resource's embedding.
    fn update_resource_embedding(
        &mut self,
        generator: &dyn EmbeddingGenerator,
        keywords: Vec<String>,
    ) -> Result<(), VectorResourceError> {
        let formatted = self.resource_embedding_data_formatted(keywords);
        let new_embedding = generator
            .generate_embedding_with_id(&formatted, "RE")
            .map_err(|_| VectorResourceError::FailedEmbeddingGeneration)?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    /// Generates a formatted string that represents the data to be used for the
    /// resource embedding. This string includes the resource's name,
    /// description, source, and the maximum number of keywords which can be
    /// fit.
    fn resource_embedding_data_formatted(&self, keywords: Vec<String>) -> String {
        let name = format!("Name: {}", self.name());
        let desc = self
            .description()
            .map(|description| format!(", Description: {}", description))
            .unwrap_or_default();
        let source = self
            .source()
            .map(|source| format!(", Source: {}", source))
            .unwrap_or_default();

        // Take keywords until we hit an upper 495 character cap to ensure
        // we do not go past the embedding LLM context window.
        let pre_keyword_length = name.len() + desc.len() + source.len();
        let mut keyword_string = String::new();
        for phrase in keywords {
            if pre_keyword_length + keyword_string.len() + phrase.len() <= 495 {
                keyword_string = format!("{}, {}", keyword_string, phrase);
            }
        }

        format!("{}{}{}, Keywords: [{}]", name, desc, source, keyword_string)
    }

    /// Performs a vector search using a query embedding and returns
    /// the most similar data chunks within a specific range.
    ///
    /// * `tolerance_range` - A float between 0 and 1, inclusive, that
    ///   determines the range of acceptable similarity scores as a percentage
    ///   of the highest score.
    fn vector_search_tolerance_ranged(&self, query: Embedding, tolerance_range: f32) -> Vec<RetrievedDataChunk> {
        // Get top 100 results
        let results = self.vector_search(query.clone(), 100);

        // Calculate the top similarity score
        let top_similarity_score = results.first().map_or(0.0, |ret_chunk| ret_chunk.score);

        // Find the range of acceptable similarity scores
        self.vector_search_tolerance_ranged_score(query, tolerance_range, top_similarity_score)
    }

    /// Performs a vector search using a query embedding and returns
    /// the most similar data chunks within a specific range of the top similarity score.
    ///
    /// * `top_similarity_score` - A float that represents the top similarity score.
    fn vector_search_tolerance_ranged_score(
        &self,
        query: Embedding,
        tolerance_range: f32,
        top_similarity_score: f32,
    ) -> Vec<RetrievedDataChunk> {
        // Clamp the tolerance_range to be between 0 and 1
        let tolerance_range = tolerance_range.max(0.0).min(1.0);

        let mut results = self.vector_search(query, 100);

        // Calculate the range of acceptable similarity scores
        let lower_bound = top_similarity_score * (1.0 - tolerance_range);

        // Filter the results to only include those within the range of the top similarity score
        results.retain(|ret_chunk| ret_chunk.score >= lower_bound && ret_chunk.score <= top_similarity_score);

        results
    }

    /// Performs a vector search using a query embedding and returns
    /// the most similar data chunks.
    fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<RetrievedDataChunk> {
        // Fetch the ordered scores from the abstracted function
        let scores = query.score_similarities(&self.chunk_embeddings(), num_of_results);

        // Fetch the RetrievedDataChunk matching the most similar embeddings
        let mut chunks: Vec<RetrievedDataChunk> = vec![];
        for (score, id) in scores {
            if let Ok(chunk) = self.get_data_chunk(id) {
                chunks.push(RetrievedDataChunk {
                    chunk: chunk.clone(),
                    score,
                    resource_pointer: self.get_resource_pointer(),
                });
            }
        }

        chunks
    }

    /// Performs a syntactic vector search using a query embedding and a list of data tag names
    /// and returns the most similar data chunks.
    fn syntactic_vector_search(
        &self,
        query: Embedding,
        num_of_results: u64,
        data_tag_names: &Vec<String>,
    ) -> Vec<RetrievedDataChunk> {
        // Fetch all data chunks with matching data tags
        let mut matching_data_tag_embeddings = vec![];
        for name in data_tag_names {
            println!("\nData tag name: {}", name);
            if let Some(ids) = self.data_tag_index().get_chunk_ids(&name) {
                println!("Matching data tag chunk ids: {:?}", ids);
                if !ids.is_empty() {
                    for id in ids {
                        if let Ok(embedding) = self.get_chunk_embedding(&id) {
                            println!("Appending embedding");
                            matching_data_tag_embeddings.push(embedding.clone());
                        }
                    }
                }
            }
        }
        // Acquires similarity score of the embeddings within the temp doc
        // let scores = temp_doc._vector_search_score_results(query, num_of_results);
        let scores = query.score_similarities(&matching_data_tag_embeddings, num_of_results);
        // Manually fetches the correct data chunks in the temp doc via iterative fetching
        let mut results: Vec<RetrievedDataChunk> = vec![];
        for (score, id) in scores {
            if let Ok(chunk) = self.get_data_chunk(id) {
                results.push(RetrievedDataChunk {
                    chunk: chunk.clone(),
                    score,
                    resource_pointer: self.get_resource_pointer(),
                });
            }
        }

        println!("Syntactic vector search results: {:?}", results);

        results
    }

    /// Generates a pointer out of the resource. Of note this is required to get around
    /// the fact that this is a trait object.
    fn get_resource_pointer(&self) -> VectorResourcePointer {
        let db_key = self.db_key();
        let resource_type = self.resource_type();
        let embedding = self.resource_embedding().clone();

        // Fetch list of data tag names from the index
        let tag_names = self.data_tag_index().data_tag_names();

        VectorResourcePointer::new(&db_key, resource_type, Some(embedding), tag_names)
    }
}
