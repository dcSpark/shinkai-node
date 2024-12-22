use super::LocalFileParser;
use crate::{file_parser::file_parser_types::TextGroup, shinkai_fs_error::ShinkaiFsError};
use csv::ReaderBuilder;
use std::io::Cursor;

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
}
