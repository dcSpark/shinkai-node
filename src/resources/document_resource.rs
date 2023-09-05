use crate::resources::data_tags::{DataTag, DataTagIndex};
use crate::resources::embedding_generator::*;
use crate::resources::embeddings::*;
use crate::resources::file_parsing::*;
use crate::resources::model_type::*;
use crate::resources::resource_errors::*;
use crate::resources::vector_resource::*;
use serde_json;
use std::collections::HashMap;

use super::base_vector_resources::BaseVectorResource;

/// A VectorResource which uses an internal numbered/ordered list data model,  
/// thus providing an ideal interface for document-like content such as PDFs,
/// epubs, web content, written works, and more.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentVectorResource {
    name: String,
    description: Option<String>,
    source: Option<String>,
    resource_id: String,
    resource_embedding: Embedding,
    embedding_model_used: EmbeddingModelType,
    chunk_embeddings: Vec<Embedding>,
    chunk_count: u64,
    data_chunks: Vec<DataChunk>,
    data_tag_index: DataTagIndex,
}

impl VectorResource for DocumentVectorResource {
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

    fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    fn resource_id(&self) -> &str {
        &self.resource_id
    }

    fn resource_embedding(&self) -> &Embedding {
        &self.resource_embedding
    }

    fn resource_type(&self) -> VectorResourceType {
        VectorResourceType::Document
    }

    fn chunk_embeddings(&self) -> Vec<Embedding> {
        self.chunk_embeddings.clone()
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

    /// Efficiently retrieves a DataChunk's matching embedding given its id by fetching it via index.
    fn get_chunk_embedding(&self, id: String) -> Result<Embedding, VectorResourceError> {
        let id = id.parse::<u64>().map_err(|_| VectorResourceError::InvalidChunkId)?;
        if id == 0 || id > self.chunk_count {
            return Err(VectorResourceError::InvalidChunkId);
        }
        let index = id.checked_sub(1).ok_or(VectorResourceError::InvalidChunkId)? as usize;
        Ok(self.chunk_embeddings[index].clone())
    }

    /// Efficiently retrieves a data chunk given its id by fetching it via index.
    fn get_data_chunk(&self, id: String) -> Result<DataChunk, VectorResourceError> {
        let id = id.parse::<u64>().map_err(|_| VectorResourceError::InvalidChunkId)?;
        if id == 0 || id > self.chunk_count {
            return Err(VectorResourceError::InvalidChunkId);
        }
        let index = id.checked_sub(1).ok_or(VectorResourceError::InvalidChunkId)? as usize;
        Ok(self.data_chunks[index].clone())
    }
}

impl DocumentVectorResource {
    /// * `resource_id` - For DocumentVectorResources this should be a Sha256 hash as a String
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
        DocumentVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source.map(String::from),
            resource_id: String::from(resource_id),
            resource_embedding,
            chunk_embeddings,
            chunk_count: data_chunks.len() as u64,
            data_chunks: data_chunks,
            embedding_model_used,
            data_tag_index: DataTagIndex::new(),
        }
    }

    /// Initializes an empty `DocumentVectorResource` with empty defaults.
    pub fn new_empty(name: &str, desc: Option<&str>, source: Option<&str>, resource_id: &str) -> Self {
        DocumentVectorResource::new(
            name,
            desc,
            source,
            resource_id,
            Embedding::new(&String::new(), vec![]),
            Vec::new(),
            Vec::new(),
            EmbeddingModelType::RemoteModel(RemoteModel::AllMiniLML12v2),
        )
    }

    /// Performs a vector search using a query embedding, and then
    /// fetches a specific number of DataChunks below and above the most
    /// similar DataChunk.
    pub fn vector_search_proximity(
        &self,
        query: Embedding,
        proximity_window: u64,
    ) -> Result<Vec<RetrievedDataChunk>, VectorResourceError> {
        let search_results = self.vector_search(query, 1);
        let most_similar_chunk = search_results.first().ok_or(VectorResourceError::VectorResourceEmpty)?;
        let most_similar_id = most_similar_chunk
            .chunk
            .id
            .parse::<u64>()
            .map_err(|_| VectorResourceError::InvalidChunkId)?;

        // Get Start/End ids
        let start_id = if most_similar_id >= proximity_window {
            most_similar_id - proximity_window
        } else {
            1
        };
        let end_id = if let Some(end_boundary) = self.chunk_count.checked_sub(1) {
            if let Some(potential_end_id) = most_similar_id.checked_add(proximity_window) {
                potential_end_id.min(end_boundary)
            } else {
                end_boundary // Or any appropriate default
            }
        } else {
            1
        };

        // Acquire surrounding chunks
        let mut chunks = Vec::new();
        for id in start_id..=(end_id + 1) {
            if let Ok(chunk) = self.get_data_chunk(id.to_string()) {
                chunks.push(RetrievedDataChunk {
                    chunk: chunk.clone(),
                    score: 0.00,
                    resource_pointer: self.get_resource_pointer(),
                });
            }
        }

        Ok(chunks)
    }

    /// Returns all DataChunks with a matching key/value pair in the metadata hashmap
    pub fn metadata_search(
        &self,
        metadata_key: &str,
        metadata_value: &str,
    ) -> Result<Vec<RetrievedDataChunk>, VectorResourceError> {
        let mut matching_chunks = Vec::new();

        for chunk in &self.data_chunks {
            match &chunk.metadata {
                Some(metadata) if metadata.get(metadata_key) == Some(&metadata_value.to_string()) => matching_chunks
                    .push(RetrievedDataChunk {
                        chunk: chunk.clone(),
                        score: 0.00,
                        resource_pointer: self.get_resource_pointer(),
                    }),
                _ => (),
            }
        }

        if matching_chunks.is_empty() {
            return Err(VectorResourceError::NoChunkFound);
        }

        Ok(matching_chunks)
    }

    /// Appends a new data chunk (with a BaseVectorResource) to the document
    /// and updates the data tags index. Of note, we use the resource's data tags
    /// and resource embedding.
    pub fn append_vector_resource(&mut self, resource: BaseVectorResource, metadata: Option<HashMap<String, String>>) {
        let embedding = resource.trait_object().resource_embedding().clone();
        let tag_names = resource.trait_object().data_tag_index().data_tag_names();
        self._append_data_without_tag_validation(DataContent::Resource(resource), metadata, &embedding, &tag_names)
    }

    /// Appends a new data chunk (with a data_string) and an associated embedding to the document
    /// and updates the data tags index.
    pub fn append_data(
        &mut self,
        data_string: &str,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) {
        let validated_data_tags = DataTag::validate_tag_list(data_string, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._append_data_without_tag_validation(
            DataContent::Data(data_string.to_string()),
            metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Appends a new data chunk and associated embedding to the document
    /// without checking if tags are valid. Used for internal purposes/the routing resource.
    pub fn _append_data_without_tag_validation(
        &mut self,
        data: DataContent,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        tag_names: &Vec<String>,
    ) {
        let id = self.chunk_count + 1;
        let data_chunk = match data {
            DataContent::Data(data_string) => {
                DataChunk::new_with_integer_id(id, &data_string, metadata.clone(), tag_names)
            }
            DataContent::Resource(resource) => {
                DataChunk::new_vector_resource_with_integer_id(id, &resource, metadata.clone())
            }
        };
        self.data_tag_index.add_chunk(&data_chunk);

        // Embedding details
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        self.append_data_chunk(data_chunk);
        self.chunk_embeddings.push(embedding);
    }

    /// Replaces an existing data chunk and associated embedding in the Document resource
    /// with a BaseVectorResource in the new DataChunk, and updates the data tags index.
    pub fn replace_vector_resource(
        &mut self,
        id: u64,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
    ) -> Result<DataChunk, VectorResourceError> {
        let embedding = new_resource.trait_object().resource_embedding().clone();
        let tag_names = new_resource.trait_object().data_tag_index().data_tag_names();
        self._replace_data_without_tag_validation(
            id,
            DataContent::Resource(new_resource),
            new_metadata,
            &embedding,
            &tag_names,
        )
    }

    /// Replaces an existing data chunk & associated embedding and updates the data tags index.
    /// * `id` - The id of the data chunk to be replaced.
    pub fn replace_data(
        &mut self,
        id: u64,
        new_data: &str,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the new data with
    ) -> Result<DataChunk, VectorResourceError> {
        // Validate which tags will be saved with the new data
        let validated_data_tags = DataTag::validate_tag_list(new_data, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._replace_data_without_tag_validation(
            id,
            DataContent::Data(new_data.to_string()),
            new_metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Pops and returns the last data chunk and associated embedding
    /// and updates the data tags index.
    pub fn pop_data(&mut self) -> Result<(DataChunk, Embedding), VectorResourceError> {
        let popped_chunk = self.data_chunks.pop();
        let popped_embedding = self.chunk_embeddings.pop();

        match (popped_chunk, popped_embedding) {
            (Some(chunk), Some(embedding)) => {
                // Remove chunk from data tag index
                self.data_tag_index.remove_chunk(&chunk);
                self.chunk_count -= 1;
                Ok((chunk, embedding))
            }
            _ => Err(VectorResourceError::VectorResourceEmpty),
        }
    }

    /// Replaces an existing data chunk & associated embedding in the Document resource
    /// without checking if tags are valid. Used for resource router.
    pub fn _replace_data_without_tag_validation(
        &mut self,
        id: u64,
        new_data: DataContent,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        new_tag_names: &Vec<String>,
    ) -> Result<DataChunk, VectorResourceError> {
        // Id + index
        if id > self.chunk_count {
            return Err(VectorResourceError::InvalidChunkId);
        }
        let index = (id - 1) as usize;

        // Next create the new chunk, and replace the old chunk in the data_chunks list
        let new_chunk = match new_data {
            DataContent::Data(data_string) => {
                DataChunk::new_with_integer_id(id, &data_string, new_metadata.clone(), new_tag_names)
            }
            DataContent::Resource(resource) => {
                DataChunk::new_vector_resource_with_integer_id(id, &resource, new_metadata.clone())
            }
        };
        let old_chunk = std::mem::replace(&mut self.data_chunks[index], new_chunk.clone());

        // Then deletion of old chunk from index and addition of new chunk
        self.data_tag_index.remove_chunk(&old_chunk);
        self.data_tag_index.add_chunk(&new_chunk);

        // Finally replacing the embedding
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        self.chunk_embeddings[index] = embedding;

        Ok(old_chunk)
    }

    /// Deletes a data chunk and associated embedding from the resource
    /// and updates the data tags index.
    pub fn delete_data(&mut self, id: u64) -> Result<(DataChunk, Embedding), VectorResourceError> {
        let deleted_chunk = self.delete_data_chunk(id)?;
        self.data_tag_index.remove_chunk(&deleted_chunk);

        let index = (id - 1) as usize;
        let deleted_embedding = self.chunk_embeddings.remove(index);

        // Adjust the ids of the remaining embeddings
        for i in index..self.chunk_embeddings.len() {
            self.chunk_embeddings[i].set_id_with_integer((i + 1) as u64);
        }

        Ok((deleted_chunk, deleted_embedding))
    }

    /// Internal data chunk deletion
    fn delete_data_chunk(&mut self, id: u64) -> Result<DataChunk, VectorResourceError> {
        if id > self.chunk_count {
            return Err(VectorResourceError::InvalidChunkId);
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

    fn append_data_chunk(&mut self, mut data_chunk: DataChunk) {
        self.chunk_count += 1;
        data_chunk.id = self.chunk_count.to_string();
        self.data_chunks.push(data_chunk);
    }

    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        serde_json::from_str(json).map_err(|_| VectorResourceError::FailedJSONParsing)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
    }

    /// Parses a list of strings filled with text into a Document VectorResource,
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
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse all text with
    ) -> Result<DocumentVectorResource, VectorResourceError> {
        // Create doc resource and initial setup
        let mut doc = DocumentVectorResource::new_empty(name, desc, source, resource_id);
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
            let embedding = generator.generate_embedding(text)?;
            embeddings.push(embedding);

            i += 1;
            // println!("Generated chunk embedding {}/{}", i, total_num_embeddings);
        }

        // Add the text + embeddings into the doc
        for (i, text) in text_list.iter().enumerate() {
            doc.append_data(text, None, &embeddings[i], parsing_tags);
        }

        Ok(doc)
    }

    /// Parses a PDF from a buffer into a Document VectorResource, automatically
    /// separating sentences + performing text parsing, as well as
    /// generating embeddings using the supplied embedding generator.
    pub fn parse_pdf(
        buffer: &[u8],
        average_chunk_size: u64,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: Option<&str>,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse all text with
    ) -> Result<DocumentVectorResource, VectorResourceError> {
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let grouped_text_list = FileParser::parse_pdf(buffer, average_chunk_size)?;
        let resource_id = FileParser::generate_data_hash(buffer);
        DocumentVectorResource::parse_text(
            grouped_text_list,
            generator,
            name,
            desc,
            source,
            &resource_id,
            parsing_tags,
        )
    }
}
