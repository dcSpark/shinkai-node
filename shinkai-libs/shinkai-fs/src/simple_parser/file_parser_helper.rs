use blake3::Hasher;
use chrono::{TimeZone, Utc};
use regex::{Captures, Regex};
use reqwest::Url;
use std::collections::HashMap;

use super::text_group::TextGroup;

pub struct ShinkaiFileParser;

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

    // // Key of likes metadata
    // pub fn likes_metadata_key() -> String {
    //     "likes".to_string()
    // }

    // // Key of reposts metadata
    // pub fn reposts_metadata_key() -> String {
    //     "reposts".to_string()
    // }

    // // Key of replies metadata
    // pub fn replies_metadata_key() -> String {
    //     "replies".to_string()
    // }

    // /// Clean's the file name of auxiliary data (file extension, url in front of file name, etc.)
    // pub fn clean_name(name: &str) -> String {
    //     // Decode URL-encoded characters to simplify processing.
    //     let decoded_name = urlencoding::decode(name).unwrap_or_else(|_| name.into());

    //     // Check if the name ends with ".htm" or ".html" and calculate the position to avoid deletion.
    //     let avoid_deletion_position = if decoded_name.ends_with(".htm") || decoded_name.ends_with(".html") {
    //         decoded_name.len().saturating_sub(4) // Position before ".htm"
    //     } else if decoded_name.ends_with(".html") {
    //         decoded_name.len().saturating_sub(5) // Position before ".html"
    //     } else if decoded_name.ends_with(".mhtml") {
    //         decoded_name.len().saturating_sub(6) // Position before ".mhtml"
    //     } else {
    //         decoded_name.len() // Use the full length if not ending with ".htm" or ".html"
    //     };

    //     // Find the last occurrence of "/" or "%2F" that is not too close to the ".htm" extension.
    //     let last_relevant_slash_position = decoded_name.rmatch_indices(&['/', '%']).find_map(|(index, _)| {
    //         if index + 3 < avoid_deletion_position && decoded_name[index..].starts_with("%2F") {
    //             Some(index)
    //         } else if index + 1 < avoid_deletion_position && decoded_name[index..].starts_with("/") {
    //             Some(index)
    //         } else {
    //             None
    //         }
    //     });
    //     // If a relevant slash is found, slice the string from the character immediately following this slash.
    //     let http_cleaned = match last_relevant_slash_position {
    //         Some(index) => decoded_name
    //             .get((index + if decoded_name[index..].starts_with("%2F") { 3 } else { 1 })..)
    //             .unwrap_or(&decoded_name),
    //         None => &decoded_name,
    //     };

    //     let http_cleaned = if http_cleaned.is_empty() || http_cleaned == ".html" || http_cleaned == ".htm" {
    //         decoded_name.to_string()
    //     } else {
    //         http_cleaned.to_string()
    //     };

    //     // Remove extension
    //     let cleaned_name = SourceFileType::clean_string_of_extension(&http_cleaned);

    //     cleaned_name
    // }

    pub async fn initialize_local_file_parser() -> Result<(), Box<dyn std::error::Error>> {
        use shinkai_ocr::image_parser::ImageParser;
        ImageParser::check_and_download_dependencies().await
    }

    /// Helper function that processes groups into a list of descriptions.
    /// Only takes the top-level group text, does not recurse into subgroups.
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
    /// Only takes the top-level `TextGroup` text, not subgroups.
    pub fn process_groups_into_description(
        groups: &Vec<TextGroup>,
        max_size: usize,
        max_node_text_size: usize,
    ) -> String {
        let descriptions = Self::process_groups_into_descriptions_list(groups, max_size, max_node_text_size);
        descriptions.join(" ")
    }

    /// Helper method for setting a description if none is provided.
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

    /// Generates a Blake3 hash of the data in the buffer.
    pub fn generate_data_hash(buffer: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(buffer);
        let result = hasher.finalize();
        result.to_hex().to_string()
    }

    /// Parse and extract metadata from `input_text`.
    /// Returns `(parsed_text, metadata, parsed_any_metadata)`.
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

    /// Helper function to extract metadata from a capture.
    /// If `is_pure == true`, the captured text is removed from the final string.
    fn extract_metadata_from_capture(
        metadata: &mut HashMap<String, String>,
        parsed_any_metadata: &mut bool,
        caps: &Captures,
        is_pure: bool,
    ) -> String {
        let key = match caps.get(1) {
            Some(key) => key.as_str(),
            None => return caps.get(0).unwrap().as_str().to_string(),
        };
        let value = match caps.get(2) {
            Some(value) => value.as_str(),
            None => return caps.get(0).unwrap().as_str().to_string(),
        };

        *parsed_any_metadata = true;

        match key {
            // timestamp or datetime: RFC3339 format
            _ if key == ShinkaiFileParser::datetime_metadata_key()
                || key == ShinkaiFileParser::timestamp_metadata_key() =>
            {
                let datetime = chrono::DateTime::parse_from_rfc3339(value);
                match datetime {
                    Ok(_) => {
                        metadata.insert(ShinkaiFileParser::datetime_metadata_key(), value.to_string());
                        if is_pure { "".to_string() } else { value.to_string() }
                    }
                    Err(_) => {
                        // Attempt a less strict format
                        let datetime = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.3fZ");
                        match datetime {
                            Ok(parsed_datetime) => {
                                let formatted_datetime = Utc.from_utc_datetime(&parsed_datetime).to_rfc3339();
                                metadata.insert(key.to_string(), formatted_datetime.clone());
                                if is_pure { "".to_string() } else { formatted_datetime }
                            }
                            Err(_) => value.to_string(),
                        }
                    }
                }
            }
            // pg_nums: array of integers
            _ if key == ShinkaiFileParser::page_numbers_metadata_key() => {
                let page_numbers: Result<Vec<u32>, _> = value
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .map(|n| n.trim().parse::<u32>())
                    .collect();

                match page_numbers {
                    Ok(_) => {
                        metadata.insert(key.to_string(), value.to_string());
                        if is_pure { "".to_string() } else { value.to_string() }
                    }
                    Err(_) => value.to_string(),
                }
            }
            // Fallback
            _ => {
                metadata.insert(key.to_string(), value.to_string());
                if is_pure { "".to_string() } else { value.to_string() }
            }
        }
    }

    /// Parse and extract Markdown URLs like `[text](url)` or `![text](url)`.
    pub fn parse_and_extract_md_metadata(input_text: &str) -> (String, HashMap<String, String>) {
        let mut metadata = HashMap::new();
        let md_url_re = Regex::new(Self::MD_URL_REGEX).unwrap();

        let parsed_result = md_url_re.replace_all(input_text, |caps: &Captures| {
            let prefix = caps.get(1).map_or("", |m| m.as_str());
            let text   = caps.get(2).map_or("", |m| m.as_str());
            let url    = caps.get(3).map_or("", |m| m.as_str());

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
                .unwrap_or_else(|| "".to_string());

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

    /// Splits `text` into as many `TextGroup`s as needed, ignoring sub-groups.
    pub fn parse_and_split_into_text_groups(
        text: String,
        max_node_text_size: u64,
        page_number: Option<u32>,
    ) -> Vec<TextGroup> {
        let mut text_groups = Vec::new();
        let (parsed_text, metadata, parsed_any_metadata) =
            ShinkaiFileParser::parse_and_extract_metadata(&text);
        let (parsed_md_text, md_metadata) =
            ShinkaiFileParser::parse_and_extract_md_metadata(&parsed_text);

        // Merge the two sets of metadata
        let all_metadata = metadata.into_iter().chain(md_metadata).collect::<HashMap<_, _>>();

        if parsed_md_text.len() as u64 > max_node_text_size {
            // If the text is too large, split it
            let chunks = if parsed_any_metadata {
                ShinkaiFileParser::split_into_chunks_with_metadata(&text, max_node_text_size as usize)
            } else {
                Self::split_into_chunks(&text, max_node_text_size as usize)
            };

            for chunk in chunks {
                let (parsed_chunk, chunk_metadata, _) = Self::parse_and_extract_metadata(&chunk);
                let (parsed_md_chunk, md_metadata_chunk) = Self::parse_and_extract_md_metadata(&parsed_chunk);

                let merged_metadata = chunk_metadata
                    .into_iter()
                    .chain(md_metadata_chunk)
                    .collect::<HashMap<_, _>>();

                let mut text_group = TextGroup::new(parsed_md_chunk, merged_metadata, None);
                if let Some(page_number) = page_number {
                    text_group.push_page_number(page_number);
                }
                text_groups.push(text_group);
            }
        } else {
            // Single chunk
            let mut text_group = TextGroup::new(parsed_md_text, all_metadata, None);
            if let Some(page_number) = page_number {
                text_group.push_page_number(page_number);
            }
            text_groups.push(text_group);
        }

        text_groups
    }

    /// Previously, this method would nest groups at `depth`.
    /// Now, we flatten everything and simply append the created groups.
    pub fn push_text_group_by_depth(
        text_groups: &mut Vec<TextGroup>,
        _depth: usize, // ignore depth
        text: String,
        max_node_text_size: u64,
        page_number: Option<u32>,
    ) {
        if !text.is_empty() {
            let created_text_groups = Self::parse_and_split_into_text_groups(
                text,
                max_node_text_size,
                page_number
            );
            // Just extend the top-level list, ignoring `_depth`.
            text_groups.extend(created_text_groups);
        }
    }
}
