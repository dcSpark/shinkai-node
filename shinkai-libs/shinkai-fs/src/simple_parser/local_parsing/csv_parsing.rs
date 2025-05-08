use crate::{
    shinkai_fs_error::ShinkaiFsError, simple_parser::{file_parser_helper::ShinkaiFileParser, text_group::TextGroup}
};

use csv::ReaderBuilder;
use std::{collections::HashMap, io::Cursor};

use super::LocalFileParser;

impl LocalFileParser {
    /// Attempts to process the provided csv file into a list of TextGroups.
    pub fn process_csv_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let csv_lines = Self::parse_csv_auto(&file_buffer).map_err(|_| ShinkaiFsError::FailedCSVParsing)?;
        Self::process_table_rows(csv_lines, max_node_text_size)
    }

    // /// Parse CSV data from a buffer and attempt to automatically detect
    // /// headers.
    pub fn parse_csv_auto(buffer: &[u8]) -> Result<Vec<String>, ShinkaiFsError> {
        let mut reader = ReaderBuilder::new().flexible(true).from_reader(Cursor::new(buffer));
        let headers = reader
            .headers()
            .map_err(|_| ShinkaiFsError::FailedCSVParsing)?
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
    pub fn parse_csv(buffer: &[u8], header: bool) -> Result<Vec<String>, ShinkaiFsError> {
        let mut reader = ReaderBuilder::new()
            .flexible(true)
            .has_headers(header)
            .from_reader(Cursor::new(buffer));
        let headers = if header {
            reader
                .headers()
                .map_err(|_| ShinkaiFsError::FailedCSVParsing)?
                .iter()
                .map(String::from)
                .collect::<Vec<String>>()
        } else {
            Vec::new()
        };

        let mut result = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|_| ShinkaiFsError::FailedCSVParsing)?;
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

    pub fn process_table_rows(
        table_rows: Vec<String>,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let mut table_rows_split = Vec::new();
        let mut current_group = Vec::new();
        let mut current_length = 0;

        for row in table_rows {
            let line_length = row.len() as u64;
            if current_length + line_length > max_node_text_size {
                if !current_group.is_empty() {
                    table_rows_split.push(current_group);
                }
                current_group = Vec::new();
                current_length = 0;
            }
            current_group.push(row);
            current_length += line_length;
        }

        if !current_group.is_empty() {
            table_rows_split.push(current_group);
        }

        let joined_lines = table_rows_split
            .into_iter()
            .map(|group| group.join("\n"))
            .collect::<Vec<String>>();

        let mut text_groups = Vec::new();
        for line in joined_lines {
            let (parsed_line, metadata, parsed_any_metadata) = ShinkaiFileParser::parse_and_extract_metadata(&line);

            if parsed_line.len() as u64 > max_node_text_size {
                // Instead of sub-groups, just create multiple TextGroups:
                let chunks = if parsed_any_metadata {
                    ShinkaiFileParser::split_into_chunks_with_metadata(&line, max_node_text_size as usize)
                } else {
                    ShinkaiFileParser::split_into_chunks(&line, max_node_text_size as usize)
                };

                for chunk in chunks {
                    let (parsed_chunk, chunk_metadata, _) = if parsed_any_metadata {
                        ShinkaiFileParser::parse_and_extract_metadata(&chunk)
                    } else {
                        (chunk.to_owned(), HashMap::new(), false)
                    };
                    text_groups.push(TextGroup::new(parsed_chunk, chunk_metadata, None));
                }
            } else if !parsed_line.is_empty() {
                text_groups.push(TextGroup::new(parsed_line, metadata, None));
            }
        }
        Ok(text_groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_csv_file() {
        // Sample CSV data
        let csv_data = b"header1,header2\nvalue1,value2\nvalue3,value4";
        let max_node_text_size = 10;

        // Call the function
        let result = LocalFileParser::process_csv_file(csv_data.to_vec(), max_node_text_size);
        eprintln!("result: {:?}", result);

        // Check the result
        assert!(result.is_ok());
        let text_groups = result.unwrap();

        // Verify the output
        assert_eq!(text_groups.len(), 6); // Expecting 6 TextGroups due to max_node_text_size
        let expected_texts = vec!["header1|he", "ader2", "value1|val", "ue2", "value3|val", "ue4"];
        for (i, text_group) in text_groups.iter().enumerate() {
            assert_eq!(text_group.text, expected_texts[i]);
        }
    }
}
