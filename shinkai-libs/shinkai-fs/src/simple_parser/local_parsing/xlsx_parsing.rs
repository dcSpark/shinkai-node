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
        let xls_result = Xls::new(cursor).and_then(|mut workbook| {
            if let Some(sheet_name) = workbook.sheet_names().first().cloned() {
                let range = workbook.worksheet_range(&sheet_name)?;
                
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
        });
        
        if xls_result.is_err() {
            let cursor = Cursor::new(buffer);
            return Xlsx::new(cursor)
                .map_err(|_| ShinkaiFsError::FailedXLSParsing)
                .and_then(|mut workbook| {
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
                });
        }
        
        xls_result.map_err(|_| ShinkaiFsError::FailedXLSParsing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;
    use std::fs;
    use std::path::Path;

    fn read_test_file(filename: &str) -> Vec<u8> {
        let path = Path::new("src/test_data").join(filename);
        fs::read(path).expect(&format!("Failed to read test file: {}", filename))
    }

    #[test]
    fn test_process_xlsx_file() {
        let _dir = testing_create_tempdir_and_set_env_var();
        
        let buffer = read_test_file("test.xlsx");
        let max_node_text_size = 1024;
        
        let result = LocalFileParser::process_xlsx_file(buffer, max_node_text_size);
        
        assert!(result.is_ok());
        let text_groups = result.unwrap();
        
        assert!(!text_groups.is_empty());
        
        let combined_text = text_groups.iter().map(|tg| tg.text.as_str()).collect::<Vec<_>>().join("\n");
        assert!(combined_text.contains("header1|header2"));
        assert!(combined_text.contains("value1|value2"));
        assert!(combined_text.contains("value3|value4"));
        assert!(combined_text.contains("value5|value6"));
    }

    #[test]
    fn test_process_xls_file() {
        let _dir = testing_create_tempdir_and_set_env_var();
        
        let buffer = read_test_file("test.xls");
        let max_node_text_size = 1024;
        
        let result = LocalFileParser::process_xls_file(buffer, max_node_text_size);
        
        assert!(result.is_ok());
        let text_groups = result.unwrap();
        
        assert!(!text_groups.is_empty());
        
        let combined_text = text_groups.iter().map(|tg| tg.text.as_str()).collect::<Vec<_>>().join("\n");
        assert!(combined_text.contains("header1|header2"));
        assert!(combined_text.contains("value1|value2"));
        assert!(combined_text.contains("value3|value4"));
        assert!(combined_text.contains("value5|value6"));
    }

    #[test]
    fn test_parse_xlsx() {
        let buffer = read_test_file("test.xlsx");
        
        let result = LocalFileParser::parse_xlsx(&buffer);
        
        assert!(result.is_ok());
        let lines = result.unwrap();
        
        assert_eq!(lines.len(), 4); // Header + 3 data rows
        
        assert_eq!(lines[0], "header1|header2");
        assert_eq!(lines[1], "value1|value2");
        assert_eq!(lines[2], "value3|value4");
        assert_eq!(lines[3], "value5|value6");
    }

    #[test]
    fn test_parse_xls() {
        let buffer = read_test_file("test.xls");
        
        let result = LocalFileParser::parse_xls(&buffer);
        
        assert!(result.is_ok());
        let lines = result.unwrap();
        
        assert_eq!(lines.len(), 4); // Header + 3 data rows
        
        assert_eq!(lines[0], "header1|header2");
        assert_eq!(lines[1], "value1|value2");
        assert_eq!(lines[2], "value3|value4");
        assert_eq!(lines[3], "value5|value6");
    }

    #[test]
    fn test_empty_buffer() {
        let buffer = Vec::new();
        let max_node_text_size = 1024;
        
        let result_xlsx = LocalFileParser::process_xlsx_file(buffer.clone(), max_node_text_size);
        let result_xls = LocalFileParser::process_xls_file(buffer, max_node_text_size);
        
        assert!(result_xlsx.is_err());
        assert!(result_xls.is_err());
    }
}
