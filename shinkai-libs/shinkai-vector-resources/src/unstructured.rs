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

    /// String of the main endpoint url for processing files
    fn endpoint_url(&self) -> String {
        if self.api_url.ends_with('/') {
            format!("{}x-unstructured-api/general/v0/general", self.api_url)
        } else {
            format!("{}/x-unstructured-api/general/v0/general", self.api_url)
        }
    }

    /// Makes a blocking request to process a file in a buffer to Unstructured,
    /// and then processing the returned results into a BaseVectorResource
    /// Note: For the time being the file name must include the extension ie. `*.pdf`
    pub fn process_file(
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
        let elements = self.file_request_blocking(file_buffer, name)?;

        self.process_file_shared(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            &resource_id,
            max_chunk_size,
        )
    }

    /// Makes an async request to process a file in a buffer to Unstructured,
    /// and then processing the returned results into a BaseVectorResource
    /// Note: For the time being the file name must include the extension ie. `*.pdf`
    pub async fn process_file_async(
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
        let elements = self.file_request_async(file_buffer, name).await?;

        self.process_file_shared(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            &resource_id,
            max_chunk_size,
        )
    }

    /// Shared code between the blocking and async versions of the process_file method
    fn process_file_shared(
        &self,
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: &str,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // If description is None, find the first Title element and use its text as the description
        let mut resource_desc = desc;
        if desc.is_none() {
            if let Some(title_element) = elements
                .iter()
                .find(|&element| matches!(element.element_type, ElementType::Title))
            {
                resource_desc = Some(&title_element.text);
            }
            // If no title available, then use the first narrative text
            if resource_desc.is_none() {
                if let Some(element) = elements
                    .iter()
                    .find(|&element| matches!(element.element_type, ElementType::NarrativeText))
                {
                    resource_desc = Some(&element.text);
                }
            }
        }

        // Create doc resource and initial setup
        let mut doc = DocumentVectorResource::new_empty(name, resource_desc, source, &resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Extract keywords from the elements
        let keywords = UnstructuredParser::extract_keywords(&elements, 50);

        // Set the resource embedding, using the keywords + name + desc + source
        doc.update_resource_embedding(generator, keywords)?;

        // Group elements together into ready-to-use strings for embedding generation
        let text_groups = UnstructuredParser::group_elements_text(&elements, max_chunk_size);

        // Generate embeddings for each group of text
        let mut embeddings = Vec::new();
        let total_num_embeddings = text_groups.len();
        let mut i = 0;
        for grouped_text in &text_groups {
            println!(
                "Text: {}\n Page Numbers: {:?}\n\n",
                grouped_text.text, grouped_text.page_numbers
            );

            let embedding = generator.generate_embedding_default(&grouped_text.text)?;
            embeddings.push(embedding);

            i += 1;
            println!("Generated chunk embedding {}/{}", i, total_num_embeddings);
        }

        // Adds the text + embeddings into the doc as appended new DataChunks
        for (i, grouped_text) in text_groups.iter().enumerate() {
            // Add page numbers to metadata
            let mut metadata = HashMap::new();
            if !grouped_text.page_numbers.is_empty() {
                metadata.insert("page_numbers".to_string(), grouped_text.format_page_num_string());
            }

            doc.append_data(&grouped_text.text, Some(metadata), &embeddings[i], parsing_tags);
        }

        Ok(BaseVectorResource::Document(doc))
    }

    /// Makes a blocking request to process a file in a buffer into a list of
    /// UnstructuredElements
    fn file_request_blocking(
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
    async fn file_request_async(
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

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    /// Given a list of `UnstructuredElement`s, groups their text together with some processing logic.
    /// Currently respects max_chunk_size, ensures splitting between narrative text/new title,
    /// and skips over all uncategorized text.
    pub fn group_elements_text(elements: &Vec<UnstructuredElement>, max_chunk_size: u64) -> Vec<GroupedText> {
        let max_chunk_size = max_chunk_size as usize;
        let mut groups = Vec::new();
        let mut current_group = GroupedText::new();

        for i in 0..elements.len() {
            let element = &elements[i];
            let element_text = element.text.clone();

            // Skip over any uncategorized text (usually filler like headers/footers)
            if element.element_type == ElementType::UncategorizedText {
                continue;
            }

            // If adding the current element text would exceed the max_chunk_size,
            // push the current group to groups and start a new group
            if current_group.text.len() + element_text.len() > max_chunk_size {
                groups.push(current_group);
                current_group = GroupedText::new();
            }

            // If the current element text is larger than max_chunk_size,
            // split it into chunks and add them to groups
            if element_text.len() > max_chunk_size {
                let chunks = Self::split_into_chunks(&element_text, max_chunk_size);
                for chunk in chunks {
                    let mut new_group = GroupedText::new();
                    new_group.push_data(&chunk, element.metadata.page_number);
                    groups.push(new_group);
                }
                continue;
            }

            // Add the current element text to the current group
            current_group.push_data(&element_text, element.metadata.page_number);

            // If the current element type is NarrativeText and the next element's type is Title,
            // push the current group to groups and start a new group
            if element.element_type == ElementType::NarrativeText
                && i + 1 < elements.len()
                && elements[i + 1].element_type == ElementType::Title
            {
                groups.push(current_group);
                current_group = GroupedText::new();
            }
        }

        // Push the last group to groups
        if !current_group.text.is_empty() {
            groups.push(current_group);
        }

        // Filter out groups with a text of 5 characters or less
        groups = groups.into_iter().filter(|group| group.text.len() > 5).collect();

        groups
    }

    /// Splits a string into chunks at the nearest whitespace to a given size
    pub fn split_into_chunks(text: &str, chunk_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let end = start + chunk_size;
            let end = if end < text.len() {
                let mut end = end;
                while end > start && !text.as_bytes()[end].is_ascii_whitespace() {
                    end -= 1;
                }
                if end == start {
                    start + chunk_size
                } else {
                    end
                }
            } else {
                text.len()
            };

            let chunk = &text[start..end];
            chunks.push(chunk.to_string());

            start = end;
        }

        chunks
    }

    /// Generates a Blake3 hash of the data in the buffer
    pub fn generate_data_hash(buffer: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(buffer);
        let result = hasher.finalize();
        result.to_hex().to_string()
    }
}

/// An intermediary type in between `UnstructuredElement`s and
/// `Embedding`s/`DataChunk`s
pub struct GroupedText {
    text: String,
    page_numbers: Vec<u32>,
}

impl GroupedText {
    pub fn new() -> Self {
        GroupedText {
            text: String::new(),
            page_numbers: Vec::new(),
        }
    }

    /// Pushes data into the GroupedText fields
    pub fn push_data(&mut self, text: &str, page_number: Option<u32>) {
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        self.text.push_str(text);

        if let Some(page_number) = page_number {
            if !self.page_numbers.contains(&page_number) {
                self.page_numbers.push(page_number);
            }
        }
    }

    /// Outputs a String that holds an array of the page numbers
    pub fn format_page_num_string(&self) -> String {
        format!(
            "[{}]",
            self.page_numbers
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

/// Different types of elements Unstructured can output
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub enum ElementType {
    Title,
    NarrativeText,
    UncategorizedText,
    ListItem,
}

/// Output data from Unstructured which holds a piece of text and
/// relevant data.
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
