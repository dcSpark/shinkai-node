use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::file_parser::ShinkaiFileParser;


/// An intermediary type for processing content into Node's held in VectorResources
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextGroup {
    pub text: String,
    pub metadata: HashMap<String, String>,
    pub sub_groups: Vec<TextGroup>,
    pub embedding: Option<Vec<f32>>,
}

impl TextGroup {
    pub fn new(
        text: String,
        metadata: HashMap<String, String>,
        sub_groups: Vec<TextGroup>,
        embedding: Option<Vec<f32>>,
    ) -> Self {
        TextGroup {
            text,
            metadata,
            sub_groups,
            embedding,
        }
    }

    pub fn new_empty() -> Self {
        TextGroup {
            text: String::new(),
            metadata: HashMap::new(),
            sub_groups: Vec::new(),
            embedding: None,
        }
    }

    /// Prepares a string to be used to generate an Embedding for this TextGroup.
    /// Extracts most prevalent keywords from all sub-groups and appends them to
    /// the end of the group's actual text.
    pub fn format_text_for_embedding(&self, max_node_text_size: u64) -> String {
        let mut keyword_string = String::new();
        let base_string = &self.text;
        let pre_keyword_length = base_string.len();

        // Extract keywords from the TextGroup and its sub-groups
        let keywords: Vec<String> = ShinkaiFileParser::extract_keywords(&vec![self.clone()], 1);

        for keyword in keywords {
            if pre_keyword_length + keyword_string.len() + keyword.len() <= max_node_text_size as usize {
                keyword_string = format!("{}, {}", keyword_string, keyword);
            } else {
                break;
            }
        }

        format!("{} Keywords: {}", base_string, keyword_string.trim_start_matches(", "))
    }

    /// Pushes data into this TextGroup and extracts metadata
    pub fn push_data(&mut self, text: &str, page_number: Option<u32>) {
        if !self.text.is_empty() {
            self.text.push(' ');
        }

        let (parsed_text, metadata, parsed_any_metadata) = ShinkaiFileParser::parse_and_extract_metadata(text);
        if parsed_any_metadata {
            self.text.push_str(&parsed_text);
            self.metadata.extend(metadata);
        } else {
            self.text.push_str(text);
        }

        if let Some(page_number) = page_number {
            self.push_page_number(page_number);
        }
    }

    pub fn push_page_number(&mut self, page_number: u32) {
        let mut unique_page_numbers: HashSet<u32> = HashSet::new();

        if let Some(page_numbers_metadata) = self.metadata.get(&ShinkaiFileParser::page_numbers_metadata_key()) {
            let page_numbers_metadata: Result<Vec<u32>, _> = page_numbers_metadata
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|n| n.trim().parse::<u32>())
                .collect();

            if let Ok(page_numbers) = page_numbers_metadata {
                for pg in page_numbers {
                    unique_page_numbers.insert(pg);
                }
            }
        }

        unique_page_numbers.insert(page_number);

        self.metadata.insert(
            ShinkaiFileParser::page_numbers_metadata_key(),
            format!(
                "[{}]",
                unique_page_numbers
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        );
    }

    pub fn push_sub_group(&mut self, sub_group: TextGroup) {
        self.sub_groups.push(sub_group);
    }
}
