use shinkai_vector_resources::file_parser::file_parser_types::TextGroup;

use crate::file_parser::PDFParser;

pub fn parse_pdf_from_file(file_path: &str, max_node_text_size: u64) -> anyhow::Result<Vec<TextGroup>> {
    let pdf_parser = PDFParser::new()?;
    let file_data = std::fs::read(file_path)?;

    pdf_parser.process_pdf_file(file_data, max_node_text_size)
}
