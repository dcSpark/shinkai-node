use crate::file_parser::file_parser_types::TextGroup;
use crate::resource_errors::VRError;
use crate::source::VRSourceReference;
use crate::vector_resource::DocumentFileType;
use crate::vector_resource::SourceFileType;
use crate::vector_resource::SourceReference;

pub struct LocalFileParser {}

impl LocalFileParser {
    /// Attempts to process a file into a list of TextGroups using local processing logic
    /// implemented in Rust directly without relying on external services.
    /// If local processing is not available for the provided source, then returns Err.
    pub fn process_file_into_grouped_text(
        file_buffer: Vec<u8>,
        file_name: String,
        max_node_text_size: u64,
        source: VRSourceReference,
    ) -> Result<Vec<TextGroup>, VRError> {
        let source_base = source;

        match &source_base {
            VRSourceReference::None => Err(VRError::UnsupportedFileType(file_name.to_string())),
            VRSourceReference::Standard(source) => match source {
                SourceReference::Other(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
                SourceReference::FileRef(file_source) => match file_source.clone().file_type {
                    SourceFileType::Image(_)
                    | SourceFileType::Code(_)
                    | SourceFileType::ConfigFileType(_)
                    | SourceFileType::Video(_)
                    | SourceFileType::Audio(_)
                    | SourceFileType::Shinkai(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
                    SourceFileType::Document(doc) => match doc {
                        DocumentFileType::Txt => LocalFileParser::process_txt_file(file_buffer, max_node_text_size),
                        DocumentFileType::Json => LocalFileParser::process_json_file(file_buffer, max_node_text_size),
                        DocumentFileType::Csv => LocalFileParser::process_csv_file(file_buffer, max_node_text_size),
                        DocumentFileType::Docx => LocalFileParser::process_docx_file(file_buffer, max_node_text_size),
                        DocumentFileType::Html => {
                            LocalFileParser::process_html_file(file_buffer, &file_name, max_node_text_size)
                        }

                        #[cfg(feature = "desktop-only")]
                        DocumentFileType::Md => LocalFileParser::process_md_file(file_buffer, max_node_text_size),

                        #[cfg(feature = "desktop-only")]
                        DocumentFileType::Pdf => LocalFileParser::process_pdf_file(file_buffer, max_node_text_size),

                        #[cfg(feature = "desktop-only")]
                        DocumentFileType::Xlsx | DocumentFileType::Xls => {
                            LocalFileParser::process_xlsx_file(file_buffer, max_node_text_size)
                        }

                        _ => Err(VRError::UnsupportedFileType(file_name.to_string())),
                    },
                },
                SourceReference::ExternalURI(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
            },
            VRSourceReference::Notarized(_) => Err(VRError::UnsupportedFileType(file_name.to_string())),
        }
    }
}
