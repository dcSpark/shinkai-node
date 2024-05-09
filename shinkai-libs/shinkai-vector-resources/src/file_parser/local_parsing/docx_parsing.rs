use std::io::Cursor;

use docx_rust::{
    document::{BodyContent, TableCellContent, TableRowContent},
    DocxFile,
};

use crate::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};

use super::LocalFileParser;

impl LocalFileParser {
    pub fn process_docx_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let docx = DocxFile::from_reader(Cursor::new(file_buffer)).map_err(|_| VRError::FailedCSVParsing)?;
        let docx = docx.parse().map_err(|_| VRError::FailedCSVParsing)?;

        let mut text_groups = Vec::new();

        docx.document.body.content.iter().for_each(|node| match node {
            BodyContent::Paragraph(paragraph) => {
                let text = paragraph.text();
                if !text.is_empty() {
                    let paragraph_groups =
                        ShinkaiFileParser::parse_and_split_into_text_groups(text, max_node_text_size);
                    text_groups.extend(paragraph_groups);
                }
            }
            BodyContent::Table(table) => {
                let mut row_text = Vec::new();
                table.rows.iter().for_each(|row| {
                    let mut cell_text = Vec::new();

                    row.cells.iter().for_each(|cell| match cell {
                        TableRowContent::TableCell(cell) => {
                            cell.content.iter().for_each(|content| match content {
                                TableCellContent::Paragraph(paragraph) => {
                                    let text = paragraph.text();
                                    cell_text.push(text);
                                }
                            });
                        }
                        _ => {}
                    });

                    row_text.push(cell_text.join("; "));
                });

                let table_text = row_text.join("\n");
                text_groups.extend(ShinkaiFileParser::parse_and_split_into_text_groups(
                    table_text,
                    max_node_text_size,
                ));
            }
            _ => {}
        });

        Ok(text_groups)
    }
}
