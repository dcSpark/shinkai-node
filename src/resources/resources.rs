use crate::resources::embeddings::*;

#[derive(Debug, Clone, PartialEq)]
pub struct DataChunk {
    pub id: String,
    pub data: String,
    pub metadata: Option<String>,
}
impl DataChunk {
    pub fn new(id: String, data: String, metadata: Option<String>) -> Self {
        Self { id, data, metadata }
    }

    pub fn new_with_integer_id(id: u64, data: String, metadata: Option<String>) -> Self {
        Self::new(id.to_string(), data, metadata)
    }
}

pub trait Resource {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn source(&self) -> Option<&str>;
    fn resource_embedding(&self) -> &Embedding;
    fn chunk_embeddings(&self) -> &Vec<Embedding>;

    // Method to retrieve data chunk
    fn get_data_chunk(&self, id: String) -> Result<&DataChunk, Box<dyn std::error::Error>>;

    fn similarity_search(
        &self,
        query: Embedding,
        num_of_results: u64,
    ) -> Result<Vec<DataChunk>, Box<dyn std::error::Error>> {
        let num_of_results = num_of_results as usize;

        // Calculate the cosine similarity between the query and each comparand, and
        // sort by similarity
        let mut similarities: Vec<(String, f32)> = self
            .chunk_embeddings()
            .iter()
            .map(|embedding| (embedding.id.clone(), query.cosine_similarity(embedding)))
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Take the top num_of_results results
        let top_results = similarities.into_iter().take(num_of_results);

        // Map ids to DataChunks
        let mut chunks: Vec<DataChunk> = Vec::new();
        for (id, _) in top_results {
            let chunk = self.get_data_chunk(id)?;
            chunks.push(chunk.clone());
        }

        Ok(chunks)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentResource {
    name: String,
    description: Option<String>,
    source: Option<String>,
    resource_embedding: Embedding,
    chunk_embeddings: Vec<Embedding>,
    chunk_count: u64,
    data_chunks: Vec<DataChunk>,
}

impl Resource for DocumentResource {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    fn resource_embedding(&self) -> &Embedding {
        &self.resource_embedding
    }

    fn chunk_embeddings(&self) -> &Vec<Embedding> {
        &self.chunk_embeddings
    }

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
    pub fn append_data(&mut self, data: String, metadata: Option<String>, embedding: &Embedding) {
        let id = self.chunk_count + 1;
        let data_chunk = DataChunk::new_with_integer_id(id, data.clone(), metadata.clone());
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
    /// * `Ok(DataChunk)` - If successful, returns the old data chunk that was
    ///   replaced.
    /// * `Err(&'static str)` - If the provided id is invalid, returns an error.
    ///
    /// The method checks if the provided id is valid, and if so, it creates a
    /// new data chunk using the provided new data and metadata, clones the
    /// provided embedding and sets its id to match the new data chunk,
    /// replaces the old data chunk and the associated embedding with
    /// the new ones, and finally returns the old data chunk.
    pub fn replace_data(
        &mut self,
        id: u64,
        new_data: String,
        new_metadata: Option<String>,
        embedding: &Embedding,
    ) -> Result<DataChunk, &'static str> {
        if id > self.chunk_count {
            return Err("Invalid chunk id");
        }
        let index = (id - 1) as usize;
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        let old_chunk = std::mem::replace(
            &mut self.data_chunks[index],
            DataChunk::new_with_integer_id(id, new_data.clone(), new_metadata.clone()),
        );
        self.chunk_embeddings[index] = embedding;
        Ok(old_chunk)
    }

    /// Removes and returns the last data chunk and associated embedding from
    /// the resource.
    ///
    /// # Returns
    ///
    /// A tuple containing the removed data chunk and embedding, or `None` if
    /// the resource is empty.
    pub fn pop_data(&mut self) -> Option<(DataChunk, Embedding)> {
        let popped_chunk = self.data_chunks.pop();
        let popped_embedding = self.chunk_embeddings.pop();

        match (popped_chunk, popped_embedding) {
            (Some(chunk), Some(embedding)) => {
                self.chunk_count -= 1;
                Some((chunk, embedding))
            }
            _ => None,
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
    /// A tuple containing the removed data chunk and embedding, or `None` if
    /// the id was invalid.
    pub fn delete_data(&mut self, id: u64) -> Result<(DataChunk, Embedding), &'static str> {
        let deleted_chunk_result = self.delete_data_chunk(id);

        if let Ok(deleted_chunk) = deleted_chunk_result {
            let index = (id - 1) as usize;
            let deleted_embedding = self.chunk_embeddings.remove(index);

            // Adjust the ids of the remaining embeddings
            for i in index..self.chunk_embeddings.len() {
                self.chunk_embeddings[i].set_id_with_integer((i + 1) as u64);
            }

            return Ok((deleted_chunk, deleted_embedding));
        }

        Err("Invalid chunk id")
    }

    fn add_data_chunk(&mut self, mut data_chunk: DataChunk) {
        self.chunk_count += 1;
        data_chunk.id = self.chunk_count.to_string();
        self.data_chunks.push(data_chunk);
    }

    fn delete_data_chunk(&mut self, id: u64) -> Result<DataChunk, &'static str> {
        if id > self.chunk_count {
            return Err("Invalid chunk id");
        }
        let index = (id - 1) as usize;
        let removed_chunk = self.data_chunks.remove(index);
        self.chunk_count -= 1;
        for chunk in self.data_chunks.iter_mut().skip(index) {
            let chunk_id: u64 = chunk.id.parse().expect("Chunk id must be a u64");
            chunk.id = format!("{}", chunk_id - 1);
        }
        Ok(removed_chunk)
    }
}
