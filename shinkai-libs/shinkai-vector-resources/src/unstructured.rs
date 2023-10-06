use reqwest::blocking::multipart as blocking_multipart;
use reqwest::multipart;
use serde::Deserialize;
use serde_json::Error as SerdeError;
use serde_json::Value as JsonValue;

use crate::resource_errors::VectorResourceError;

#[derive(Debug)]
pub struct UnstructuredAPI {
    api_url: String,
}

impl UnstructuredAPI {
    pub fn new(api_url: String) -> Self {
        Self { api_url }
    }

    fn process_file_request_blocking(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<JsonValue, VectorResourceError> {
        let client = reqwest::blocking::Client::new();

        let part = blocking_multipart::Part::bytes(file_buffer)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")?;

        let form = blocking_multipart::Form::new().part("files", part);

        let res = client.post(&self.api_url).multipart(form).send()?;

        let body = res.text()?;

        let json: JsonValue = serde_json::from_str(&body)?;

        Ok(json)
    }

    async fn process_file_request_async(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<JsonValue, VectorResourceError> {
        let client = reqwest::Client::new();

        let part = multipart::Part::bytes(file_buffer)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")?;

        let form = multipart::Form::new().part("files", part);

        let res = client.post(&self.api_url).multipart(form).send().await?;

        let body = res.text().await?;

        let json: JsonValue = serde_json::from_str(&body)?;

        Ok(json)
    }
}

#[derive(Debug)]
pub struct UnstructuredResponseParser;

impl UnstructuredResponseParser {
    pub fn parse_response_json(json: JsonValue) -> Result<Vec<UnstructuredElement>, VectorResourceError> {
        if let JsonValue::Array(array) = json {
            let mut elements = Vec::new();
            for item in array {
                let element: UnstructuredElement = serde_json::from_value(item)
                    .map_err(|err| VectorResourceError::FailedParsingUnstructedAPIJSON(err.to_string()))?;
                elements.push(element);
            }
            Ok(elements)
        } else {
            Err(VectorResourceError::FailedParsingUnstructedAPIJSON(
                "Response is not an array at top level".to_string(),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub filename: String,
    pub filetype: String,
    pub page_number: u32,
    pub parent_id: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct Title {
    #[serde(rename = "type")]
    pub element_type: String,
    pub element_id: String,
    pub metadata: Metadata,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct NarrativeText {
    #[serde(rename = "type")]
    pub element_type: String,
    pub element_id: String,
    pub metadata: Metadata,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct UncategorizedText {
    #[serde(rename = "type")]
    pub element_type: String,
    pub element_id: String,
    pub metadata: Metadata,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ListItem {
    #[serde(rename = "type")]
    pub element_type: String,
    pub element_id: String,
    pub metadata: Metadata,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UnstructuredElement {
    Title(Title),
    NarrativeText(NarrativeText),
    UncategorizedText(UncategorizedText),
    ListItem(ListItem),
}
