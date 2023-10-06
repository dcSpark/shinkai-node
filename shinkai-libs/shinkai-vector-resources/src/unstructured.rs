use crate::base_vector_resources::BaseVectorResource;
use crate::data_tags::DataTag;
use crate::document_resource::DocumentVectorResource;
use crate::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::VectorResource;
use blake3::Hasher;
use keyphrases::KeyPhraseExtractor;
use reqwest::blocking::multipart as blocking_multipart;
use reqwest::multipart;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug)]
pub struct UnstructuredAPI {
    api_url: String,
    api_key: Option<String>,
}

impl UnstructuredAPI {
    pub fn new(api_url: String, api_key: Option<String>) -> Self {
        Self { api_url, api_key }
    }

    /// Makes a blocking request to process a file in a buffer to Unstructured,
    /// and then processing the returned results into a BaseVectorResource
    pub fn process_file_blocking(
        &self,
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // Parse pdf into groups of lines + a resource_id from the hash of the data
        let resource_id = UnstructuredParser::generate_data_hash(&file_buffer);
        let elements = self.process_file_request_blocking(file_buffer, name)?;
        eprintln!("Parsed file composed of {} elements", elements.len());

        // Create doc resource and initial setup
        let mut doc = DocumentVectorResource::new_empty(name, desc, source, &resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Extract keywords from the elements
        let keywords = UnstructuredParser::extract_keywords(&elements, 50);
        println!("Keywords: {:?}", keywords);

        // Set the resource embedding, using the keywords + name + desc + source
        doc.update_resource_embedding(generator, keywords)?;

        // Generate embeddings for each group of text
        let mut embeddings = Vec::new();
        let total_num_embeddings = elements.len();
        let mut i = 0;
        for element in &elements {
            let embedding = generator.generate_embedding_default(&element.text)?;
            embeddings.push(embedding);

            i += 1;
            println!("Generated chunk embedding {}/{}", i, total_num_embeddings);
        }

        // Add the text + embeddings into the doc
        for (i, element) in elements.iter().enumerate() {
            let mut metadata = HashMap::new();
            // Check if element.metadata.page_number exists, if so then add the value to "page_number" in the hashmap

            doc.append_data(&element.text, Some(metadata), &embeddings[i], parsing_tags);
        }

        Ok(BaseVectorResource::Document(doc))
    }

    /// Makes a blocking request to process a file in a buffer into a list of
    /// UnstructuredElements
    pub fn process_file_request_blocking(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<Vec<UnstructuredElement>, VectorResourceError> {
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

        let elements = UnstructuredParser::parse_response_json(json)?;
        Ok(elements)
    }

    /// Makes an async request to process a file in a buffer into a list of
    /// UnstructuredElements
    async fn process_file_request_async(
        &self,
        file_buffer: Vec<u8>,
        file_name: &str,
    ) -> Result<Vec<UnstructuredElement>, VectorResourceError> {
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

        let elements = UnstructuredParser::parse_response_json(json)?;
        Ok(elements)
    }
}

/// Struct which contains several methods related to parsing output from Unstructured
#[derive(Debug)]
pub struct UnstructuredParser;

impl UnstructuredParser {
    /// Parses the JSON Array response from Unstructured into a list of `UnstructuredElement`s
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

    /// Extracts the most important keywords from a given text,
    /// using the RAKE algorithm.
    pub fn extract_keywords(elements: &Vec<UnstructuredElement>, num: u64) -> Vec<String> {
        // Extract all the text out of all the elements and combine them together into a single string
        let text = elements
            .iter()
            .map(|element| element.text.as_str())
            .collect::<Vec<&str>>()
            .join(" ");

        // Create a new KeyPhraseExtractor with a maximum of num keywords
        let extractor = KeyPhraseExtractor::new(&text, num as usize);

        // Get the keywords
        let keywords = extractor.get_keywords();

        // Printing logic
        // keywords
        //     .iter()
        //     .for_each(|(score, keyword)| println!("{}: {}", keyword, score));

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    /// Generates a Blake3 hash of the data in the buffer
    pub fn generate_data_hash(buffer: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(buffer);
        let result = hasher.finalize();
        result.to_hex().to_string()
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub enum ElementType {
    Title,
    NarrativeText,
    UncategorizedText,
    ListItem,
}

#[derive(Debug, Deserialize)]
pub struct UnstructuredElement {
    #[serde(rename = "type")]
    pub element_type: ElementType,
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
