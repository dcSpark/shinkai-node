use blake3::Hasher;
use chrono::{TimeZone, Utc};
use regex::{Captures, Regex};
use reqwest::Url;
use std::collections::HashMap;

use super::file_parser::ShinkaiFileParser;
use super::file_parser_types::TextGroup;
use crate::vector_resource::SourceFileType;

impl ShinkaiFileParser {
    pub const PURE_METADATA_REGEX: &'static str = r"!\{\{\{([^:}]+):((?:[^}]*\}{0,2}[^}]+))\}\}\}!";
    pub const METADATA_REGEX: &'static str = r"\{\{\{([^:}]+):((?:[^}]*\}{0,2}[^}]+))\}\}\}";
    pub const MD_URL_REGEX: &'static str = r"(.?)\[(.*?)\]\((.*?)\)";

    /// Key of page numbers metadata
    pub fn page_numbers_metadata_key() -> String {
        "pg_nums".to_string()
    }

    /// Key of datetime metadata
    pub fn datetime_metadata_key() -> String {
        "datetime".to_string()
    }

    /// Key of timestamp metadata
    pub fn timestamp_metadata_key() -> String {
        "timestamp".to_string()
    }

    // Key of likes metadata
    pub fn likes_metadata_key() -> String {
        "likes".to_string()
    }

    // Key of reposts metadata
    pub fn reposts_metadata_key() -> String {
        "reposts".to_string()
    }

    // Key of replies metadata
    pub fn replies_metadata_key() -> String {
        "replies".to_string()
    }

    /// Clean's the file name of auxiliary data (file extension, url in front of file name, etc.)
    pub fn clean_name(name: &str) -> String {
        // Decode URL-encoded characters to simplify processing.
        let decoded_name = urlencoding::decode(name).unwrap_or_else(|_| name.into());

        // Check if the name ends with ".htm" or ".html" and calculate the position to avoid deletion.
        let avoid_deletion_position = if decoded_name.ends_with(".htm") || decoded_name.ends_with(".html") {
            decoded_name.len().saturating_sub(4) // Position before ".htm"
        } else if decoded_name.ends_with(".html") {
            decoded_name.len().saturating_sub(5) // Position before ".html"
        } else if decoded_name.ends_with(".mhtml") {
            decoded_name.len().saturating_sub(6) // Position before ".mhtml"
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

    // Parse and extract metadata in a text
    // Returns the parsed text and a hashmap of metadata
    pub fn parse_and_extract_metadata(input_text: &str) -> (String, HashMap<String, String>, bool) {
        let mut metadata = HashMap::new();
        let mut parsed_any_metadata = false;
        let pure_metadata_re = Regex::new(Self::PURE_METADATA_REGEX).unwrap();
        let replaceable_metadata_re = Regex::new(Self::METADATA_REGEX).unwrap();

        let pure_result = pure_metadata_re.replace_all(input_text, |caps: &Captures| {
            Self::extract_metadata_from_capture(&mut metadata, &mut parsed_any_metadata, caps, true)
        });

        let parsed_result = replaceable_metadata_re.replace_all(&pure_result, |caps: &Captures| {
            Self::extract_metadata_from_capture(&mut metadata, &mut parsed_any_metadata, caps, false)
        });

        (parsed_result.to_string(), metadata, parsed_any_metadata)
    }

    // Helper function to extract metadata from a capture
    // is_pure is used to determine if the metadata should be removed from the text
    fn extract_metadata_from_capture(
        metadata: &mut HashMap<String, String>,
        parsed_any_metadata: &mut bool,
        caps: &Captures,
        is_pure: bool,
    ) -> String {
        // In case extracting the capture groups fails, return the original text which is guaranteed to be valid
        let key = match caps.get(1) {
            Some(key) => key.as_str(),
            None => return caps.get(0).unwrap().as_str().to_string(),
        };

        let value = match caps.get(2) {
            Some(value) => value.as_str(),
            None => return caps.get(0).unwrap().as_str().to_string(),
        };

        *parsed_any_metadata = true;

        // 1. Verify supported key value constraints and ignore invalid ones
        // 2. Replace any matched metadata or remove if it's pure
        match key {
            // timestamp or datetime: RFC3339 formatted date string
            _ if key == ShinkaiFileParser::datetime_metadata_key()
                || key == ShinkaiFileParser::timestamp_metadata_key() =>
            {
                let datetime = chrono::DateTime::parse_from_rfc3339(value);

                match datetime {
                    Ok(_) => {
                        metadata.insert(ShinkaiFileParser::datetime_metadata_key(), value.to_string());

                        if is_pure {
                            "".to_string()
                        } else {
                            value.to_string()
                        }
                    }
                    Err(_) => {
                        // Attempt to parse timestamp in a less strict format
                        let datetime = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.3fZ");

                        match datetime {
                            Ok(parsed_datetime) => {
                                let formatted_datetime = Utc.from_utc_datetime(&parsed_datetime).to_rfc3339();
                                metadata.insert(key.to_string(), formatted_datetime.clone());

                                if is_pure {
                                    "".to_string()
                                } else {
                                    formatted_datetime
                                }
                            }
                            Err(_) => value.to_string(),
                        }
                    }
                }
            }
            // pg_nums: Array of integers
            _ if key == ShinkaiFileParser::page_numbers_metadata_key() => {
                let page_numbers: Result<Vec<u32>, _> = value
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(",")
                    .map(|n| n.trim().parse::<u32>())
                    .collect();

                match page_numbers {
                    Ok(_) => {
                        metadata.insert(key.to_string(), value.to_string());

                        if is_pure {
                            "".to_string()
                        } else {
                            value.to_string()
                        }
                    }
                    Err(_) => value.to_string(),
                }
            }
            // likes, reposts, replies: Integer
            _ if key == ShinkaiFileParser::likes_metadata_key()
                || key == ShinkaiFileParser::reposts_metadata_key()
                || key == ShinkaiFileParser::replies_metadata_key() =>
            {
                let number = value.parse::<u32>();

                match number {
                    Ok(_) => {
                        metadata.insert(key.to_string(), value.to_string());

                        if is_pure {
                            "".to_string()
                        } else {
                            value.to_string()
                        }
                    }
                    Err(_) => value.to_string(),
                }
            }
            _ => {
                metadata.insert(key.to_string(), value.to_string());

                if is_pure {
                    "".to_string()
                } else {
                    value.to_string()
                }
            }
        }
    }

    pub fn parse_and_extract_md_metadata(input_text: &str) -> (String, HashMap<String, String>) {
        let mut metadata = HashMap::new();
        let md_url_re = Regex::new(Self::MD_URL_REGEX).unwrap();

        let parsed_result = md_url_re.replace_all(input_text, |caps: &Captures| {
            let prefix = match caps.get(1) {
                Some(prefix) => prefix.as_str(),
                None => return caps.get(0).unwrap().as_str().to_string(),
            };

            let text = match caps.get(2) {
                Some(text) => text.as_str(),
                None => return caps.get(0).unwrap().as_str().to_string(),
            };

            let url = match caps.get(3) {
                Some(url) => url.as_str(),
                None => return caps.get(0).unwrap().as_str().to_string(),
            };

            let mut shortened_url = Url::parse(url)
                .ok()
                .map(|u| {
                    let mut scheme = u.scheme().to_string();
                    let host = u.host_str().unwrap_or("").to_string();

                    if !scheme.is_empty() {
                        scheme = format!("{}://", scheme);
                    }

                    format!("{}{}", scheme, host)
                })
                .unwrap_or("".to_string());

            if shortened_url.is_empty() {
                shortened_url = url.chars().take(100).collect();
            }

            match prefix {
                "!" => {
                    let image_urls_entry = metadata.entry("image-urls".to_string()).or_insert(Vec::<String>::new());
                    image_urls_entry.push(format!("![{}]({})", text, url));
                    format!("![{}]({})", text, shortened_url)
                }
                _ => {
                    let link_urls_entry = metadata.entry("link-urls".to_string()).or_insert(Vec::<String>::new());
                    link_urls_entry.push(format!("[{}]({})", text, url));
                    format!("{}[{}]({})", prefix, text, shortened_url)
                }
            }
        });

        let serialized_metadata = metadata
            .into_iter()
            .map(|(key, values)| (key, serde_json::to_string(&values).unwrap_or_default()))
            .collect::<HashMap<String, String>>();

        (parsed_result.to_string(), serialized_metadata)
    }

    pub fn parse_and_split_into_text_groups(text: String, max_node_text_size: u64) -> Vec<TextGroup> {
        let mut text_groups = Vec::new();
        let (parsed_text, metadata, parsed_any_metadata) = ShinkaiFileParser::parse_and_extract_metadata(&text);
        let (parsed_md_text, md_metadata) = ShinkaiFileParser::parse_and_extract_md_metadata(&parsed_text);

        if parsed_md_text.len() as u64 > max_node_text_size {
            let chunks = if parsed_any_metadata {
                ShinkaiFileParser::split_into_chunks_with_metadata(&text, max_node_text_size as usize)
            } else {
                ShinkaiFileParser::split_into_chunks(&text, max_node_text_size as usize)
            };

            for chunk in chunks {
                let (parsed_chunk, metadata, _) = ShinkaiFileParser::parse_and_extract_metadata(&chunk);
                let (parsed_md_chunk, md_metadata) = ShinkaiFileParser::parse_and_extract_md_metadata(&parsed_chunk);
                let metadata = metadata.into_iter().chain(md_metadata).collect();
                text_groups.push(TextGroup::new(parsed_md_chunk, metadata, vec![], None));
            }
        } else {
            let metadata = metadata.into_iter().chain(md_metadata).collect();
            text_groups.push(TextGroup::new(parsed_md_text, metadata, vec![], None));
        }

        text_groups
    }

    // Creates a new text group and nests it under the last group at the given depth.
    // It splits text groups into chunks if needed and parses metadata in the text.
    pub fn push_text_group_by_depth(
        text_groups: &mut Vec<TextGroup>,
        depth: usize,
        text: String,
        max_node_text_size: u64,
    ) {
        if !text.is_empty() {
            let created_text_groups = ShinkaiFileParser::parse_and_split_into_text_groups(text, max_node_text_size);

            if depth > 0 {
                let mut parent_group = text_groups.last_mut();
                for _ in 1..depth {
                    if let Some(last_group) = parent_group {
                        parent_group = last_group.sub_groups.last_mut();
                    }
                }

                if let Some(last_group) = parent_group {
                    for text_group in created_text_groups {
                        last_group.push_sub_group(text_group);
                    }
                } else {
                    for text_group in created_text_groups {
                        text_groups.push(text_group);
                    }
                }
            } else {
                for text_group in created_text_groups {
                    text_groups.push(text_group);
                }
            }
        }
    }
}
