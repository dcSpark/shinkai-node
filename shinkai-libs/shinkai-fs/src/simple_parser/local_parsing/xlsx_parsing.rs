use crate::{
    shinkai_fs_error::ShinkaiFsError,
    simple_parser::text_group::TextGroup,
};

use calamine::{Reader, Xlsx, Xls};
use std::io::Cursor;

use super::LocalFileParser;

impl LocalFileParser {
    pub fn process_xlsx_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let xlsx_lines = Self::parse_xlsx(&file_buffer).map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;
        Self::process_table_rows(xlsx_lines, max_node_text_size)
    }

    pub fn process_xls_file(file_buffer: Vec<u8>, max_node_text_size: u64) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let xls_lines = Self::parse_xls(&file_buffer).map_err(|_| ShinkaiFsError::FailedXLSParsing)?;
        Self::process_table_rows(xls_lines, max_node_text_size)
    }

    fn parse_xlsx(buffer: &[u8]) -> Result<Vec<String>, ShinkaiFsError> {
        let cursor = Cursor::new(buffer);
        let mut workbook = Xlsx::new(cursor).map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;
        
        if let Some(sheet_name) = workbook.sheet_names().first().cloned() {
            let range = workbook.worksheet_range(&sheet_name)
                .map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;
            
            let mut result = Vec::new();
            for row in range.rows() {
                let row_string = row.iter()
                    .map(|cell| cell.to_string())
                    .collect::<Vec<_>>()
                    .join("|");
                
                if !row_string.is_empty() {
                    result.push(row_string);
                }
            }
            
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    fn parse_xls(buffer: &[u8]) -> Result<Vec<String>, ShinkaiFsError> {
        let cursor = Cursor::new(buffer);
        let mut workbook = Xls::new(cursor).map_err(|_| ShinkaiFsError::FailedXLSParsing)?;
        
        if let Some(sheet_name) = workbook.sheet_names().first().cloned() {
            let range = workbook.worksheet_range(&sheet_name)
                .map_err(|_| ShinkaiFsError::FailedXLSParsing)?;
            
            let mut result = Vec::new();
            for row in range.rows() {
                let row_string = row.iter()
                    .map(|cell| cell.to_string())
                    .collect::<Vec<_>>()
                    .join("|");
                
                if !row_string.is_empty() {
                    result.push(row_string);
                }
            }
            
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    fn create_mock_buffer() -> Vec<u8> {
        Vec::new()
    }

    #[test]
    #[ignore] // Ignore this test as it requires an actual XLS file
    fn test_process_xls_file() {
        let buffer = create_mock_buffer();
        let max_node_text_size = 1024;
        
        let result = LocalFileParser::process_xls_file(buffer, max_node_text_size);
        assert!(result.is_err());
    }
}
