use super::{unstructured_parser::UnstructuredParser, unstructured_types::UnstructuredElement};
use crate::base_vector_resources::BaseVectorResource;
use crate::data_tags::DataTag;
use crate::embedding_generator::EmbeddingGenerator;
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
#[cfg(feature = "native-http")]
use reqwest::{blocking::multipart as blocking_multipart, multipart};
use serde_json::Value as JsonValue;

#[derive(Debug)]
#[cfg(feature = "native-http")]
pub struct UnstructuredAPI {
    api_url: String,
    api_key: Option<String>,
}

#[cfg(feature = "native-http")]
impl UnstructuredAPI {
    pub fn new(api_url: String, api_key: Option<String>) -> Self {
        Self { api_url, api_key }
    }

    /// String of the main endpoint url for processing files
    fn endpoint_url(&self) -> String {
        if self.api_url.ends_with('/') {
            format!("{}x-unstructured-api/general/v0/general", self.api_url)
        } else {
            format!("{}/x-unstructured-api/general/v0/general", self.api_url)
        }
    }

    /// Makes a blocking request to process a file in a buffer to Unstructured server,
    /// and then processing the returned results into a BaseVectorResource
    /// Note: Requires name to include the extension ie. `*.pdf`
    pub fn process_file(
        &self,
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let resource_id = UnstructuredParser::generate_data_hash(&file_buffer);
        let elements = self.file_request_blocking(file_buffer, &name)?;

        UnstructuredParser::process_elements_into_resource(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            resource_id,
            max_chunk_size,
        )
    }

    /// Makes an async request to process a file in a buffer to Unstructured server,
    /// and then processing the returned results into a BaseVectorResource
    /// Note: Requires name to include the extension ie. `*.pdf`
    pub async fn process_file_async(
        &self,
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let resource_id = UnstructuredParser::generate_data_hash(&file_buffer);
        let elements = self.file_request_async(file_buffer, &name).await?;

        UnstructuredParser::process_elements_into_resource(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            resource_id,
            max_chunk_size,
        )
    }

    /// Makes a blocking request to process a file in a buffer into a list of
    /// UnstructuredElements
    pub fn file_request_blocking(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<Vec<UnstructuredElement>, VectorResourceError> {
        let client = reqwest::blocking::Client::new();

        let part = blocking_multipart::Part::bytes(file_buffer)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")?;

        let form = blocking_multipart::Form::new().part("files", part);

        let mut request_builder = client
            .post(&self.endpoint_url())
            .header("Accept", "application/json")
            .multipart(form);

        if let Some(api_key) = &self.api_key {
            request_builder = request_builder.header("unstructured-api-key", api_key);
        }

        let res = request_builder.send()?;

        let body = res.text()?;

        let json: JsonValue = serde_json::from_str(&body)?;

        let elements = UnstructuredParser::parse_response_json(json)?;
        Ok(elements)
    }

    /// Makes an async request to process a file in a buffer into a list of
    /// UnstructuredElements
    pub async fn file_request_async(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<Vec<UnstructuredElement>, VectorResourceError> {
        let client = reqwest::Client::new();

        let part = multipart::Part::bytes(file_buffer)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")?;

        let form = multipart::Form::new().part("files", part);

        let mut request_builder = client
            .post(&self.endpoint_url())
            .header("Accept", "application/json")
            .multipart(form);

        if let Some(api_key) = &self.api_key {
            request_builder = request_builder.header("unstructured-api-key", api_key);
        }

        let res = request_builder.send().await?;

        let body = res.text().await?;

        let json: JsonValue = serde_json::from_str(&body)?;

        let elements = UnstructuredParser::parse_response_json(json)?;
        Ok(elements)
    }
}
