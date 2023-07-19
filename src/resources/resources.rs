use crate::resources::embeddings::*;
use crate::resources::resource_errors::*;
use ordered_float::NotNan;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;

/// Represents a data chunk with an id, data, and optional metadata.
#[derive(Debug, Clone, PartialEq)]
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
    fn resource_embedding(&self) -> &Embedding;
    fn chunk_embeddings(&self) -> &Vec<Embedding>;
    fn set_resource_embedding(&mut self, embedding: Embedding);

    /// Retrieves a data chunk given its id.
    ///
    /// # Arguments
    ///
    /// * `id` - The `String` id of the data chunk.
    ///
    /// # Returns
    ///
    /// A reference to the `DataChunk` if found, or an error.
    fn get_data_chunk(&self, id: String) -> Result<&DataChunk, Box<dyn std::error::Error>>;

    /// Regenerates and updates the resource's embedding. The new
    /// embedding is generated using the provided `EmbeddingGenerator` and
    /// the resource's name, description, and source.
    ///
    /// # Arguments
    ///
    /// * `generator` - The `EmbeddingGenerator` to be used for generating the
    ///   new embedding.
    ///
    /// # Returns
    ///
    /// * `Result<(), Box<dyn std::error::Error>>` - Returns `Ok(())` if the
    ///   embedding
    /// is successfully updated, or an error if the embedding generation fails.
    fn update_resource_embedding(&mut self, generator: &EmbeddingGenerator) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = self.resource_embedding_data_formatted();
        let new_embedding = generator.generate_embedding(&metadata, "1")?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    /// Generates a formatted string that represents the data to be used for the
    /// resource embedding. This string includes
    /// the resource's name, description, and source.
    ///
    /// # Returns
    ///
    /// * `String` - The formatted metadata string in the format of "Name: N,
    ///   Description: D, Source: S". If any are None, they are skipped.
    fn resource_embedding_data_formatted(&self) -> String {
        let name = format!("Name: {}", self.name());

        let desc = self
            .description()
            .map(|description| format!(", Description: {}", description))
            .unwrap_or_default();

        let source = self
            .source()
            .map(|source| format!(", Source: {}", source))
            .unwrap_or_default();

        format!("{}{}{}", name, desc, source)
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
    /// A `Result` that contains a vector of `DataChunk`s sorted by similarity
    /// score in descending order, or an error if something goes wrong.
    fn similarity_search(
        &self,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<DataChunk>, Box<dyn std::error::Error>> {
        let results = self._similarity_search(query, num_of_results)?;
        Ok(results.into_iter().map(|(chunk, _)| chunk).collect())
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
    /// A `Result` that contains a vector of `DataChunk`s sorted by similarity
    /// score in descending order, but only including those within the tolerance
    /// range, or an error if something goes wrong.
    fn similarity_search_tolerance_ranged(
        &self,
        query: Embedding,
        num_of_results: u64,
        tolerance_range: f32,
    ) -> Result<Vec<DataChunk>, Box<dyn std::error::Error>> {
        // Clamp the tolerance_range to be between 0 and 1
        let tolerance_range = tolerance_range.max(0.0).min(1.0);

        let mut results = self._similarity_search(query, num_of_results)?;

        // Calculate the range of acceptable similarity scores
        if let Some((_, highest_similarity)) = results.first() {
            let lower_bound = highest_similarity * (1.0 - tolerance_range);

            // Filter the results to only include those within the tolerance range
            results.retain(|&(_, similarity)| similarity >= lower_bound);
        }

        Ok(results.into_iter().map(|(chunk, _)| chunk).collect())
    }

    /// A helper function to perform a similarity search. This function is not
    /// meant to be used directly, but rather to provide shared
    /// functionality for the public similarity search methods.
    ///
    /// # Arguments
    ///
    /// * `query` - An embedding that is the basis for the similarity search.
    /// * `num_of_results` - The number of top results to return.
    ///
    /// # Returns
    ///
    /// A `Result` that contains a vector of tuples. Each tuple consists of a
    /// `DataChunk` and its similarity score. The vector is sorted by similarity
    /// score in descending order.
    fn _similarity_search(
        &self,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<(DataChunk, f32)>, Box<dyn Error>> {
        let num_of_results = num_of_results as usize;

        // Calculate the similarity scores for all chunk embeddings and skip any that
        // are NaN
        let scores: Vec<(String, NotNan<f32>)> = self
            .chunk_embeddings()
            .iter()
            .filter_map(|embedding| {
                let similarity = query.cosine_similarity(embedding);
                match NotNan::new(similarity) {
                    Ok(not_nan_similarity) => Some((embedding.id.clone(), not_nan_similarity)),
                    Err(_) => None, // Skip this embedding if similarity is NaN
                }
            })
            .collect();

        // Use a binary heap to more efficiently order the scores to get most similar
        let mut heap = BinaryHeap::with_capacity(num_of_results);
        for score in scores {
            println!("Current to be added to heap: (Id: {}, Score: {})", score.0, score.1);
            if heap.len() < 1 {
                println!("First score, added to heap: {}", score.1);
                heap.push(Reverse(score));
            } else if let Some(least_similar_score) = heap.peek() {
                // Access the tuple via `.0` and then the second element of the tuple via `.1`
                // Since the heap is a min-heap, we want to replace the least value only if
                // the new score is larger than the least score.
                if least_similar_score.0 .1 < score.1 {
                    println!(
                        "New score (Id: {}, Score: {}) greater than old (Id: {}, Score: {}). Replacing in heap.",
                        score.0, score.1, least_similar_score.0 .0, least_similar_score.0 .1
                    );
                    heap.pop();
                    heap.push(Reverse(score));
                }
            }
        }

        // Fetch the DataChunks matching the most similar embeddings
        let mut chunks: Vec<(DataChunk, f32)> = Vec::new();
        while let Some(Reverse((id, similarity))) = heap.pop() {
            println!("{}: {}%", id, similarity);
            let chunk = self.get_data_chunk(id)?; // Propagate the error if `get_data_chunk` fails
            chunks.push((chunk.clone(), similarity.into_inner()));
        }

        Ok(chunks)
    }
}

/// Represents a document resource with properties and operations related to
/// data chunks and embeddings.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentResource {
    pub name: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub resource_embedding: Embedding,
    chunk_embeddings: Vec<Embedding>,
    chunk_count: u64,
    data_chunks: Vec<DataChunk>,
}

impl Resource for DocumentResource {
    /// # Returns
    ///
    /// The name of the `DocumentResource`.
    fn name(&self) -> &str {
        &self.name
    }

    /// # Returns
    ///
    /// The optional description of the `DocumentResource`.
    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// # Returns
    ///
    /// The optional source of the `DocumentResource`.
    fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// # Returns
    ///
    /// The resource `Embedding` of the `DocumentResource`.
    fn resource_embedding(&self) -> &Embedding {
        &self.resource_embedding
    }

    /// # Returns
    ///
    /// The chunk `Embedding`s of the `DocumentResource`.
    fn chunk_embeddings(&self) -> &Vec<Embedding> {
        &self.chunk_embeddings
    }

    fn set_resource_embedding(&mut self, embedding: Embedding) {
        self.resource_embedding = embedding;
    }

    /// Retrieves a data chunk given its id.
    ///
    /// # Arguments
    ///
    /// * `id` - The `String` id of the data chunk.
    ///
    /// # Returns
    ///
    /// A reference to the `DataChunk` if found, or an error.
    fn get_data_chunk(&self, id: String) -> Result<&DataChunk, Box<dyn std::error::Error>> {
        let id = id.parse::<u64>().map_err(|_| "Chunk id must be a u64")?;
        if id > self.chunk_count {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid chunk id",
            )));
        }
        let index = (id - 1) as usize;
        Ok(&self.data_chunks[index])
    }
}

impl DocumentResource {
    // Constructors
    /// Creates a new instance of a `DocumentResource`.
    ///
    /// # Arguments
    ///
    /// * `name` - A string slice that holds the name of the document resource.
    /// * `desc` - An optional string slice that holds the description of the
    ///   document resource.
    /// * `source` - An optional string slice that holds the source of the
    ///   document resource.
    /// * `resource_embedding` - An `Embedding` struct that holds the embedding
    ///   of the document resource.
    /// * `chunk_embeddings` - A vector of `Embedding` structs that hold the
    ///   embeddings of the data chunks.
    /// * `data_chunks` - A vector of `DataChunk` structs that hold the data
    ///   chunks.
    ///
    /// # Returns
    ///
    /// * `Self` - A new instance of `DocumentResource`.
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
        resource_embedding: Embedding,
        chunk_embeddings: Vec<Embedding>,
        data_chunks: Vec<DataChunk>,
    ) -> Self {
        DocumentResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source.map(String::from),
            resource_embedding,
            chunk_embeddings,
            chunk_count: data_chunks.len() as u64,
            data_chunks: data_chunks,
        }
    }

    /// Initializes an empty `DocumentResource` with an empty resource
    /// embedding.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the `DocumentResource`.
    /// * `desc` - The optional description of the `DocumentResource`.
    /// * `source` - The optional source of the `DocumentResource`.
    ///
    /// # Returns
    ///
    /// * `Self` - A new instance of `DocumentResource`.
    pub fn new_empty(name: &str, desc: Option<&str>, source: Option<&str>) -> Self {
        DocumentResource::new(
            name,
            desc,
            source,
            Embedding::new(&String::new(), vec![]),
            Vec::new(),
            Vec::new(),
        )
    }

    /// Performs a vector similarity search using a query embedding, and then
    /// fetches a specific number of DataChunks below and above the most
    /// similar DataChunk.
    ///
    /// # Arguments
    ///
    /// * `query` - The query `Embedding`.
    /// * `proximity_window` - The number of DataChunks to fetch below and above
    ///   the most similar DataChunk.
    ///
    /// # Returns
    ///
    /// A vector of `DataChunk`s sorted by their ids, or an error.
    pub fn similarity_search_proximity(
        &self,
        query: Embedding,
        proximity_window: u64,
    ) -> Result<Vec<DataChunk>, Box<dyn std::error::Error>> {
        let search_results = self.similarity_search(query, 1)?;

        if search_results.is_empty() {
            return Err("No matching data chunks found".into());
        }

        let mut chunks: Vec<DataChunk> = Vec::new();
        let most_similar_chunk = search_results.first().unwrap(); // This is a safe unwrap
        let most_similar_id = most_similar_chunk.id.parse::<u64>()?;

        let start_id = if most_similar_id > proximity_window {
            most_similar_id - proximity_window
        } else {
            1
        };

        let end_id = most_similar_id + proximity_window;
        for id in start_id..=end_id {
            let chunk = self.get_data_chunk(id.to_string())?;
            chunks.push(chunk.clone());
        }

        Ok(chunks)
    }

    /// Performs a metadata search, returning all DataChunks with the same
    /// metadata.
    ///
    /// # Arguments
    ///
    /// * `query_metadata` - The metadata string to search for.
    ///
    /// # Returns
    ///
    /// A vector of `DataChunk`s with the same metadata, or an error.
    pub fn metadata_search(&self, query_metadata: &str) -> Result<Vec<DataChunk>, Box<dyn std::error::Error>> {
        let mut matching_chunks: Vec<DataChunk> = Vec::new();

        for chunk in &self.data_chunks {
            match &chunk.metadata {
                Some(metadata) if metadata == &query_metadata => matching_chunks.push(chunk.clone()),
                _ => (),
            }
        }

        if matching_chunks.is_empty() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No matching data chunks found",
            )));
        }

        Ok(matching_chunks)
    }

    /// Appends a new data chunk and associated embedding to the document
    /// resource.
    ///
    /// # Arguments
    ///
    /// * `data` - A string representing the data to be added in the new data
    ///   chunk.
    /// * `metadata` - An optional string representing additional metadata for
    ///   the data chunk.
    /// * `embedding` - An embedding related to the data chunk.
    ///
    /// The method creates a new data chunk using the provided data and
    /// metadata, clones the provided embedding and sets its id to match the
    /// new data chunk, and finally adds the new data chunk and the updated
    /// embedding to the resource.
    pub fn append_data(&mut self, data: &str, metadata: Option<&str>, embedding: &Embedding) {
        let id = self.chunk_count + 1;
        let data_chunk = DataChunk::new_with_integer_id(id, data, metadata.clone());
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        self.add_data_chunk(data_chunk);
        self.chunk_embeddings.push(embedding);
    }

    /// Replaces an existing data chunk and associated embedding in the
    /// resource.
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the data chunk to be replaced.
    /// * `new_data` - A string representing the new data.
    /// * `new_metadata` - An optional string representing the new metadata.
    /// * `embedding` - An embedding related to the new data chunk.
    ///
    /// # Returns
    ///
    /// * `Result<DataChunk, Box<dyn std::error::Error>>` - If successful,
    ///   returns the old `DataChunk` that was replaced.
    ///
    /// The method checks if the provided id is valid, and if so, it creates a
    /// new data chunk using the provided new data and metadata, clones the
    /// provided embedding and sets its id to match the new data chunk,
    /// replaces the old data chunk and the associated embedding with
    /// the new ones, and finally returns the old data chunk.
    pub fn replace_data(
        &mut self,
        id: u64,
        new_data: &str,
        new_metadata: Option<&str>,
        embedding: &Embedding,
    ) -> Result<DataChunk, Box<dyn Error>> {
        if id > self.chunk_count {
            return Err(Box::new(InvalidChunkIdError));
        }
        let index = (id - 1) as usize;
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        let old_chunk = std::mem::replace(
            &mut self.data_chunks[index],
            DataChunk::new_with_integer_id(id, &new_data, new_metadata),
        );
        self.chunk_embeddings[index] = embedding;
        Ok(old_chunk)
    }

    /// Removes and returns the last data chunk and associated embedding from
    /// the resource.
    ///
    /// # Returns
    ///
    /// * `Result<(DataChunk, Embedding), Box<dyn std::error::Error>>` - If
    ///   successful, returns a tuple containing the removed data chunk and
    ///   embedding. If the resource is empty, returns a `ResourceEmptyError`.
    ///
    /// The method attempts to pop the last `DataChunk` and `Embedding` from
    /// their respective vectors. If this is successful, it decrements
    /// `chunk_count` and returns the popped `DataChunk` and `Embedding`. If
    /// the resource is empty, it returns a `ResourceEmptyError`.
    pub fn pop_data(&mut self) -> Result<(DataChunk, Embedding), Box<dyn std::error::Error>> {
        let popped_chunk = self.data_chunks.pop();
        let popped_embedding = self.chunk_embeddings.pop();

        match (popped_chunk, popped_embedding) {
            (Some(chunk), Some(embedding)) => {
                self.chunk_count -= 1;
                Ok((chunk, embedding))
            }
            _ => Err(Box::new(ResourceEmptyError)),
        }
    }

    /// Deletes a data chunk and associated embedding from the resource.
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the data chunk to be deleted.
    ///
    /// # Returns
    ///
    /// A tuple containing the removed data chunk and embedding, or error.
    pub fn delete_data(&mut self, id: u64) -> Result<(DataChunk, Embedding), Box<dyn Error>> {
        let deleted_chunk = self.delete_data_chunk(id)?;

        let index = (id - 1) as usize;
        let deleted_embedding = self.chunk_embeddings.remove(index);

        // Adjust the ids of the remaining embeddings
        for i in index..self.chunk_embeddings.len() {
            self.chunk_embeddings[i].set_id_with_integer((i + 1) as u64);
        }

        Ok((deleted_chunk, deleted_embedding))
    }

    // Internal data chunk deletion
    fn delete_data_chunk(&mut self, id: u64) -> Result<DataChunk, Box<dyn Error>> {
        if id > self.chunk_count {
            return Err(Box::new(InvalidChunkIdError));
        }
        let index = (id - 1) as usize;
        let removed_chunk = self.data_chunks.remove(index);
        self.chunk_count -= 1;
        for chunk in self.data_chunks.iter_mut().skip(index) {
            let chunk_id: u64 = chunk.id.parse().unwrap();
            chunk.id = format!("{}", chunk_id - 1);
        }
        Ok(removed_chunk)
    }

    // Internal adding a data chunk
    fn add_data_chunk(&mut self, mut data_chunk: DataChunk) {
        self.chunk_count += 1;
        data_chunk.id = self.chunk_count.to_string();
        self.data_chunks.push(data_chunk);
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_document_resource_similarity_search() {
        // Prepare generator and doc resource
        let generator = EmbeddingGenerator::new_default();
        let mut doc = DocumentResource::new_empty(
            "3 Animal Facts",
            Some("A bunch of facts about animals and wildlife"),
            Some("animalwildlife.com"),
        );

        // Prepare embeddings + data, then add it to the doc
        let fact1 = "Dogs are creatures with 4 legs that bark.";
        let fact1_embeddings = generator.generate_embedding(fact1, "").unwrap();
        let fact2 = "Camels are slow animals with large humps.";
        let fact2_embeddings = generator.generate_embedding(fact2, "").unwrap();
        let fact3 = "Seals swim in the ocean.";
        let fact3_embeddings = generator.generate_embedding(fact3, "").unwrap();
        doc.append_data(fact1, None, &fact1_embeddings);
        doc.append_data(fact2, None, &fact2_embeddings);
        doc.append_data(fact3, None, &fact3_embeddings);

        // Testing similarity search works
        let query_string = "What animal barks?";
        let query_embedding = generator.generate_embedding(query_string, "").unwrap();
        let res = doc.similarity_search(query_embedding, 3).unwrap();
        assert_eq!(fact1, res[0].data);

        let query_string2 = "What animal is slow?";
        let query_embedding2 = generator.generate_embedding(query_string2, "").unwrap();
        let res2 = doc.similarity_search(query_embedding2, 3).unwrap();
        assert_eq!(fact2, res2[0].data);

        let query_string3 = "What animal swims in the ocean?";
        let query_embedding3 = generator.generate_embedding(query_string3, "").unwrap();
        let res3 = doc.similarity_search(query_embedding3, 3).unwrap();
        assert_eq!(fact3, res3[0].data);
    }
}
