use std::collections::HashMap;

use super::LocalFileParser;
use crate::{shinkai_fs_error::ShinkaiFsError, simple_parser::{file_parser_helper::ShinkaiFileParser, text_group::TextGroup}};
use serde_json::Value as JsonValue;

impl LocalFileParser {
    /// Attempts to process the provided json file into a list of TextGroups.
    pub fn process_json_file(
        file_buffer: Vec<u8>,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let json_string =
            String::from_utf8(file_buffer).map_err(|_| ShinkaiFsError::FailedJSONParsing)?;
        let json: JsonValue = serde_json::from_str(&json_string)?;

        let text_groups = Self::process_container_json_value(&json, max_node_text_size);
        Ok(text_groups)
    }

    /// Recursively processes a JSON value into a *flat* list of TextGroups.
    pub fn process_container_json_value(json: &JsonValue, max_node_text_size: u64) -> Vec<TextGroup> {
        // Helper to merge small TextGroups
        let fn_merge_groups = |mut acc: Vec<TextGroup>, current_group: TextGroup| {
            if let Some(prev_group) = acc.last_mut() {
                if prev_group.text.len() + current_group.text.len() < max_node_text_size as usize {
                    prev_group
                        .text
                        .push_str(format!("\n{}", current_group.text).as_str());
                    return acc;
                }
            }
            acc.push(current_group);
            acc
        };

        match json {
            JsonValue::Object(map) => {
                // For each (key, value), produce a TextGroup for `key`, plus sub-groups from `value`.
                let mut result = Vec::new();
                for (key, value) in map {
                    // Optionally create a TextGroup for the key itself
                    result.push(TextGroup::new(key.clone(), HashMap::new(), None));
                    // Then flatten out whatever the value contains
                    let sub_result = Self::process_container_json_value(value, max_node_text_size);
                    result.extend(sub_result);
                }
                result.into_iter().fold(Vec::new(), fn_merge_groups)
            }
            JsonValue::Array(arr) => {
                // Flatten all elements
                let mut result = Vec::new();
                for value in arr {
                    let sub_result = Self::process_container_json_value(value, max_node_text_size);
                    result.extend(sub_result);
                }
                result.into_iter().fold(Vec::new(), fn_merge_groups)
            }
            // Base case: it’s a primitive (string, number, bool, null)
            _ => Self::process_content_json_value(None, json, max_node_text_size),
        }
    }

    /// Processes a single JSON value (primitive) into one or more TextGroups.
    fn process_content_json_value(
        key: Option<&str>,
        value: &JsonValue,
        max_node_text_size: u64,
    ) -> Vec<TextGroup> {
        let mut text_groups = Vec::new();
        let text = match key {
            Some(k) => format!("{}: {}", k, value.to_string()),
            None => value.to_string(),
        };

        if text.len() as u64 > max_node_text_size {
            let chunks = ShinkaiFileParser::split_into_chunks(&text, max_node_text_size as usize);
            for chunk in chunks {
                text_groups.push(TextGroup::new(chunk, HashMap::new(), None));
            }
        } else {
            text_groups.push(TextGroup::new(text, HashMap::new(), None));
        }

        text_groups
    }
}
