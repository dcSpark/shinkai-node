use crate::resource_errors::VectorResourceError;
use reqwest::blocking::multipart as blocking_multipart;
use reqwest::multipart;
use serde::Deserialize;
use serde_json::Value as JsonValue;

#[derive(Debug)]
pub struct UnstructuredAPI {
    api_url: String,
}

impl UnstructuredAPI {
    pub fn new(api_url: String) -> Self {
        Self { api_url }
    }

    pub fn process_file_request_blocking(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<JsonValue, VectorResourceError> {
        let client = reqwest::blocking::Client::new();

        let part = blocking_multipart::Part::bytes(file_buffer)
            .file_name(file_name.to_string())
            .mime_str("application/octet-stream")?;

        let form = blocking_multipart::Form::new().part("files", part);

        let res = client
            .post(&self.api_url)
            .header("Accept", "application/json")
            .multipart(form)
            .send()?;

        let body = res.text()?;

        println!("{:?}", body);

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

        let res = client
            .post(&self.api_url)
            .header("Accept", "application/json")
            .multipart(form)
            .send()
            .await?;

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
#[serde(untagged)]
pub enum UnstructuredElement {
    Title(Title),
    NarrativeText(NarrativeText),
    UncategorizedText(UncategorizedText),
    ListItem(ListItem),
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
pub struct Metadata {
    pub filename: String,
    pub file_directory: Option<String>,
    pub last_modified: Option<String>,
    pub filetype: String,
    pub coordinates: Option<Vec<f32>>,
    pub page_number: Option<u32>,
    pub page_name: Option<String>,
    pub sent_from: Option<String>,
    pub sent_to: Option<String>,
    pub subject: Option<String>,
    pub attached_to_filename: Option<String>,
    pub header_footer_type: Option<String>,
    pub link_urls: Option<Vec<String>>,
    pub link_texts: Option<Vec<String>>,
    pub links: Option<Vec<Link>>,
    pub section: Option<String>,
    pub parent_id: Option<String>,
    pub category_depth: Option<u32>,
    pub text_as_html: Option<String>,
    pub languages: Option<Vec<String>>,
    pub emphasized_text_contents: Option<String>,
    pub emphasized_text_tags: Option<Vec<String>>,
    pub num_characters: Option<u32>,
    pub is_continuation: Option<bool>,
    pub detection_class_prob: Option<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
pub struct Link {
    text: String,
    url: String,
}
