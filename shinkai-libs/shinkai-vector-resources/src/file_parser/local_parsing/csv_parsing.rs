use super::LocalFileParser;
use crate::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};
use csv::ReaderBuilder;
use std::{collections::HashMap, io::Cursor};

impl LocalFileParser {
    /// Attempts to process the provided csv file into a list of TextGroups.
    pub fn process_csv_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let csv_lines = Self::parse_csv_auto(&file_buffer).map_err(|_| VRError::FailedCSVParsing)?;

        // Join as many lines as possible into a text group.
        let mut csv_lines_split = Vec::new();
        let mut current_group = Vec::new();
        let mut current_length = 0;

        for line in csv_lines {
            let line_length = line.len() as u64;
            if current_length + line_length > max_node_text_size {
                csv_lines_split.push(current_group);
                current_group = Vec::new();
                current_length = 0;
            }
            current_group.push(line);
            current_length += line_length;
        }

        if !current_group.is_empty() {
            csv_lines_split.push(current_group);
        }

        let joined_lines = csv_lines_split
            .into_iter()
            .map(|group| group.join("\n"))
            .collect::<Vec<String>>();

        let mut text_groups = Vec::new();
        for line in joined_lines {
            let (parsed_line, metadata, parsed_any_metadata) = ShinkaiFileParser::parse_and_extract_metadata(&line);

            if parsed_line.len() as u64 > max_node_text_size {
                // If the line itself exceeds max_node_text_size, split it into chunks
                // Split the unparsed line into chunks and parse metadata in each chunk
                let chunks = if parsed_any_metadata {
                    ShinkaiFileParser::split_into_chunks_with_metadata(&line, max_node_text_size as usize)
                } else {
                    ShinkaiFileParser::split_into_chunks(&line, max_node_text_size as usize)
                };

                if let Some(first_chunk) = chunks.first() {
                    let (parsed_chunk, metadata, _) = if parsed_any_metadata {
                        ShinkaiFileParser::parse_and_extract_metadata(&first_chunk)
                    } else {
                        (first_chunk.to_owned(), HashMap::new(), false)
                    };

                    let mut line_group = TextGroup::new(parsed_chunk, metadata, vec![], None);

                    if chunks.len() > 1 {
                        for chunk in chunks.into_iter().skip(1) {
                            let (parsed_chunk, metadata, _) = if parsed_any_metadata {
                                ShinkaiFileParser::parse_and_extract_metadata(&chunk)
                            } else {
                                (chunk.to_owned(), HashMap::new(), false)
                            };

                            line_group.push_sub_group(TextGroup::new(parsed_chunk, metadata, vec![], None));
                        }
                    }

                    text_groups.push(line_group);
                }
            } else {
                text_groups.push(TextGroup::new(parsed_line, metadata, vec![], None));
            }
        }

        Ok(text_groups)
    }

    // /// Parse CSV data from a buffer and attempt to automatically detect
    // /// headers.
    pub fn parse_csv_auto(buffer: &[u8]) -> Result<Vec<String>, VRError> {
        let mut reader = ReaderBuilder::new().flexible(true).from_reader(Cursor::new(buffer));
        let headers = reader
            .headers()
            .map_err(|_| VRError::FailedCSVParsing)?
            .iter()
            .map(String::from)
            .collect::<Vec<String>>();

        let likely_header = headers.iter().all(|s| {
            let is_alphabetic = s.chars().all(|c| c.is_alphabetic() || c.is_whitespace() || c == '_');
            let no_duplicates = headers.iter().filter(|&item| item == s).count() == 1;
            let no_prohibited_chars = !s.contains(&['@', '#', '$', '%', '^', '&', '*']);

            is_alphabetic && no_duplicates && no_prohibited_chars
        });

        Self::parse_csv(&buffer, likely_header)
    }

    // /// Parse CSV data from a buffer.
    // /// * `header` - A boolean indicating whether to prepend column headers to
    // ///   values.
    pub fn parse_csv(buffer: &[u8], header: bool) -> Result<Vec<String>, VRError> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(header)
            .from_reader(Cursor::new(buffer));
        let headers = if header {
            reader
                .headers()
                .map_err(|_| VRError::FailedCSVParsing)?
                .iter()
                .map(String::from)
                .collect::<Vec<String>>()
        } else {
            Vec::new()
        };

        let mut result = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|_| VRError::FailedCSVParsing)?;
            let row: Vec<String> = if header {
                record
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}: {}", headers[i], e))
                    .collect()
            } else {
                record.iter().map(String::from).collect()
            };
            let row_string = row.join("|");
            result.push(row_string);
        }

        Ok(result)
    }
}
