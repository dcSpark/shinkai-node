/*

- takes a file (filepath)
- checks if it exists
- reads the filetype and redirects to the appropriate parser depending on the filetype
- it gets a vec of chunks (or another structure)
- it returns that

Use generator: &dyn EmbeddingGenerator for converting chunks to embeddings
also use the generator to know how big the chunks could be
*/

use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;

use crate::shinkai_fs_error::ShinkaiFsError;

use std::{fmt, fs};

use super::{local_parsing::LocalFileParser, text_group::TextGroup};

pub struct SimpleParser;

#[derive(Debug, PartialEq, Eq)]
enum SupportedFileType {
    Txt,
    Json,
    Csv,
    Html,
    Md,
    Pdf,
    Xlsx,
    Xls,
}

impl SupportedFileType {
    fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            "txt" => Some(SupportedFileType::Txt),
            "json" => Some(SupportedFileType::Json),
            "csv" => Some(SupportedFileType::Csv),
            "html" => Some(SupportedFileType::Html),
            "md" => Some(SupportedFileType::Md),
            "pdf" => Some(SupportedFileType::Pdf),
            "xlsx" => Some(SupportedFileType::Xlsx),
            "xls" => Some(SupportedFileType::Xls),
            _ => None,
        }
    }
}

impl fmt::Display for SupportedFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file_type_str = match self {
            SupportedFileType::Txt => "txt",
            SupportedFileType::Json => "json",
            SupportedFileType::Csv => "csv",
            SupportedFileType::Html => "html",
            SupportedFileType::Md => "md",
            SupportedFileType::Pdf => "pdf",
            SupportedFileType::Xlsx => "xlsx",
            SupportedFileType::Xls => "xls",
        };
        write!(f, "{}", file_type_str)
    }
}

impl SimpleParser {
    pub fn parse_file(filepath: ShinkaiPath, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        // check if file exists
        if !filepath.exists() {
            return Err(ShinkaiFsError::FileNotFoundWithPath(filepath.to_string()));
        }

        // extract file extension
        let extension = filepath.extension();

        if extension.is_none() {
            return Err(ShinkaiFsError::UnsupportedFileType(filepath.to_string()));
        }

        // check if the file extension is supported
        let file_type = SupportedFileType::from_extension(extension.unwrap())
            .ok_or_else(|| ShinkaiFsError::UnsupportedFileType(filepath.to_string()))?;

        // read file into memory
        let file_buffer = fs::read(&filepath.as_path()).map_err(|e| ShinkaiFsError::FailedIO(e.to_string()))?;

        // call the new function based on the file extension
        let text_groups = SimpleParser::process_file_by_extension(file_buffer, file_type, max_node_text_size)?;

        Ok(text_groups)
    }

    fn process_file_by_extension(
        file_buffer: Vec<u8>,
        file_type: SupportedFileType,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        match file_type {
            SupportedFileType::Txt => LocalFileParser::process_txt_file(file_buffer, max_node_text_size),
            SupportedFileType::Json => LocalFileParser::process_json_file(file_buffer, max_node_text_size),
            SupportedFileType::Csv => LocalFileParser::process_csv_file(file_buffer, max_node_text_size),
            SupportedFileType::Html => LocalFileParser::process_html_file(file_buffer, "filename", max_node_text_size),
            SupportedFileType::Md => LocalFileParser::process_md_file(file_buffer, max_node_text_size),
            SupportedFileType::Pdf => LocalFileParser::process_pdf_file(file_buffer, max_node_text_size),
            _ => Err(ShinkaiFsError::UnsupportedFileType(file_type.to_string())),
            // SupportedFileType::Xlsx | SupportedFileType::Xls => LocalFileParser::process_xlsx_file(file_buffer, max_node_text_size),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_parse_csv_file() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let shinkai_path = ShinkaiPath::from_string("test.csv".to_string());

        // Write a simple CSV content to the file
        let mut file = fs::File::create(&shinkai_path.as_path()).unwrap();
        writeln!(file, "header1,header2").unwrap();
        writeln!(file, "value1,value2").unwrap();

        // Call the parse_file function
        let result = SimpleParser::parse_file(shinkai_path, 1024);

        // Assert the result is Ok and contains expected data
        assert!(result.is_ok());
        let text_groups = result.unwrap();
        assert!(!text_groups.is_empty());
    }

    #[test]
    fn test_parse_large_csv_file() {
        let _dir = testing_create_tempdir_and_set_env_var();

        // Create a ShinkaiPath directly
        let shinkai_path = ShinkaiPath::from_string("large_test.csv".to_string());

        // Write a larger CSV content to the file
        let mut file = fs::File::create(&shinkai_path.as_path()).unwrap();
        writeln!(file, "header1,header2,header3").unwrap();
        for i in 0..100 {
            writeln!(file, "value1_{},value2_{},value3_{}", i, i, i).unwrap();
        }

        // Call the parse_file function with a smaller max_node_text_size
        let result = SimpleParser::parse_file(shinkai_path, 20);

        // Assert the result is Ok and contains expected data
        assert!(result.is_ok());
        let text_groups = result.unwrap();

        eprintln!("length: {:?}", text_groups.len());

        assert!(!text_groups.is_empty());

        // No need to manually close _dir as it will be automatically cleaned up
    }
}
