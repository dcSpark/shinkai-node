use super::unstructured_types::{ElementType, GroupedText, UnstructuredElement};
use crate::base_vector_resources::BaseVectorResource;
use crate::data_tags::DataTag;
use crate::document_resource::DocumentVectorResource;
use crate::embedding_generator::EmbeddingGenerator;
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::VectorResource;
use blake3::Hasher;
use keyphrases::KeyPhraseExtractor;
#[cfg(feature = "native-http")]
use serde_json::Value as JsonValue;
use std::collections::HashMap;

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
    pub fn extract_keywords(group: &Vec<GroupedText>, num: u64) -> Vec<String> {
        // Extract all the text out of all the elements and combine them together into a single string
        let text = group
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

    /// Processes an ordered list of `UnstructuredElement`s returned from Unstructured into
    /// a ready-to-go BaseVectorResource
    pub fn process_elements_into_resource(
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: &str,
        max_chunk_size: u64,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // Group elements together before generating the doc
        let text_groups = UnstructuredParser::hierarchical_group_elements_text(&elements, max_chunk_size);
        Self::process_new_doc_resource(text_groups, generator, name, desc, source, parsing_tags, resource_id)
    }

    /// Recursively processes all text groups & their sub groups into DocumentResources
    fn process_new_doc_resource(
        text_groups: Vec<GroupedText>,
        generator: &dyn EmbeddingGenerator,
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        resource_id: &str,
    ) -> Result<BaseVectorResource, VectorResourceError> {
        // If description is None, use the first text
        let mut resource_desc = desc;
        let desc_string = text_groups[0].text.to_string();
        if desc.is_none() && text_groups.len() > 0 {
            resource_desc = Some(&desc_string);
        }
        // Create doc resource and initial setup
        let mut doc = DocumentVectorResource::new_empty(name, resource_desc, source.clone(), &resource_id);
        doc.set_embedding_model_used(generator.model_type());

        // Extract keywords from the elements
        let keywords = UnstructuredParser::extract_keywords(&text_groups, 50);

        // Set the resource embedding, using the keywords + name + desc + source
        doc.update_resource_embedding(generator, keywords)?;

        // Add each text group as either Vector Resource DataChunks,
        // or data-holding DataChunks depending on if each has any sub-groups
        for grouped_text in &text_groups {
            // If the grouped_text has at least 1 sub group, recurse
            if !grouped_text.sub_groups.is_empty() {
                println!(
                    "Processing groupd text `{}`\nCreating New Vector Resource",
                    grouped_text.text
                );
                let new_name = &grouped_text.text;
                let new_resource_id = Self::generate_data_hash(new_name.as_bytes());
                let new_doc = Self::process_new_doc_resource(
                    grouped_text.sub_groups.clone(),
                    generator,
                    new_name,
                    None,
                    source.clone(),
                    parsing_tags,
                    &new_resource_id,
                )?;
                let mut metadata = HashMap::new();
                metadata.insert("page_numbers".to_string(), grouped_text.format_page_num_string());
                doc.append_vector_resource(new_doc, Some(metadata));
            } else {
                println!(
                    "Processing groupd text `{}`\nAdding to doc: `{}`",
                    grouped_text.text,
                    doc.name()
                );
                // Skip creating DataChunks for no/very little text
                if grouped_text.text.len() < 6 {
                    continue;
                }
                // Generate the embedding
                let embedding = generator.generate_embedding_default(&grouped_text.text)?;

                // Add page numbers to metadata
                let mut metadata = HashMap::new();
                if !grouped_text.page_numbers.is_empty() {
                    metadata.insert("page_numbers".to_string(), grouped_text.format_page_num_string());
                }

                // Append the data
                doc.append_data(&grouped_text.text, Some(metadata), &embedding, parsing_tags);
            }
        }

        Ok(BaseVectorResource::Document(doc))
    }

    /// Given a list of `UnstructuredElement`s, groups their text together with some processing logic.
    /// Currently respects max_chunk_size, ensures splitting between narrative text/new title,
    /// and skips over all uncategorized text.
    pub fn flat_group_elements_text(elements: &Vec<UnstructuredElement>, max_chunk_size: u64) -> Vec<GroupedText> {
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

        // Filter out groups with a text of 15 characters or less
        groups = groups.into_iter().filter(|group| group.text.len() > 15).collect();

        groups
    }

    /// Internal method used to push into correct group for hierarchical grouping
    fn push_group_to_appropriate_parent(
        group: GroupedText,
        title_group: &mut Option<GroupedText>,
        groups: &mut Vec<GroupedText>,
    ) {
        if let Some(title_group) = title_group.as_mut() {
            title_group.push_sub_group(group);
        } else {
            groups.push(group);
        }
    }

    /// Given a list of `UnstructuredElement`s, groups their text together into a hierarchy.
    /// Currently respects max_chunk_size, ensures all content in between title elements are sub-grouped,
    /// and skips over all uncategorized text.
    pub fn hierarchical_group_elements_text(
        elements: &Vec<UnstructuredElement>,
        max_chunk_size: u64,
    ) -> Vec<GroupedText> {
        let max_chunk_size = max_chunk_size as usize;
        let mut groups = Vec::new();
        let mut current_group = GroupedText::new();
        let mut current_title_group: Option<GroupedText> = None;

        // Remove duplicate titles to cleanup elements
        let elements = Self::remove_duplicate_title_elements(elements);

        // Pre-processing step: handle the first sequence of consecutive titles
        let mut elements_iter = elements.iter().peekable();
        while let Some(element) = elements_iter.peek() {
            if element.element_type == ElementType::Title {
                current_group.push_data(&element.text, element.metadata.page_number);
                elements_iter.next(); // advance the iterator
            } else {
                break;
            }
        }

        // Main loop: process the remaining elements
        for element in elements_iter {
            let element_text = element.text.clone();

            // Skip over any uncategorized text (usually filler like headers/footers)
            if element.element_type == ElementType::UncategorizedText {
                continue;
            }

            // If adding the current element text would exceed the max_chunk_size,
            // push the current group to title group or groups and start a new group
            if current_group.text.len() + element_text.len() > max_chunk_size {
                Self::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
                current_group = GroupedText::new();
            }

            // If the current element text is larger than max_chunk_size,
            // split it into chunks and add them to title group or groups
            if element_text.len() > max_chunk_size {
                let chunks = Self::split_into_chunks(&element_text, max_chunk_size);
                for chunk in chunks {
                    let mut new_group = GroupedText::new();
                    new_group.push_data(&chunk, element.metadata.page_number);
                    Self::push_group_to_appropriate_parent(new_group, &mut current_title_group, &mut groups);
                }
                continue;
            }

            // Add the current element text to the current group
            current_group.push_data(&element_text, element.metadata.page_number);

            // If the current element type is Title,
            // push the current title group to groups and start a new title group
            if element.element_type == ElementType::Title {
                // Add the current group to the existing title group only if it contains more than the title text
                if !current_group.text.is_empty() && current_group.text != element_text {
                    Self::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
                } else if let Some(title_group) = current_title_group.as_mut() {
                    // If the current group only contains the title text, add an empty GroupedText to the sub-groups
                    title_group.sub_groups.push(GroupedText::new());
                }
                current_group = GroupedText::new();

                // Push the existing title group to groups
                if let Some(title_group) = current_title_group.take() {
                    groups.push(title_group);
                }

                // Start a new title group
                current_title_group = Some(GroupedText::new());
                if let Some(title_group) = current_title_group.as_mut() {
                    title_group.push_data(&element_text, element.metadata.page_number);
                }
                continue;
            }
        }

        // Push the last group to title group or groups
        if !current_group.text.is_empty() {
            Self::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
        }

        // Push the last title group to groups
        if let Some(title_group) = current_title_group.take() {
            groups.push(title_group);
        }

        // Filter out groups with a text of 15 characters or less and no sub-groups
        groups = groups
            .into_iter()
            .filter(|group| group.text.len() > 15 || !group.sub_groups.is_empty())
            .collect();

        groups
    }

    /// Removes any title element which occurs more than once.
    /// Useful especially for PDFs/docs where headers/footers repeat
    /// and Unstructured failed to separate them out as Uncategorized.
    pub fn remove_duplicate_title_elements(elements: &Vec<UnstructuredElement>) -> Vec<UnstructuredElement> {
        let mut title_counts = HashMap::new();
        let mut result = Vec::new();

        // First pass: count the occurrences of each title
        for element in elements {
            if element.element_type == ElementType::Title {
                *title_counts.entry(&element.text).or_insert(0) += 1;
            }
        }

        // Second pass: build the result, skipping titles that appear more than once
        for element in elements {
            if element.element_type == ElementType::Title {
                if title_counts[&element.text] > 1 {
                    continue;
                }
            }
            result.push(element.clone());
        }

        result
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
