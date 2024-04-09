use super::LocalFileParser;
use crate::file_parser::file_parser::ShinkaiFileParser;
use crate::file_parser::file_parser_types::TextGroup;
use crate::resource_errors::VRError;
use crate::source::VRSourceReference;
use serde_json::Value as JsonValue;

impl LocalFileParser {
    /// Attempts to process the provided json file into a list of TextGroups.
    pub fn process_txt_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let txt_string = String::from_utf8(file_buffer.clone()).map_err(|_| VRError::FailedJSONParsing)?;
        let sentences = LocalFileParser::process_into_sentences(txt_string);
        let text_groups = LocalFileParser::process_into_text_groups(sentences, max_node_text_size);
        // for sentence in &sentences {
        //     println!("S: {}", sentence);
        // }
        // for text_group in &text_groups {
        //     println!("TG: {}", text_group.text);
        // }

        Ok(text_groups)
    }

    /// Splits the text into a list of sentences.
    pub fn process_into_sentences(text: String) -> Vec<String> {
        let punctuation_marks = [',', '.', ';', '-', '&', '(', '{', '<', '"', '\'', '`'];
        text.split("\n")
            .filter(|line| line.trim().len() > 1) // Filter out lines that are empty or have only a single character after trimming
            .flat_map(|s| {
                let s = s.trim();
                let s = if !punctuation_marks.iter().any(|&mark| s.ends_with(mark)) {
                    format!("{}.", s)
                } else {
                    s.to_string()
                };
                s.split(". ").map(|s| s.trim().to_string()).collect::<Vec<String>>()
            })
            .collect()
    }

    /// Build a non-hierarchical list of TextGroups using the sentences
    pub fn process_into_text_groups(text_lines: Vec<String>, max_node_text_size: u64) -> Vec<TextGroup> {
        let mut text_groups = Vec::new();
        let mut current_text = String::new();

        for line in text_lines {
            if line.len() as u64 + current_text.len() as u64 > max_node_text_size {
                if !current_text.is_empty() {
                    text_groups.push(TextGroup::new(current_text.clone(), vec![], vec![], None));
                    current_text.clear();
                }
                if line.len() as u64 > max_node_text_size {
                    // If the line itself exceeds max_node_text_size, split it into chunks
                    let chunks = ShinkaiFileParser::split_into_chunks(&line, max_node_text_size as usize);
                    for chunk in chunks {
                        text_groups.push(TextGroup::new(chunk, vec![], vec![], None));
                    }
                } else {
                    current_text = line;
                }
            } else {
                if !current_text.is_empty() {
                    current_text.push(' '); // Add space between sentences
                }
                current_text.push_str(&line);
            }
        }

        // Don't forget to add the last accumulated text as a TextGroup if it's not empty
        if !current_text.is_empty() {
            text_groups.push(TextGroup::new(current_text, vec![], vec![], None));
        }

        text_groups
    }
}
