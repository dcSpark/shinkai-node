use std::collections::HashMap;

use super::LocalFileParser;
use crate::file_parser::file_parser::ShinkaiFileParser;
use crate::file_parser::file_parser_types::TextGroup;
use crate::shinkai_fs_error::ShinkaiFsError;
use serde_json::Value as JsonValue;

impl LocalFileParser {
    /// Attempts to process the provided json file into a list of TextGroups.
    pub fn process_json_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let json_string = String::from_utf8(file_buffer.clone()).map_err(|_| ShinkaiFsError::FailedJSONParsing)?;
        let json: JsonValue = serde_json::from_str(&json_string)?;

        let text_groups = Self::process_container_json_value(&json, max_node_text_size);

        Ok(text_groups)
    }

    /// Recursively processes a JSON value to build a hierarchy of TextGroups.
    pub fn process_container_json_value(json: &JsonValue, max_node_text_size: u64) -> Vec<TextGroup> {
        let fn_merge_groups = |mut acc: Vec<TextGroup>, current_group: TextGroup| {
            if let Some(prev_group) = acc.last_mut() {
                if prev_group.sub_groups.is_empty()
                    && current_group.sub_groups.is_empty()
                    && prev_group.text.len() + current_group.text.len() < max_node_text_size as usize
                {
                    prev_group.text.push_str(format!("\n{}", current_group.text).as_str());
                    return acc;
                }
            }

            acc.push(current_group);
            acc
        };

        match json {
            JsonValue::Object(map) => map
                .iter()
                .flat_map(|(key, value)| match value {
                    JsonValue::Object(_) | JsonValue::Array(_) => {
                        let mut text_group = TextGroup::new_empty();
                        text_group.text = key.clone();
                        text_group.sub_groups = Self::process_container_json_value(value, max_node_text_size);

                        vec![text_group]
                    }
                    _ => Self::process_content_json_value(Some(key), value, max_node_text_size),
                })
                .fold(Vec::new(), fn_merge_groups),
            JsonValue::Array(arr) => arr
                .iter()
                .flat_map(|value| Self::process_container_json_value(value, max_node_text_size))
                .fold(Vec::new(), fn_merge_groups),
            _ => Self::process_content_json_value(None, json, max_node_text_size),
        }
    }

    fn process_content_json_value(key: Option<&str>, value: &JsonValue, max_node_text_size: u64) -> Vec<TextGroup> {
        let mut text_groups = Vec::new();

        let text = match key {
            Some(key) => format!("{}: {}", key, value.to_string()),
            None => value.to_string(),
        };

        if text.len() as u64 > max_node_text_size {
            let chunks = ShinkaiFileParser::split_into_chunks(&text, max_node_text_size as usize);

            for chunk in chunks {
                text_groups.push(TextGroup::new(chunk, HashMap::new(), vec![], None));
            }
        } else {
            text_groups.push(TextGroup::new(text, HashMap::new(), vec![], None));
        }

        text_groups
    }
}
