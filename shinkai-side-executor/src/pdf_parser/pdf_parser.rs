use std::collections::HashMap;

use pdfium_render::prelude::*;
use shinkai_vector_resources::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};

pub struct PDFParser {
    pdfium: Pdfium,
}

impl PDFParser {
    pub fn new() -> Self {
        PDFParser {
            pdfium: Pdfium::new(Pdfium::bind_to_statically_linked_library().unwrap()),
        }
    }

    pub fn process_pdf_file(&self, file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let document = self
            .pdfium
            .load_pdf_from_byte_vec(file_buffer, None)
            .map_err(|_| VRError::FailedPDFParsing)?;

        let mut text_groups = Vec::new();

        for (page_index, page) in document.pages().iter().enumerate() {
            for object in page.objects().iter() {
                match object.object_type() {
                    PdfPageObjectType::Text => {
                        let text_object = object.as_text_object().unwrap();
                        let text = text_object.text();
                        let (parsed_text, metadata, parsed_any_metadata) =
                            ShinkaiFileParser::parse_and_extract_metadata(&text);

                        if parsed_text.len() as u64 > max_node_text_size {
                            let chunks = if parsed_any_metadata {
                                ShinkaiFileParser::split_into_chunks_with_metadata(&text, max_node_text_size as usize)
                            } else {
                                ShinkaiFileParser::split_into_chunks(&text, max_node_text_size as usize)
                            };

                            for chunk in chunks {
                                let (parsed_chunk, metadata, _) = ShinkaiFileParser::parse_and_extract_metadata(&chunk);

                                let metadata: HashMap<String, String> = {
                                    let mut map = metadata.clone();
                                    map.insert(
                                        ShinkaiFileParser::page_numbers_metadata_key(),
                                        format!("[{}]", page_index + 1),
                                    );
                                    map
                                };
                                text_groups.push(TextGroup::new(parsed_chunk, metadata, vec![], None));
                            }
                        } else {
                            let metadata: HashMap<String, String> = {
                                let mut map = metadata.clone();
                                map.insert(
                                    ShinkaiFileParser::page_numbers_metadata_key(),
                                    format!("[{}]", page_index + 1),
                                );
                                map
                            };
                            text_groups.push(TextGroup::new(parsed_text, metadata, vec![], None));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(text_groups)
    }
}
