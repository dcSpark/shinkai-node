use std::collections::HashMap;

use regex::Regex;

use super::LocalFileParser;
use crate::{shinkai_fs_error::ShinkaiFsError, simple_parser::{file_parser_helper::ShinkaiFileParser, text_group::TextGroup}};

impl LocalFileParser {
    /// Attempts to process the provided json file into a list of TextGroups.
    pub fn process_txt_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let txt_string = String::from_utf8(file_buffer).map_err(|_| ShinkaiFsError::FailedTXTParsing)?;
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

    /// Build a non-hierarchical list of TextGroups using the sentences
    pub fn process_into_text_groups(text_lines: Vec<String>, max_node_text_size: u64) -> Vec<TextGroup> {
        let mut text_groups = Vec::new();
        let mut current_text = String::new();
        let mut current_metadata = HashMap::new();

        for line in text_lines {
            let (parsed_line, metadata, parsed_any_metadata) = ShinkaiFileParser::parse_and_extract_metadata(&line);

            if (parsed_line.len() as u64 + current_text.len() as u64) > max_node_text_size {
                if !current_text.is_empty() {
                    text_groups.push(
                        TextGroup::new(current_text.clone(), current_metadata.clone(), None)
                    );
                    current_text.clear();
                    current_metadata.clear();
                }
                if parsed_line.len() as u64 > max_node_text_size {
                    // If the line itself exceeds max_node_text_size, split it into chunks.
                    let chunks = if parsed_any_metadata {
                        ShinkaiFileParser::split_into_chunks_with_metadata(
                            &line,
                            max_node_text_size as usize
                        )
                    } else {
                        ShinkaiFileParser::split_into_chunks(&line, max_node_text_size as usize)
                    };

                    for chunk in chunks {
                        let (parsed_chunk, chunk_metadata, _) = if parsed_any_metadata {
                            ShinkaiFileParser::parse_and_extract_metadata(&chunk)
                        } else {
                            (chunk, HashMap::new(), false)
                        };
                        text_groups.push(TextGroup::new(parsed_chunk, chunk_metadata, None));
                    }
                } else {
                    current_text = parsed_line;
                    current_metadata.extend(metadata);
                }
            } else {
                if !current_text.is_empty() {
                    current_text.push(' '); // space between sentences
                }
                current_text.push_str(&parsed_line);
                current_metadata.extend(metadata);
            }
        }

        // Push the last segment
        if !current_text.is_empty() {
            text_groups.push(TextGroup::new(current_text, current_metadata.clone(), None));
        }

        text_groups
    }

    /// Given a piece of text, split it into a list of sentences, doing its best to respect punctuation
    /// and taking into account English-based exceptions.
    pub fn process_into_sentences(text: String) -> Vec<String> {
        let punctuation_marks = [
            ',', '.', ';', '-', '&', '(', '{', '<', '"', '\'', '`'
        ];
        text.split('\n')
            .filter(|line| !line.trim().is_empty() && line.trim().len() > 1)
            .flat_map(|line| {
                let trimmed_line = line.trim();

                let re = Regex::new(ShinkaiFileParser::PURE_METADATA_REGEX).unwrap();
                let is_pure_metadata = re.is_match(trimmed_line)
                    && re
                        .find(trimmed_line)
                        .map(|m| m.start() == 0 && m.end() == trimmed_line.len())
                        .unwrap_or(false);

                // Ensure each line ends with punctuation, or default to a newline
                let line_with_ending = if is_pure_metadata
                    || punctuation_marks
                        .iter()
                        .any(|&mark| trimmed_line.ends_with(mark))
                {
                    trimmed_line.to_string()
                } else {
                    format!("{}\n", trimmed_line)
                };

                Self::split_line_into_sentences(&line_with_ending)
            })
            .collect()
    }

    /// Splits a single line into sentences, considering common exceptions for English.
    fn split_line_into_sentences(line: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut start = 0;

        // Common abbreviations in lowercase
        let exceptions = [
            " mr.", " mrs.", " ms.", " dr.", " prof.", " gen.", " rep.", " sen.", " jr.", " sr.",
            " ave.", " blvd.", " st.", " rd.", " ln.", " ter.", " ct.", " pl.", " p.o.", " a.m.",
            " p.m.", " cm.", " kg.", " lb.", " oz.", " ft.", " in.", " mi.", " b.a.", " m.a.",
            " ph.d.", " m.d.", " b.sc.", " m.sc.", " inc.", " ltd.", " co.", " corp.", " llc.",
            " plc.", " et al.", " e.g.", " i.e.", " vs.", " viz.", " approx.", " dept.", " div.",
            " est.",
        ];

        for (index, _) in line.match_indices(". ") {
            let potential_end = index + 1;
            let sentence = &line[start..potential_end];
            let sentence_end_lc = sentence.to_lowercase();

            // Skip splitting if it matches an abbreviation
            if exceptions.iter().any(|&exc| sentence_end_lc.ends_with(exc)) {
                continue;
            }

            sentences.push(sentence.trim().to_string());
            start = potential_end + 1;
        }

        // Final leftover
        if start < line.len() {
            sentences.push(line[start..].trim().to_string());
        }

        sentences
    }
}
