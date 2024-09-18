use std::io::Cursor;

use docx_rust::{
    document::{BodyContent, ParagraphContent, TableCellContent, TableRowContent},
    formatting::JustificationVal,
    DocxFile,
};

use crate::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};

use super::LocalFileParser;

impl LocalFileParser {
    pub fn process_docx_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let docx = DocxFile::from_reader(Cursor::new(file_buffer)).map_err(|_| VRError::FailedDOCXParsing)?;
        let docx = docx.parse().map_err(|_| VRError::FailedDOCXParsing)?;

        let mut text_groups = Vec::new();
        let mut current_text = "".to_string();
        let mut heading_depth: usize = 0;

        docx.document.body.content.iter().for_each(|node| match node {
            BodyContent::Paragraph(paragraph) => {
                let text = paragraph.text();
                if text.is_empty() {
                    return;
                }

                let style = if let Some(property) = paragraph.property.as_ref() {
                    let style_value = if let Some(style_id) = property.style_id.as_ref() {
                        style_id.value.to_string()
                    } else {
                        "".to_string()
                    };

                    let is_centered = if let Some(justification) = property.justification.as_ref() {
                        matches!(justification.value, JustificationVal::Center)
                    } else {
                        false
                    };

                    let is_bold = if paragraph.content.iter().any(|content| match content {
                        ParagraphContent::Run(run) => run.property.as_ref().map_or(false, |p| p.bold.is_some()),
                        _ => false,
                    }) {
                        true
                    } else {
                        // Cloning r_pr because of pyo3 build error using as_ref()
                        property
                            .r_pr
                            .clone()
                            .into_iter()
                            .fold(false, |acc, p| acc || p.bold.is_some())
                    };

                    let has_size = if paragraph.content.iter().any(|content| match content {
                        ParagraphContent::Run(run) => run.property.as_ref().map_or(false, |p| p.size.is_some()),
                        _ => false,
                    }) {
                        true
                    } else {
                        // Cloning r_pr because of pyo3 build error using as_ref()
                        property
                            .r_pr
                            .clone()
                            .into_iter()
                            .fold(false, |acc, p| acc || p.size.is_some())
                    };

                    if style_value == "Title" || style_value.starts_with("Heading") {
                        style_value
                    } else {
                        let likely_heading = is_bold && has_size;
                        let likely_title = likely_heading && is_centered;

                        if likely_title {
                            "Title".to_string()
                        } else if likely_heading {
                            "Heading".to_string()
                        } else {
                            "".to_string()
                        }
                    }
                } else {
                    "".to_string()
                };

                if style == "Title" || style.starts_with("Heading") {
                    ShinkaiFileParser::push_text_group_by_depth(
                        &mut text_groups,
                        heading_depth,
                        current_text.clone(),
                        max_node_text_size,
                        None,
                    );
                    current_text = "".to_string();

                    if style == "Title" {
                        heading_depth = 0;
                    } else if style.starts_with("Heading") {
                        heading_depth = if heading_depth == 0 { 0 } else { 1 };
                    }

                    ShinkaiFileParser::push_text_group_by_depth(
                        &mut text_groups,
                        heading_depth,
                        text,
                        max_node_text_size,
                        None,
                    );
                    heading_depth += 1;
                    return;
                }

                let is_list_item = if let Some(property) = paragraph.property.as_ref() {
                    property.numbering.is_some()
                } else {
                    false
                };

                if is_list_item {
                    current_text.push_str(format!("\n- {}", text).as_str());
                } else {
                    ShinkaiFileParser::push_text_group_by_depth(
                        &mut text_groups,
                        heading_depth,
                        current_text.clone(),
                        max_node_text_size,
                        None,
                    );
                    current_text = text;
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

                ShinkaiFileParser::push_text_group_by_depth(
                    &mut text_groups,
                    heading_depth,
                    current_text.clone(),
                    max_node_text_size,
                    None,
                );
                current_text = "".to_string();

                let table_text = row_text.join("\n");
                ShinkaiFileParser::push_text_group_by_depth(
                    &mut text_groups,
                    heading_depth,
                    table_text,
                    max_node_text_size,
                    None,
                );
            }
            _ => {}
        });

        ShinkaiFileParser::push_text_group_by_depth(
            &mut text_groups,
            heading_depth,
            current_text,
            max_node_text_size,
            None,
        );

        Ok(text_groups)
    }
}
