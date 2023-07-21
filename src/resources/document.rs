use crate::resources::embedding_generator::*;
use crate::resources::embeddings::*;
use crate::resources::file_parsing::*;
use crate::resources::model_type::*;
use crate::resources::resource::*;
use crate::resources::resource_errors::*;
use llm::load_progress_callback_stdout as load_callback;
use serde_json;
use std::fs::File;
use std::io::prelude::*;

// Impromptu function for testing local pdf parsing into resource document
pub fn local_pdf_to_doc() {
    // Load model and create a generator
    // let model_architecture = llm::ModelArchitecture::GptNeoX;
    // let model = llm::load_dynamic(
    //     Some(model_architecture),
    //     std::path::Path::new("pythia-160m-q4_0.bin"),
    //     llm::TokenizerSource::Embedded,
    //     Default::default(),
    //     load_callback,
    // )
    // .unwrap_or_else(|err| panic!("Failed to load model: {}", err));
    // let generator = LocalEmbeddingGenerator::new(model, model_architecture);
    let model_architecture = EmbeddingModelType::RemoteModel(RemoteModel::OpenAITextEmbeddingAda002);
    let generator = RemoteEmbeddingGenerator::new(model_architecture, "http://0.0.0.0:8080", None);

    // Read the pdf from file into a buffer, then parse it into a DocumentResource
    let desc = "Description of the pdf";
    let buffer = std::fs::read("mina.pdf")
        .map_err(|_| ResourceError::FailedPDFParsing)
        .unwrap();
    let doc = DocumentResource::parse_pdf(&buffer, &generator, "Mina Whitepaper", Some(desc), None).unwrap();

    // Convert the DocumentResource into json and save to file
    let json = doc.to_json().unwrap();
    let file_path = "mina_doc_resource.json";
    let mut file = std::fs::File::create(file_path).expect("Failed to create the file.");
    file.write_all(json.as_bytes()).expect("Failed to write JSON to file.");
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentResource {
    pub name: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub resource_embedding: Embedding,
    embedding_model_used: EmbeddingModelType,
    chunk_embeddings: Vec<Embedding>,
    chunk_count: u64,
    data_chunks: Vec<DataChunk>,
}

impl Resource for DocumentResource {
    /// # Returns
    ///
    /// The LLM model used to generate embeddings for this resource.
    fn embedding_model_used(&self) -> EmbeddingModelType {
        self.embedding_model_used.clone()
    }

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

    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType) {
        self.embedding_model_used = model_type;
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
    /// * `embedding_model_used` - The model used to generate the embeddings for
    ///   this resource
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
        embedding_model_used: EmbeddingModelType,
    ) -> Self {
        DocumentResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source.map(String::from),
            resource_embedding,
            chunk_embeddings,
            chunk_count: data_chunks.len() as u64,
            data_chunks: data_chunks,
            embedding_model_used,
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
            EmbeddingModelType::LocalModel(LocalModel::GptNeoX),
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
    ) -> Result<Vec<DataChunk>, ResourceError> {
        let search_results = self.similarity_search(query, 1);

        let most_similar_chunk = search_results.first().ok_or(ResourceError::ResourceEmpty)?; // If there's no first element, return an InvalidChunkId error

        let mut chunks: Vec<DataChunk> = Vec::new();
        let most_similar_id = most_similar_chunk
            .id
            .parse::<u64>()
            .map_err(|_| ResourceError::InvalidChunkId)?;

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
    pub fn metadata_search(&self, query_metadata: &str) -> Result<Vec<DataChunk>, ResourceError> {
        let mut matching_chunks: Vec<DataChunk> = Vec::new();

        for chunk in &self.data_chunks {
            match &chunk.metadata {
                Some(metadata) if metadata == &query_metadata => matching_chunks.push(chunk.clone()),
                _ => (),
            }
        }

        if matching_chunks.is_empty() {
            return Err(ResourceError::NoChunkFound);
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
    /// * `Result<DataChunk, ResourceError>` - If successful, returns the old
    ///   `DataChunk` that was replaced.
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
    ///
    /// # Returns
    ///
    /// * `Result<(DataChunk, Embedding), ResourceError>` - If successful,
    ///   returns a tuple containing the removed data chunk and embedding. If
    ///   the resource is empty, returns a `ResourceError`.
    ///
    /// The method attempts to pop the last `DataChunk` and `Embedding` from
    /// their respective vectors. If this is successful, it decrements
    /// `chunk_count` and returns the popped `DataChunk` and `Embedding`. If
    /// the resource is empty, it returns a `ResourceError`.
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
    ///
    /// # Arguments
    ///
    /// * `id` - The id of the data chunk to be deleted.
    ///
    /// # Returns
    ///
    /// A tuple containing the removed data chunk and embedding, or error.
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

    // Internal data chunk deletion
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

    // Internal adding a data chunk
    fn add_data_chunk(&mut self, mut data_chunk: DataChunk) {
        self.chunk_count += 1;
        data_chunk.id = self.chunk_count.to_string();
        self.data_chunks.push(data_chunk);
    }

    // Convert to json
    pub fn to_json(&self) -> Result<String, ResourceError> {
        serde_json::to_string(self).map_err(|_| ResourceError::FailedJSONParsing)
    }

    // Convert from json
    pub fn from_json(json: &str) -> Result<Self, ResourceError> {
        serde_json::from_str(json).map_err(|_| ResourceError::FailedJSONParsing)
    }

    /// Parse a PDF from a buffer into a Document Resource, automatically
    /// generating embeddings using the supplied embedding generator.
    ///
    /// # Arguments
    ///
    /// * `buffer` - A byte slice containing the PDF data.
    /// * `generator` - Any struct that implements `EmbeddingGenerator` trait.
    /// * `name` - The name of the document.
    /// * `desc` - An optional description of the document.
    /// * `source` - An optional source of the document.
    ///
    /// # Returns
    ///
    /// A `Result` containing a ResourceDocument. If
    /// an error occurs while parsing the PDF data, the `Result` will
    /// contain an `Error`.
    pub fn parse_pdf(
        buffer: &[u8],
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
    ) -> Result<DocumentResource, ResourceError> {
        // Create doc resource and initial setup
        let mut doc = DocumentResource::new_empty(name, desc, source);
        doc.set_embedding_model_used(generator.model_type());
        doc.update_resource_embedding(generator)?;
        println!("Generated resource embedding");

        // Parse the pdf into grouped text blocks
        let grouped_text_list = FileParser::parse_pdf(buffer)?;

        // Generate embeddings for each group of text
        let mut embeddings = Vec::new();
        let total_num_embeddings = grouped_text_list.len();
        let mut i = 0;
        for text in &grouped_text_list {
            let embedding = generator.generate_embedding_default(text)?;
            embeddings.push(embedding);

            i += 1;
            println!("Generated chunk embedding {}/{}", i, total_num_embeddings);
        }

        // Add the text + embeddings into the doc
        for (i, text) in grouped_text_list.iter().enumerate() {
            doc.append_data(text, None, &embeddings[i]);
        }

        Ok(doc)
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_document_resource_similarity_search() {
        // Prepare generator and doc resource
        let generator = LocalEmbeddingGenerator::new_default();
        let mut doc = DocumentResource::new_empty(
            "3 Animal Facts",
            Some("A bunch of facts about animals and wildlife"),
            Some("animalwildlife.com"),
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

        // Testing similarity search works
        let query_string = "What animal barks?";
        let query_embedding = generator.generate_embedding_default(query_string).unwrap();
        let res = doc.similarity_search(query_embedding, 1);
        assert_eq!(fact1, res[0].data);

        let query_string2 = "What animal is slow?";
        let query_embedding2 = generator.generate_embedding_default(query_string2).unwrap();
        let res2 = doc.similarity_search(query_embedding2, 3);
        assert_eq!(fact2, res2[0].data);

        let query_string3 = "What animal swims in the ocean?";
        let query_embedding3 = generator.generate_embedding_default(query_string3).unwrap();
        let res3 = doc.similarity_search(query_embedding3, 2);
        assert_eq!(fact3, res3[0].data);

        // Testing JSON serialization/deserialization
        let json = doc.to_json().unwrap();
        let deserialized_doc: DocumentResource = DocumentResource::from_json(&json).unwrap();
        assert_eq!(doc, deserialized_doc);
    }
}
