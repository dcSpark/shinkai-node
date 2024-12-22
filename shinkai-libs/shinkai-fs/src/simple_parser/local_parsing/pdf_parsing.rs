use crate::{shinkai_fs_error::ShinkaiFsError, simple_parser::{file_parser_helper::ShinkaiFileParser, text_group::TextGroup}};

use super::LocalFileParser;

impl LocalFileParser {
    pub fn process_pdf_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        use shinkai_ocr::pdf_parser::PDFParser;

        let pdf_parser = PDFParser::new().map_err(|_| ShinkaiFsError::FailedPDFParsing)?;
        let parsed_pages = pdf_parser
            .process_pdf_file(file_buffer)
            .map_err(|_| ShinkaiFsError::FailedPDFParsing)?;

        let mut text_groups = Vec::new();

        for page in parsed_pages.into_iter() {
            for pdf_text in page.content.into_iter() {
                ShinkaiFileParser::push_text_group_by_depth(
                    &mut text_groups,
                    0,
                    pdf_text.text,
                    max_node_text_size,
                    Some(page.page_number.try_into().unwrap_or_default()),
                );
            }
        }

        Ok(text_groups)
    }
}
