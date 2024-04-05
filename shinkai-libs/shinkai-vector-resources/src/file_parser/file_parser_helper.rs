use super::file_parser::ShinkaiFileParser;
use super::file_parser_types::TextGroup;
use super::unstructured_api::UnstructuredAPI;
use crate::data_tags::DataTag;
use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::resource_errors::VRError;
use crate::source::DistributionInfo;
use crate::source::TextChunkingStrategy;
use crate::source::VRSourceReference;
use crate::vector_resource::SourceFileReference;
use crate::vector_resource::SourceFileType;
use crate::vector_resource::SourceReference;
use crate::vector_resource::{BaseVectorResource, DocumentVectorResource, VectorResource, VectorResourceCore};
use blake3::Hasher;
use futures::stream::SelectNextSome;
use serde_json::Value as JsonValue;

impl ShinkaiFileParser {
    /// Clean's the file name of auxiliary data (file extension, url in front of file name, etc.)
    pub fn clean_name(name: &str) -> String {
        // Decode URL-encoded characters to simplify processing.
        let decoded_name = urlencoding::decode(name).unwrap_or_else(|_| name.into());

        // Check if the name ends with ".htm" or ".html" and calculate the position to avoid deletion.
        let avoid_deletion_position = if decoded_name.ends_with(".htm") || decoded_name.ends_with(".html") {
            decoded_name.len().saturating_sub(4) // Position before ".htm" or ".html"
        } else {
            decoded_name.len() // Use the full length if not ending with ".htm" or ".html"
        };
        // Find the last occurrence of "/" or "%2F" that is not too close to the ".htm" extension.
        let last_relevant_slash_position = decoded_name.rmatch_indices(&['/', '%']).find_map(|(index, _)| {
            if index + 3 < avoid_deletion_position && decoded_name[index..].starts_with("%2F") {
                Some(index)
            } else if index + 1 < avoid_deletion_position && decoded_name[index..].starts_with("/") {
                Some(index)
            } else {
                None
            }
        });
        // If a relevant slash is found, slice the string from the character immediately following this slash.
        let http_cleaned = match last_relevant_slash_position {
            Some(index) => decoded_name
                .get((index + if decoded_name[index..].starts_with("%2F") { 3 } else { 1 })..)
                .unwrap_or(&decoded_name),
            None => &decoded_name,
        };

        let http_cleaned = if http_cleaned.is_empty() || http_cleaned == ".html" || http_cleaned == ".htm" {
            decoded_name.to_string()
        } else {
            http_cleaned.to_string()
        };

        // Remove extension
        let cleaned_name = SourceFileType::clean_string_of_extension(&http_cleaned);

        cleaned_name
    }

    /// Helper function that processes groups into a list of descriptions.
    /// Only takes the top level Group text, does not traverse deeper.
    pub fn process_groups_into_descriptions_list(
        groups: &Vec<TextGroup>,
        max_size: usize,
        max_node_text_size: usize,
    ) -> Vec<String> {
        let mut descriptions = Vec::new();
        let mut description = String::new();
        let mut total_size = 0;

        for group in groups {
            let element_text = &group.text;
            if description.len() + element_text.len() > max_node_text_size {
                descriptions.push(description.clone());
                total_size += description.len();
                description.clear();
            }
            if total_size + element_text.len() > max_size {
                break;
            }
            description.push_str(element_text);
            description.push(' ');
        }
        if !description.is_empty() {
            descriptions.push(description);
        }

        descriptions
    }

    /// Processes groups into a single description string.
    /// Only takes the top level Group text, does not traverse deeper.
    pub fn process_groups_into_description(
        groups: &Vec<TextGroup>,
        max_size: usize,
        max_node_text_size: usize,
    ) -> String {
        let descriptions = Self::process_groups_into_descriptions_list(groups, max_size, max_node_text_size);
        descriptions.join(" ")
    }

    /// Helper method for setting a description if none provided for process_new_doc_resource
    pub fn _setup_resource_description(
        desc: Option<String>,
        text_groups: &Vec<TextGroup>,
        max_size: usize,
        max_node_text_size: usize,
    ) -> Option<String> {
        if let Some(description) = desc {
            Some(description.to_string())
        } else if !text_groups.is_empty() {
            Some(Self::process_groups_into_description(
                text_groups,
                max_size,
                max_node_text_size,
            ))
        } else {
            None
        }
    }

    /// Generates a Blake3 hash of the data in the buffer
    pub fn generate_data_hash(buffer: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(buffer);
        let result = hasher.finalize();
        result.to_hex().to_string()
    }
}
