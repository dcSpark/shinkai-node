use serde_json::Value;
use shinkai_non_rust_code::functions::parse_xlsx::parse_xlsx;

use crate::{shinkai_fs_error::ShinkaiFsError, simple_parser::text_group::TextGroup};

use std::path::PathBuf;

use super::LocalFileParser;

impl LocalFileParser {
    pub async fn parse_xlsx(file_path: PathBuf) -> Result<Vec<String>, ShinkaiFsError> {
        let parsed_xlsx = parse_xlsx(file_path)
            .await
            .map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;

        let parsed_xlsx: Vec<Vec<String>> = parsed_xlsx
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        Value::String(s) => s.to_string(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => "".to_string(),
                    })
                    .collect::<Vec<String>>()
            })
            .collect();

        let parsed_lines = parsed_xlsx
            .into_iter()
            .map(|row| row.join("|"))
            .collect::<Vec<String>>();
        Ok(parsed_lines)
    }

    pub async fn process_xlsx_file(
        file_path: PathBuf,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let parsed_xls = parse_xlsx(file_path)
            .await
            .map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;
        let parsed_xls: Vec<Vec<String>> = parsed_xls
            .rows
            .iter()
            .map(|row| -> Result<Vec<String>, ShinkaiFsError> {
                Ok(row
                    .iter()
                    .map(|cell| match cell {
                        Value::String(s) => s.to_string(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => "".to_string(),
                    })
                    .collect())
            })
            .collect::<Result<Vec<Vec<String>>, ShinkaiFsError>>()?;
        let parsed_lines = parsed_xls.into_iter().map(|row| row.join("|")).collect::<Vec<String>>();
        Self::process_table_rows(parsed_lines, max_node_text_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;
    use std::path;
    use std::path::Path;

    #[tokio::test]
    async fn test_process_xlsx_file() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("./src/test_data/test.xlsx"))
            .unwrap()
            .to_path_buf();
        let max_node_text_size = 1024;

        let result = LocalFileParser::process_xlsx_file(xlsx_file_path, max_node_text_size).await;

        assert!(result.is_ok());
        let text_groups = result.unwrap();

        assert!(!text_groups.is_empty());

        let combined_text = text_groups
            .iter()
            .map(|tg| tg.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(combined_text.contains("header1|header2"));
        assert!(combined_text.contains("value1|value2"));
        assert!(combined_text.contains("value3|value4"));
        assert!(combined_text.contains("value5|value6"));
    }

    #[tokio::test]
    async fn test_process_xls_file() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("./src/test_data/test.xls"))
            .unwrap()
            .to_path_buf();
        let max_node_text_size = 1024;

        let result = LocalFileParser::process_xlsx_file(xlsx_file_path, max_node_text_size).await;

        assert!(result.is_ok());
        let text_groups = result.unwrap();

        assert!(!text_groups.is_empty());

        let combined_text = text_groups
            .iter()
            .map(|tg| tg.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(combined_text.contains("0|First Name|Last Name|Gender|Country|Age|Date|Id"));
        assert!(combined_text.contains("1|Dulce|Abril|Female|United States|32|15/10/2017|1562"));
    }

    #[tokio::test]
    async fn test_parse_xlsx() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("./src/test_data/test.xlsx"))
            .unwrap()
            .to_path_buf();
        let result = LocalFileParser::parse_xlsx(xlsx_file_path).await;

        assert!(result.is_ok());
        let lines = result.unwrap();

        assert_eq!(lines.len(), 4); // Header + 3 data rows

        assert_eq!(lines[0], "header1|header2");
        assert_eq!(lines[1], "value1|value2");
        assert_eq!(lines[2], "value3|value4");
        assert_eq!(lines[3], "value5|value6");
    }

    #[tokio::test]
    async fn test_parse_xls() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("./src/test_data/test.xls"))
            .unwrap()
            .to_path_buf();

        let result = LocalFileParser::parse_xlsx(xlsx_file_path).await;

        assert!(result.is_ok());
        let lines = result.unwrap();

        assert_eq!(lines.len(), 11); // Header + 3 data rows

        assert_eq!(lines[0], "0|First Name|Last Name|Gender|Country|Age|Date|Id");
        assert_eq!(lines[8], "8|Earlean|Melgar|Female|United States|27|16/08/2016|2456");
    }

    #[tokio::test]
    async fn test_empty_file() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("./src/test_data/potato.xlsx"))
            .unwrap()
            .to_path_buf();
        let xls_file_path = path::absolute(Path::new("./src/test_data/potato.xls"))
            .unwrap()
            .to_path_buf();
        let max_node_text_size = 1024;

        let result_xlsx = LocalFileParser::process_xlsx_file(xlsx_file_path.clone(), max_node_text_size).await;
        let result_xls = LocalFileParser::process_xlsx_file(xls_file_path, max_node_text_size).await;

        assert!(result_xlsx.is_err());
        assert!(result_xls.is_err());
    }
}
