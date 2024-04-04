use crate::{embeddings::Embedding, file_parser::file_parser::ShinkaiFileParser};
use serde::Deserialize;

/// An intermediary type in between `UnstructuredElement`s and
/// `Embedding`s/`Node`s
#[derive(Debug, Clone, PartialEq)]
pub struct GroupedText {
    pub text: String,
    pub page_numbers: Vec<u32>,
    pub sub_groups: Vec<GroupedText>,
    pub embedding: Option<Embedding>,
}

impl GroupedText {
    pub fn new() -> Self {
        GroupedText {
            text: String::new(),
            page_numbers: Vec::new(),
            sub_groups: Vec::new(),
            embedding: None,
        }
    }

    /// Prepares a string to be used to generate an Embedding for this GroupedText.
    /// Extracts most prevalent keywords from all sub-groups and appends them to
    /// the end of the groups actual text.
    pub fn format_text_for_embedding(&self, max_chunk_size: u64) -> String {
        let mut keyword_string = String::new();
        let base_string = &self.text;
        let pre_keyword_length = base_string.len();

        // Extract keywords from the GroupedText and its sub-groups
        let keywords: Vec<String> = ShinkaiFileParser::extract_keywords(&vec![self.clone()], 1);

        for keyword in keywords {
            if pre_keyword_length + keyword_string.len() + keyword.len() <= max_chunk_size as usize {
                keyword_string = format!("{}, {}", keyword_string, keyword);
            } else {
                break;
            }
        }

        format!("{} Keywords: {}", base_string, keyword_string.trim_start_matches(", "))
    }

    /// Pushes data into this GroupedText
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

    /// Pushes a sub-group into this GroupedText
    pub fn push_sub_group(&mut self, sub_group: GroupedText) {
        self.sub_groups.push(sub_group);
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
