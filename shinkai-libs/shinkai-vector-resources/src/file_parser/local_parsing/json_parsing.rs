use super::LocalFileParser;
use crate::file_parser::file_parser_types::TextGroup;
use crate::resource_errors::VRError;

use serde_json::Value as JsonValue;

impl LocalFileParser {
    /// Attempts to process the provided json file into a list of TextGroups.
    pub fn process_json_file(file_buffer: Vec<u8>, _max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let json_string = String::from_utf8(file_buffer.clone()).map_err(|_| VRError::FailedJSONParsing)?;
        let _json: JsonValue = serde_json::from_str(&json_string)?;

        Ok(vec![])
    }

    /// Recursively processes a JSON value to build a hierarchy of TextGroups.
    pub fn process_container_json_value(json: &JsonValue, max_node_text_size: u64) -> Vec<TextGroup> {
        let mut accumulated_string: String = String::new();
        match json {
            JsonValue::Object(map) => map
                .iter()
                .map(|(key, value)| {
                    let mut text_group = TextGroup::new_empty();
                    text_group.text = key.clone();
                    match value {
                        JsonValue::Object(_) | JsonValue::Array(_) => {
                            text_group.sub_groups = Self::process_container_json_value(value, max_node_text_size);
                        }
                        _ => {
                            // Check if the content is not too large, then try to append it to accumulated_string
                            if let Some(updated) =
                                Self::process_content_json_value(key, json, &mut accumulated_string, max_node_text_size)
                            {
                                accumulated_string = updated;
                            } else {
                            }
                        }
                    }
                    text_group
                })
                .collect(),
            JsonValue::Array(arr) => arr
                .iter()
                .flat_map(|value| Self::process_container_json_value(value, max_node_text_size))
                .collect(),
            _ => vec![],
        }
    }

    fn process_content_json_value(
        key: &str,
        json: &JsonValue,
        _accumulated_string: &String,
        max_node_text_size: u64,
    ) -> Option<String> {
        let formatted_string = format!("{}: {}", key, json.to_string());

        if formatted_string.len() > max_node_text_size as usize {
            None
        } else {
            Some(formatted_string)
        }
    }
}

// /// Recursively processes a JSON value to build a hierarchy of TextGroups.
// pub fn process_container_json_value(json: &JsonValue, max_node_text_size: u64) -> Vec<TextGroup> {
//     let mut text_groups: Vec<TextGroup> = Vec::new();
//     let mut accumulated_string: String = String::new();

//     match json {
//         JsonValue::Object(map) => {
//             for (key, value) in map {
//                 match value {
//                     JsonValue::Object(_) | JsonValue::Array(_) => {
//                         if !accumulated_string.is_empty() {
//                             let mut text_group = TextGroup::new_empty();
//                             text_group.text = std::mem::take(&mut accumulated_string);
//                             text_groups.push(text_group);
//                         }
//                         let mut text_group = TextGroup::new_empty();
//                         text_group.text = key.clone();
//                         text_group.sub_groups = Self::process_container_json_value(value, max_node_text_size);
//                         text_groups.push(text_group);
//                     } else {
//                         let content = format!("{}: {}", key, value.to_string());
//                         if accumulated_string.len() + content.len() > max_node_text_size as usize {
//                             if !accumulated_string.is_empty() {
//                                 let mut text_group = TextGroup::new_empty();
//                                 text_group.text = std::mem::take(&mut accumulated_string);
//                                 text_groups.push(text_group);
//                             }
//                             if content.len() > max_node_text_size as usize {
//                                 let mut part = content.chars().take(max_node_text_size as usize).collect::<String>();
//                                 // Ensure we don't split in the middle of a character
//                                 while !part.is_char_boundary(part.len()) {
//                                     part.pop();
//                                 }
//                                 accumulated_string.push_str(&part);
//                                 // If the part was too large and had to be split, add the first part as a TextGroup
//                                 let mut text_group = TextGroup::new_empty();
//                                 text_group.text = std::mem::take(&mut accumulated_string);
//                                 text_groups.push(text_group);
//                             } else {
//                                 accumulated_string = content;
//                             }
//                         } else {
//                             accumulated_string.push_str(&content);
//                         }
//                     }
//                 }
//             }
//         },
//         JsonValue::Array(arr) => {
//             for value in arr {
//                 text_groups.extend(Self::process_container_json_value(value, max_node_text_size));
//             }
//         },
//         _ => {}
//     }

//     if !accumulated_string.is_empty() {
//         let mut text_group = TextGroup::new_empty();
//         text_group.text = accumulated_string;
//         text_groups.push(text_group);
//     }

//     text_groups
// }
