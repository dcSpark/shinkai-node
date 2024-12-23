use num_traits::cast::ToPrimitive;
use std::{collections::HashMap, io::Cursor};

use crate::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};

use super::LocalFileParser;

impl LocalFileParser {
    
    pub fn process_xlsx_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let spreadsheet = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(file_buffer), true)
            .map_err(|_| VRError::FailedXLSXParsing)?;

        let mut table_rows = Vec::new();
        if let Some(worksheet) = spreadsheet.get_sheet(&0) {
            for row_index in 1..u32::MAX {
                let row_cells = worksheet.get_collection_by_row(&row_index);
                let is_empty_row =
                    row_cells.is_empty() || row_cells.iter().all(|cell| cell.get_cell_value().is_empty());

                if is_empty_row {
                    break;
                }

                let mut cell_values = Vec::new();
                let num_columns = row_cells.len();
                for col_index in 1..=num_columns {
                    if let Some(cell) = worksheet.get_cell((col_index.to_u32().unwrap_or_default(), row_index)) {
                        let cell_value = cell.get_value().to_string();
                        cell_values.push(cell_value);
                    }
                }

                let row_string = cell_values.join("|");

                table_rows.push(row_string);
            }
        }

        Self::process_table_rows(table_rows, max_node_text_size)
    }

    pub fn process_table_rows(table_rows: Vec<String>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        // Join as many rows as possible into a text group.
        let mut table_rows_split = Vec::new();
        let mut current_group = Vec::new();
        let mut current_length = 0;

        for row in table_rows {
            let line_length = row.len() as u64;
            if current_length + line_length > max_node_text_size {
                table_rows_split.push(current_group);
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
}
