use crate::db::ShinkaiDB;
use crate::resources::embedding_generator::*;
use crate::resources::embeddings::*;
use crate::resources::model_type::*;
use crate::resources::resource_errors::*;
use ordered_float::NotNan;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::str::FromStr;

/// Enum used for all Resources to specify their type
/// when dealing with Trait objects.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ResourceType {
    Document,
    KeyValue,
}

impl ResourceType {
    pub fn to_str(&self) -> &str {
        match self {
            ResourceType::Document => "Document",
            ResourceType::KeyValue => "KeyValue",
        }
    }
}

impl FromStr for ResourceType {
    type Err = ResourceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Document" => Ok(ResourceType::Document),
            "KeyValue" => Ok(ResourceType::KeyValue),
            _ => Err(ResourceError::InvalidResourceType),
        }
    }
}

/// A data chunk that was retrieved from a vector similarity search.
/// Includes extra data like the resource_id of the resource it was from
/// and the similarity search score.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetrievedDataChunk {
    pub chunk: DataChunk,
    pub score: f32,
    pub resource_id: String,
}

/// Represents a data chunk with an id, data, and optional metadata.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DataChunk {
    pub id: String,
    pub data: String,
    pub metadata: Option<String>,
}

impl DataChunk {
    /// Creates a new `DataChunk` with a `String` id, data, and optional
    /// metadata.
    ///
    /// # Arguments
    ///
    /// * `id` - A `String` that holds the id of the `DataChunk`.
    /// * `data` - The data of the `DataChunk`.
    /// * `metadata` - Optional metadata for the `DataChunk`.
    ///
    /// # Returns
    ///
    /// A new `DataChunk` instance.
    pub fn new(id: String, data: &str, metadata: Option<&str>) -> Self {
        Self {
            id,
            data: data.to_string(),
            metadata: metadata.map(|s| s.to_string()),
        }
    }

    /// Creates a new `DataChunk` with a `u64` id converted to a `String`, data,
    /// and optional metadata.
    ///
    /// # Arguments
    ///
    /// * `id` - A `u64` that holds the id of the `DataChunk`. It gets converted
    ///   to a `String`.
    /// * `data` - The data of the `DataChunk`.
    /// * `metadata` - Optional metadata for the `DataChunk`.
    ///
    /// # Returns
    ///
    /// A new `DataChunk` instance.
    pub fn new_with_integer_id(id: u64, data: &str, metadata: Option<&str>) -> Self {
        Self::new(id.to_string(), data, metadata)
    }
}

/// Represents a Resource which includes properties and operations related to
/// data chunks and embeddings.
pub trait Resource {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn source(&self) -> Option<&str>;
    fn resource_id(&self) -> &str;
    fn resource_embedding(&self) -> &Embedding;
    fn resource_type(&self) -> ResourceType;
    fn embedding_model_used(&self) -> EmbeddingModelType;
    fn chunk_embeddings(&self) -> &Vec<Embedding>;
    fn set_resource_embedding(&mut self, embedding: Embedding);
    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType);

    /// Convert to json
    // Note we cannot add from_json in the trait due to trait object limitations
    // with &self.
    fn to_json(&self) -> Result<String, ResourceError>;

    /// Retrieves a data chunk given its id.
    ///
    /// # Arguments
    ///
    /// * `id` - The `String` id of the data chunk.
    ///
    /// # Returns
    ///
    /// A reference to the `DataChunk` if found, or an error.
    fn get_data_chunk(&self, id: String) -> Result<&DataChunk, ResourceError>;

    /// Returns a String representing the Key that this Resource
    /// will be/is saved to in the Topic::Resources in the DB.
    ///
    /// The db key is: `{name}.{resource_id}`
    fn db_key(&self) -> String {
        let name = self.name().replace(" ", "_");
        let resource_id = self.resource_id().replace(" ", "_");
        format!("{}.{}", name, resource_id)
    }

    /// Regenerates and updates the resource's embedding. The new
    /// embedding is generated using the provided `EmbeddingGenerator` plus
    /// the resource's name, description, source, and list of provided keywords.
    ///
    /// # Arguments
    ///
    /// * `generator` - The `EmbeddingGenerator` to be used for generating the
    ///   new embedding.
    /// * `keywords` - A list of Strings that are keywords from the resource
    ///   content which were externally extracted. Note these keywords are only
    ///   used for resource embedding generation, and are not saved in the
    ///   resource.
    ///
    /// # Returns
    ///
    /// * `Result<(), ResourceError>` - Returns `Ok(())` if the embedding
    /// is successfully updated, or an error if the embedding generation fails.
    fn update_resource_embedding(
        &mut self,
        generator: &dyn EmbeddingGenerator,
        keywords: Vec<String>,
    ) -> Result<(), ResourceError> {
        let formatted = self.resource_embedding_data_formatted(keywords);
        let new_embedding = generator
            .generate_embedding(&formatted, "RE")
            .map_err(|_| ResourceError::FailedEmbeddingGeneration)?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    /// Generates a formatted string that represents the data to be used for the
    /// resource embedding. This string includes the resource's name,
    /// description, source, and the maximum number of keywords which can be
    /// fit.
    ///
    /// * `keywords` - A list of Strings that are keywords from the resource
    ///   content which were externally extracted.
    ///
    /// # Returns
    ///
    /// * `String` - The formatted metadata string in the format of "Name: N,
    ///   Description: D, Source: S". If any are None, they are skipped.
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

    /// Performs a vector similarity search using a query embedding and returns
    /// the most similar data chunks within a specific range.
    ///
    /// # Arguments
    ///
    /// * `query` - An embedding that is the basis for the similarity search.
    /// * `num_of_results` - The number of top results to initially consider
    ///   (aka. upper max).
    /// * `tolerance_range` - A float between 0 and 1, inclusive, that
    ///   determines the range of acceptable similarity scores as a percentage
    ///   of the highest score. Any result outside this range is ignored.
    ///
    /// # Returns
    ///
    /// A vector of `DataChunk`s sorted by similarity
    /// score in descending order, but only including those within the tolerance
    /// range
    fn similarity_search_tolerance_ranged(
        &self,
        query: Embedding,
        num_of_results: u64,
        tolerance_range: f32,
    ) -> Vec<RetrievedDataChunk> {
        // Clamp the tolerance_range to be between 0 and 1
        let tolerance_range = tolerance_range.max(0.0).min(1.0);

        let mut results = self.similarity_search(query, num_of_results);

        // Calculate the range of acceptable similarity scores
        if let Some(ret_chunk) = results.first() {
            let lower_bound = ret_chunk.score * (1.0 - tolerance_range);

            // Filter the results to only include those within the tolerance range
            results.retain(|ret_chunk| ret_chunk.score >= lower_bound);
        }

        results
    }

    /// Performs a vector similarity search using a query embedding and returns
    /// the most similar data chunks.
    ///
    /// # Arguments
    ///
    /// * `query` - An embedding that is the basis for the similarity search.
    /// * `num_of_results` - The number of top results to return (top-k)
    ///
    /// # Returns
    ///
    /// A vector of `RetrievedDataChunk`s sorted by similarity score in descending order
    fn similarity_search(&self, query: Embedding, num_of_results: u64) -> Vec<RetrievedDataChunk> {
        let num_of_results = num_of_results as usize;

        // Calculate the similarity scores for all chunk embeddings and skip any that
        // are NaN
        let scores: Vec<(NotNan<f32>, String)> = self
            .chunk_embeddings()
            .iter()
            .filter_map(|embedding| {
                let similarity = query.cosine_similarity(embedding);
                match NotNan::new(similarity) {
                    Ok(not_nan_similarity) => Some((not_nan_similarity, embedding.id.clone())),
                    Err(_) => None, // Skip this embedding if similarity is NaN
                }
            })
            .collect();

        // Use a binary heap to more efficiently order the scores to get most similar
        let mut heap = BinaryHeap::with_capacity(num_of_results);
        for score in scores {
            println!("Current to be added to heap: (Id: {}, Score: {})", score.1, score.0);
            if heap.len() < num_of_results {
                heap.push(Reverse(score));
            } else if let Some(least_similar_score) = heap.peek() {
                // Access the tuple via `.0` and then the second element of the tuple via `.1`
                // Since the heap is a min-heap, we want to replace the least value only if
                // the new score is larger than the least score.
                if least_similar_score.0 .0 < score.0 {
                    heap.pop();
                    heap.push(Reverse(score));
                }
            }
        }

        // Fetch the RetrievedDataChunk matching the most similar embeddings
        let mut chunks: Vec<RetrievedDataChunk> = vec![];
        while let Some(Reverse((similarity, id))) = heap.pop() {
            println!("{}: {}%", id, similarity);
            if let Ok(chunk) = self.get_data_chunk(id) {
                println!("Pushing chunk: {:?}", chunk);
                chunks.push(RetrievedDataChunk {
                    chunk: chunk.clone(),
                    score: similarity.into_inner(),
                    resource_id: self.resource_id().to_string(),
                });
                println!("pusehd");
            }
        }

        println!("before rev Chunks: {:?}", chunks);
        // Reverse the order of chunks so that the highest score is first
        chunks.reverse();

        println!("Chunks: {:?}", chunks);

        chunks
    }
}
