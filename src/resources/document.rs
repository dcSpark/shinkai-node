use crate::resources::embedding_generator::*;
use crate::resources::embeddings::*;
use crate::resources::file_parsing::*;
use crate::resources::model_type::*;
use crate::resources::resource::*;
use crate::resources::resource_errors::*;
use serde_json;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentResource {
    name: String,
    description: Option<String>,
    source: Option<String>,
    resource_id: String,
    resource_embedding: Embedding,
    embedding_model_used: EmbeddingModelType,
    chunk_embeddings: Vec<Embedding>,
    chunk_count: u64,
    data_chunks: Vec<DataChunk>,
}

impl Resource for DocumentResource {
    fn embedding_model_used(&self) -> EmbeddingModelType {
        self.embedding_model_used.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    fn resource_id(&self) -> &str {
        &self.resource_id
    }

    fn resource_embedding(&self) -> &Embedding {
        &self.resource_embedding
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Document
    }

    fn chunk_embeddings(&self) -> &Vec<Embedding> {
        &self.chunk_embeddings
    }

    fn to_json(&self) -> Result<String, ResourceError> {
        serde_json::to_string(self).map_err(|_| ResourceError::FailedJSONParsing)
    }

    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType) {
        self.embedding_model_used = model_type;
    }

    fn set_resource_embedding(&mut self, embedding: Embedding) {
        self.resource_embedding = embedding;
    }

    /// Retrieves a data chunk given its id.
    fn get_data_chunk(&self, id: String) -> Result<&DataChunk, ResourceError> {
        let id = id.parse::<u64>().map_err(|_| ResourceError::InvalidChunkId)?;
        if id > self.chunk_count {
            return Err(ResourceError::InvalidChunkId);
        }
        let index = (id - 1) as usize;
        Ok(&self.data_chunks[index])
    }
}

impl DocumentResource {
    /// * `resource_id` - For DocumentResources this should be a Sha256 hash as a String
    ///  from the bytes of the original data.
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
        resource_id: &str,
        resource_embedding: Embedding,
        chunk_embeddings: Vec<Embedding>,
        data_chunks: Vec<DataChunk>,
        embedding_model_used: EmbeddingModelType,
    ) -> Self {
        DocumentResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source.map(String::from),
            resource_id: String::from(resource_id),
            resource_embedding,
            chunk_embeddings,
            chunk_count: data_chunks.len() as u64,
            data_chunks: data_chunks,
            embedding_model_used,
        }
    }

    /// Initializes an empty `DocumentResource` with empty defaults.
    pub fn new_empty(name: &str, desc: Option<&str>, source: Option<&str>, resource_id: &str) -> Self {
        DocumentResource::new(
            name,
            desc,
            source,
            resource_id,
            Embedding::new(&String::new(), vec![]),
            Vec::new(),
            Vec::new(),
            EmbeddingModelType::LocalModel(LocalModel::GptNeoX),
        )
    }

    /// Performs a vector similarity search using a query embedding, and then
    /// fetches a specific number of DataChunks below and above the most
    /// similar DataChunk.
    pub fn similarity_search_proximity(
        &self,
        query: Embedding,
        proximity_window: u64,
    ) -> Result<Vec<RetrievedDataChunk>, ResourceError> {
        let search_results = self.similarity_search(query, 1);

        let most_similar_chunk = search_results.first().ok_or(ResourceError::ResourceEmpty)?;

        let most_similar_id = most_similar_chunk
            .chunk
            .id
            .parse::<u64>()
            .map_err(|_| ResourceError::InvalidChunkId)?;

        let start_id = if most_similar_id > proximity_window {
            most_similar_id - proximity_window
        } else {
            1
        };

        let mut chunks = Vec::new();
        let end_id = most_similar_id + proximity_window;
        for id in start_id..=end_id {
            let chunk = self.get_data_chunk(id.to_string())?;
            chunks.push(RetrievedDataChunk {
                chunk: chunk.clone(),
                score: 0.00,
                resource_id: self.resource_id().to_string(),
            });
        }

        Ok(chunks)
    }

    /// Returns all DataChunks with the same metadata.
    pub fn metadata_search(&self, query_metadata: &str) -> Result<Vec<RetrievedDataChunk>, ResourceError> {
        let mut matching_chunks = Vec::new();

        for chunk in &self.data_chunks {
            match &chunk.metadata {
                Some(metadata) if metadata == &query_metadata => matching_chunks.push(RetrievedDataChunk {
                    chunk: chunk.clone(),
                    score: 0.00,
                    resource_id: self.resource_id().to_string(),
                }),
                _ => (),
            }
        }

        if matching_chunks.is_empty() {
            return Err(ResourceError::NoChunkFound);
        }

        Ok(matching_chunks)
    }

    /// Appends a new data chunk and associated embedding to the document.
    pub fn append_data(&mut self, data: &str, metadata: Option<&str>, embedding: &Embedding) {
        let id = self.chunk_count + 1;
        let data_chunk = DataChunk::new_with_integer_id(id, data, metadata.clone());
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        self.add_data_chunk(data_chunk);
        self.chunk_embeddings.push(embedding);
    }

    /// Replaces an existing data chunk and associated embedding.
    /// * `id` - The id of the data chunk to be replaced.
    pub fn replace_data(
        &mut self,
        id: u64,
        new_data: &str,
        new_metadata: Option<&str>,
        embedding: &Embedding,
    ) -> Result<DataChunk, ResourceError> {
        if id > self.chunk_count {
            return Err(ResourceError::InvalidChunkId);
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
    pub fn pop_data(&mut self) -> Result<(DataChunk, Embedding), ResourceError> {
        let popped_chunk = self.data_chunks.pop();
        let popped_embedding = self.chunk_embeddings.pop();

        match (popped_chunk, popped_embedding) {
            (Some(chunk), Some(embedding)) => {
                self.chunk_count -= 1;
                Ok((chunk, embedding))
            }
            _ => Err(ResourceError::ResourceEmpty),
        }
    }

    /// Deletes a data chunk and associated embedding from the resource.
    /// Returns a tuple containing the removed data chunk and embedding, or error.
    pub fn delete_data(&mut self, id: u64) -> Result<(DataChunk, Embedding), ResourceError> {
        let deleted_chunk = self.delete_data_chunk(id)?;

        let index = (id - 1) as usize;
        let deleted_embedding = self.chunk_embeddings.remove(index);

        // Adjust the ids of the remaining embeddings
        for i in index..self.chunk_embeddings.len() {
            self.chunk_embeddings[i].set_id_with_integer((i + 1) as u64);
        }

        Ok((deleted_chunk, deleted_embedding))
    }

    /// Internal data chunk deletion
    fn delete_data_chunk(&mut self, id: u64) -> Result<DataChunk, ResourceError> {
        if id > self.chunk_count {
            return Err(ResourceError::InvalidChunkId);
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

    fn add_data_chunk(&mut self, mut data_chunk: DataChunk) {
        self.chunk_count += 1;
        data_chunk.id = self.chunk_count.to_string();
        self.data_chunks.push(data_chunk);
    }

    pub fn from_json(json: &str) -> Result<Self, ResourceError> {
        serde_json::from_str(json).map_err(|_| ResourceError::FailedJSONParsing)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
    }

    /// Parses a list of strings filled with text into a Document Resource,
    /// extracting keywords, and generating embeddings using the supplied
    /// embedding generator.
    ///
    /// Of note, this function assumes you already pre-parsed the text,
    /// performed cleanup, ensured that each String is under the 512 token
    /// limit and is ready to be used to create a DataChunk.
    pub fn parse_text(
        text_list: Vec<String>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
        resource_id: &str,
    ) -> Result<DocumentResource, ResourceError> {
        // Create doc resource and initial setup
        let mut doc = DocumentResource::new_empty(name, desc, source, resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Parse the pdf into grouped text blocks
        let keywords = FileParser::extract_keywords(&text_list.join(" "), 50);

        // Set the resource embedding, using the keywords + name + desc + source
        doc.update_resource_embedding(generator, keywords)?;
        // println!("Generated resource embedding");

        // Generate embeddings for each group of text
        let mut embeddings = Vec::new();
        let total_num_embeddings = text_list.len();
        let mut i = 0;
        for text in &text_list {
            let embedding = generator.generate_embedding_default(text)?;
            embeddings.push(embedding);

            i += 1;
            // println!("Generated chunk embedding {}/{}", i, total_num_embeddings);
        }

        // Add the text + embeddings into the doc
        for (i, text) in text_list.iter().enumerate() {
            doc.append_data(text, None, &embeddings[i]);
        }

        Ok(doc)
    }

    /// Parses a PDF from a buffer into a Document Resource, automatically
    /// separating sentences + performing text parsing, as well as
    /// generating embeddings using the supplied embedding generator.
    pub fn parse_pdf(
        buffer: &[u8],
        average_chunk_size: u64,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
    ) -> Result<DocumentResource, ResourceError> {
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let grouped_text_list = FileParser::parse_pdf(buffer, average_chunk_size)?;
        let resource_id = FileParser::generate_data_hash(buffer);
        DocumentResource::parse_text(grouped_text_list, generator, name, desc, source, &resource_id)
    }
}

mod tests {
    use super::*;
    use crate::resources::bert_cpp::BertCPPProcess;

    #[test]
    fn test_manual_document_resource() {
        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();

        let mut doc = DocumentResource::new_empty(
            "3 Animal Facts",
            Some("A bunch of facts about animals and wildlife"),
            Some("animalwildlife.com"),
            "animal_resource",
        );

        doc.set_embedding_model_used(generator.model_type()); // Not required, but good practice

        // Prepare embeddings + data, then add it to the doc
        let fact1 = "Dogs are creatures with 4 legs that bark.";
        let fact1_embeddings = generator.generate_embedding_default(fact1).unwrap();
        let fact2 = "Camels are slow animals with large humps.";
        let fact2_embeddings = generator.generate_embedding_default(fact2).unwrap();
        let fact3 = "Seals swim in the ocean.";
        let fact3_embeddings = generator.generate_embedding_default(fact3).unwrap();
        doc.append_data(fact1, None, &fact1_embeddings);
        doc.append_data(fact2, None, &fact2_embeddings);
        doc.append_data(fact3, None, &fact3_embeddings);

        // Testing JSON serialization/deserialization
        let json = doc.to_json().unwrap();
        let deserialized_doc: DocumentResource = DocumentResource::from_json(&json).unwrap();
        assert_eq!(doc, deserialized_doc);

        // Testing similarity search works
        let query_string = "What animal barks?";
        let query_embedding = generator.generate_embedding_default(query_string).unwrap();
        let res = doc.similarity_search(query_embedding, 1);
        assert_eq!(fact1, res[0].chunk.data);

        let query_string2 = "What animal is slow?";
        let query_embedding2 = generator.generate_embedding_default(query_string2).unwrap();
        let res2 = doc.similarity_search(query_embedding2, 3);
        assert_eq!(fact2, res2[0].chunk.data);

        let query_string3 = "What animal swims in the ocean?";
        let query_embedding3 = generator.generate_embedding_default(query_string3).unwrap();
        let res3 = doc.similarity_search(query_embedding3, 2);
        assert_eq!(fact3, res3[0].chunk.data);
    }

    #[test]
    fn test_pdf_parsed_document_resource() {
        let bert_process = BertCPPProcess::start(); // Gets killed if out of scope
        let generator = RemoteEmbeddingGenerator::new_default();

        // Read the pdf from file into a buffer
        let buffer = std::fs::read("files/shinkai_manifesto.pdf")
            .map_err(|_| ResourceError::FailedPDFParsing)
            .unwrap();

        // Generate DocumentResource
        let desc = "An initial manifesto of the Shinkai Network.";
        let doc = DocumentResource::parse_pdf(
            &buffer,
            100,
            &generator,
            "Shinkai Manifesto",
            Some(desc),
            Some("http://shinkai.com"),
        )
        .unwrap();

        // Testing JSON serialization/deserialization
        let json = doc.to_json().unwrap();
        let deserialized_doc: DocumentResource = DocumentResource::from_json(&json).unwrap();
        assert_eq!(doc, deserialized_doc);

        // Testing similarity search works
        let query_string = "Who is building Shinkai?";
        let query_embedding = generator.generate_embedding_default(query_string).unwrap();
        let res = doc.similarity_search(query_embedding, 1);
        assert_eq!(
            "Shinkai Network Manifesto (Early Preview) Robert Kornacki rob@shinkai. com Nicolas Arqueros nico@shinkai.",
            res[0].chunk.data
        );

        let query_string = "What about up-front costs?";
        let query_embedding = generator.generate_embedding_default(query_string).unwrap();
        let res = doc.similarity_search(query_embedding, 1);
        assert_eq!(
            "No longer will we need heavy up front costs to build apps that allow users to use their money/data to interact with others in an extremely limited experience (while also taking away control from the user), but instead we will build the underlying architecture which unlocks the ability for the user s various AI agents to go about performing everything they need done and connecting all of their devices/data together.",
            res[0].chunk.data
        );

        let query_string = "Does this relate to crypto?";
        let query_embedding = generator.generate_embedding_default(query_string).unwrap();
        let res = doc.similarity_search(query_embedding, 1);
        assert_eq!(
            "With lessons derived from the P2P nature of blockchains, we in fact have all of the core primitives at hand to build a new AI coordinated computing paradigm that takes decentralization and user privacy seriously while offering native integration into the modern crypto stack.",
            res[0].chunk.data
        );
    }
}
