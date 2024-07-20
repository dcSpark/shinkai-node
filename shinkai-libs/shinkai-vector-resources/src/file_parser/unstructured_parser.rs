use std::collections::HashMap;

use super::file_parser::ShinkaiFileParser;
use super::file_parser_types::TextGroup;
use super::unstructured_types::{ElementType, UnstructuredElement};



use crate::resource_errors::VRError;





use serde_json::Value as JsonValue;

/// Struct which contains several methods related to parsing output from Unstructured
#[derive(Debug)]
pub struct UnstructuredParser;

impl UnstructuredParser {
    /// Parses the JSON Array response from Unstructured into a list of `UnstructuredElement`s
    pub fn parse_response_json(json: JsonValue) -> Result<Vec<UnstructuredElement>, VRError> {
        if let JsonValue::Array(array) = &json {
            let mut elements = Vec::new();
            for item in array {
                let el = serde_json::from_value(item.clone());
                if let Ok(element) = el {
                    elements.push(element);
                } else if let Err(err) = el {
                    eprintln!(
                        "Failed parsing Unstructured element JSON: {}  --- {}",
                        err.to_string(),
                        json.to_string()
                    );
                }
            }
            Ok(elements)
        } else {
            Err(VRError::FailedParsingUnstructedAPIJSON(format!(
                "{}: {}",
                "Response is not an array at top level: ".to_string(),
                json.to_string()
            )))
        }
    }

    /// Given a list of `UnstructuredElement`s, groups their text together into a hierarchy.
    /// Currently respects max_node_text_size, ensures all content in between title elements are sub-grouped,
    /// and skips over all uncategorized text.
    pub fn hierarchical_group_elements_text(
        elements: &Vec<UnstructuredElement>,
        max_node_text_size: u64,
    ) -> Vec<TextGroup> {
        let max_node_text_size = max_node_text_size as usize;
        let mut groups = Vec::new();
        let mut current_group = TextGroup::new_empty();
        let mut current_title_group: Option<TextGroup> = None;

        // Step 1: Remove duplicate titles to cleanup elements
        let elements = Self::remove_duplicate_title_elements(elements);

        // Step 2: Concatenate the first sequence of consecutive titles (useful for pdfs)
        let mut elements_iter = elements.iter().peekable();
        while let Some(element) = elements_iter.peek() {
            if element.element_type == ElementType::Title
                && current_group.text.len() + element.text.len() < max_node_text_size
                && element.text.len() > 2
            {
                current_group.push_data(&element.text, element.metadata.page_number);
                elements_iter.next(); // advance the iterator
            } else if element.text.len() <= 2 {
                elements_iter.next(); // advance the iterator
            } else {
                break;
            }
        }

        // Step 3: Main loop: process the remaining elements
        for element in elements_iter {
            let element_text = element.text.clone();

            // Pre-parsing to skip useless elements that Unstructured failed to clean out itself properly
            if Self::should_element_be_skipped(element) {
                continue;
            }

            if element.element_type != ElementType::Title {
                // If adding the current element text would exceed the max_node_text_size,
                // push the current group to title group or groups and start a new group
                if current_group.text.len() + element_text.len() > max_node_text_size {
                    ShinkaiFileParser::push_group_to_appropriate_parent(
                        current_group,
                        &mut current_title_group,
                        &mut groups,
                    );
                    current_group = TextGroup::new_empty();
                }

                // If the current element text is larger than max_node_text_size,
                // split it into chunks and add them to title group or groups
                if element_text.len() > max_node_text_size {
                    let chunks = ShinkaiFileParser::split_into_chunks(&element_text, max_node_text_size);
                    for chunk in chunks {
                        let mut new_group = TextGroup::new_empty();
                        new_group.push_data(&chunk, element.metadata.page_number);
                        ShinkaiFileParser::push_group_to_appropriate_parent(
                            new_group,
                            &mut current_title_group,
                            &mut groups,
                        );
                    }
                    continue;
                }

                // Add the current element text to the current group
                current_group.push_data(&element_text, element.metadata.page_number);
            }

            // If the current element type is Title,
            // push the current title group to groups and start a new title group
            if element.element_type == ElementType::Title {
                // Add the current group to the existing title group only if it contains more than the title text
                if !current_group.text.is_empty() && current_group.text != element_text {
                    ShinkaiFileParser::push_group_to_appropriate_parent(
                        current_group,
                        &mut current_title_group,
                        &mut groups,
                    );
                } else if let Some(title_group) = current_title_group.as_mut() {
                    if element_text.len() > 12 {
                        // If the current group only contains the title text and is > 12 len, add a default TextGroup that holds the title's text
                        // This both pre-populates the sub-group field, and allows for the title to be found in a search
                        // as a RetrievedNode to the LLM which can be useful in some content.
                        let mut new_grouped_text = TextGroup::new_empty();
                        new_grouped_text.push_data(&title_group.text, None);
                        title_group.sub_groups.push(new_grouped_text);
                    }
                }
                current_group = TextGroup::new_empty();

                // Push the existing title group to groups
                if let Some(title_group) = current_title_group.take() {
                    if title_group.text.len() > 12 || title_group.sub_groups.len() > 0 {
                        groups.push(title_group);
                    }
                }

                // Start a new title group
                current_title_group = Some(TextGroup::new_empty());
                if let Some(title_group) = current_title_group.as_mut() {
                    title_group.push_data(&element_text, element.metadata.page_number);
                }
                continue;
            }
        }

        // Push the last group to title group or groups
        if current_group.text.len() >= 2 {
            ShinkaiFileParser::push_group_to_appropriate_parent(current_group, &mut current_title_group, &mut groups);
        }
        // Push the last title group to groups
        if let Some(title_group) = current_title_group.take() {
            groups.push(title_group);
        }
        groups
    }

    /// Skip over any elements that Unstructured failed to clean out.
    fn should_element_be_skipped(element: &UnstructuredElement) -> bool {
        // Remove Uncategorized text (usually filler like headers/footers) && elements with no content at all.
        if element.element_type == ElementType::UncategorizedText || element.text.len() <= 2 {
            return true;
        }

        // Remove short narrative text which doesn't have enough content to matter
        if element.element_type == ElementType::NarrativeText && element.text.len() <= 12 {
            return true;
        }

        // For pieces of codeblocks which Unstructured failed to parse together and split up horribly.
        if !element.text.contains(' ')
            && (element.text.contains('.')
                || element.text.contains('_')
                || element.text.contains('[')
                || element.text.contains(']')
                || element.text.contains("::"))
        {
            return true;
        }

        false
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

    /// Concatenate elements text up to a maximum size.
    pub fn concatenate_elements_up_to_max_size(elements: &[UnstructuredElement], max_size: usize) -> String {
        let mut desc = String::new();
        for e in elements {
            if desc.len() + e.text.len() + 1 > max_size {
                break; // Stop appending if adding the next element would exceed max_size
            }
            desc.push_str(&e.text);
            desc.push('\n'); // Add a line break after each element's text
        }
        desc.trim_end().to_string() // Trim any trailing space before returning
    }
}
