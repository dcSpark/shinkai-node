use shinkai_ocr::image_parser::ImageParser;

use crate::{
    file_parser::{file_parser::ShinkaiFileParser, file_parser_types::TextGroup},
    resource_errors::VRError,
};

use super::LocalFileParser;

impl LocalFileParser {
    pub fn process_image_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, VRError> {
        let image_parser = ImageParser::new().map_err(|_| VRError::FailedImageParsing)?;
        let text = image_parser
            .process_image_file(file_buffer)
            .map_err(|_| VRError::FailedImageParsing)?;

        let text_groups = ShinkaiFileParser::parse_and_split_into_text_groups(text, max_node_text_size);

        Ok(text_groups)
    }
}
