use super::file_parser::ShinkaiFileParser;
use super::file_parser_types::GroupedText;
use super::html_content_parsing::extract_core_content;
use super::{unstructured_parser::UnstructuredParser, unstructured_types::UnstructuredElement};
use crate::embedding_generator::EmbeddingGenerator;
use crate::resource_errors::VRError;
use crate::source::{distribution, TextChunkingStrategy, VRSourceReference};
use crate::vector_resource::SourceFileType;
use crate::{data_tags::DataTag, vector_resource::BaseVectorResource};
use distribution::DistributionInfo;
#[cfg(feature = "native-http")]
use reqwest::{blocking::multipart as blocking_multipart, multipart};
#[cfg(feature = "native-http")]
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq)]
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

    pub fn new_default() -> Self {
        Self {
            api_url: format!("https://internal.shinkai.com/x-unstructured-api/"),
            api_key: None,
        }
    }

    /// String of the main endpoint url for processing files
    pub fn endpoint_url(&self) -> String {
        if self.api_url.ends_with('/') {
            format!("{}general/v0/general", self.api_url)
        } else {
            format!("{}/general/v0/general", self.api_url)
        }
    }

    /// Makes an async request to process a file in a buffer to Unstructured server,
    /// and then processing the returned results into a list of GroupedText
    pub async fn process_file_into_grouped_text(
        &self,
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
    ) -> Result<Vec<GroupedText>, VRError> {
        // Parse into Unstructured elements, and then into text_groups
        let elements = self.file_request(file_buffer, &file_name).await?;
        Ok(UnstructuredParser::hierarchical_group_elements_text(
            &elements,
            max_node_text_size,
        ))
    }

    /// Makes an blocking request to process a file in a buffer to Unstructured server,
    /// and then processing the returned results into a list of GroupedText
    pub fn process_file_into_grouped_text_blocking(
        &self,
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
    ) -> Result<Vec<GroupedText>, VRError> {
        // Parse into Unstructured elements, and then into text_groups
        let elements = self.file_request_blocking(file_buffer, &file_name)?;
        Ok(UnstructuredParser::hierarchical_group_elements_text(
            &elements,
            max_node_text_size,
        ))
    }

    /// Makes a blocking request to process a file in a buffer into a list of
    /// UnstructuredElements
    pub fn file_request_blocking(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<Vec<UnstructuredElement>, VRError> {
        let client = reqwest::blocking::Client::new();
        let file_buffer = extract_core_content(file_buffer, file_name);

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

    /// Makes an async request to process a file in a buffer into a list of UnstructuredElements
    pub async fn file_request(
        &self,
        mut file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<Vec<UnstructuredElement>, VRError> {
        let client = reqwest::Client::new();

        // First attempt with the original file_buffer
        let attempt = self.send_file_request(&client, &file_buffer, file_name).await;

        match attempt {
            Ok(elements) => Ok(elements),
            Err(_) => {
                // If failed, retry with the cleaned file_buffer
                let file_content_lossy = String::from_utf8_lossy(&file_buffer);
                let cleaned_content = clean_string_for_gb2312(&file_content_lossy);
                file_buffer = cleaned_content.into_bytes();

                self.send_file_request(&client, &file_buffer, file_name).await
            }
        }
    }

    /// Internal method that makes the actual file request
    async fn send_file_request(
        &self,
        client: &reqwest::Client,
        file_buffer: &[u8],
        file_name: &str,
    ) -> Result<Vec<UnstructuredElement>, VRError> {
        let file_buffer = extract_core_content(file_buffer.to_vec(), file_name);

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

/// Removes characters from a string that are not representable in 'gb2312'.
/// Encodes a string to 'gb2312' and decodes it back, effectively removing characters
/// not representable in 'gb2312'.
fn clean_string_for_gb2312(input: &str) -> String {
    // Encode the input string to 'gb2312'. Unsupported characters will be handled
    // according to the library's default behavior (likely replaced or ignored).
    let encoded = textcode::gb2312::encode_to_vec(input);

    // Prepare an empty String to hold the decoded output.
    let mut decoded = String::new();

    // Decode the 'gb2312' encoded bytes back into a String.
    // This step assumes that the encoding process has already filtered
    // out or replaced unsupported characters.
    textcode::gb2312::decode(&encoded, &mut decoded);

    decoded
}
