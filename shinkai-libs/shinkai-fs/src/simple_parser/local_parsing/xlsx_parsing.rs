use serde_json::{json, Value};
use shinkai_tools_runner::tools::{
    code_files::CodeFiles, deno_runner::DenoRunner, deno_runner_options::DenoRunnerOptions, execution_context::ExecutionContext, runner_type::RunnerType
};

use crate::{shinkai_fs_error::ShinkaiFsError, simple_parser::text_group::TextGroup};

use std::{
    collections::HashMap, env, path::{Path, PathBuf}
};

use super::LocalFileParser;

pub async fn parse_xlsx_to_matrix(xlsx_file_path: PathBuf) -> Result<Vec<Vec<String>>, ShinkaiFsError> {
    println!("parsing XLSX file: {:?}", xlsx_file_path);
    let code_files = CodeFiles {
        files: HashMap::from([(
            "main.ts".to_string(),
            r#"
            // @deno-types="https://cdn.sheetjs.com/xlsx-0.20.3/package/types/index.d.ts"
            import * as XLSX from 'https://cdn.sheetjs.com/xlsx-0.20.3/package/xlsx.mjs';

            async function run(configurations, params) {
                console.log(params.file);
                const workbook = XLSX.read(params.file, {type: 'file'});
                const firstSheetName = workbook.SheetNames[0];
                const worksheet = workbook.Sheets[firstSheetName];
                const rows = XLSX.utils.sheet_to_json(worksheet, { header: 1, defval: null});
                console.log("Sheet name: ", firstSheetName);
                return {
                    rows
                };
            }
            "#
            .to_string(),
        )]),
        entrypoint: "main.ts".to_string(),
    };
    let deno_runner = DenoRunner::new(
        code_files,
        json!({}),
        Some(DenoRunnerOptions {
            deno_binary_path: PathBuf::from(
                env::var("SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH")
                    .unwrap_or_else(|_| "./shinkai-tools-runner-resources/deno".to_string()),
            ),
            context: ExecutionContext {
                storage: PathBuf::from(env::var("NODE_STORAGE_PATH").unwrap_or_else(|_| "./".to_string()))
                    .join("internal_tools_storage"),
                context_id: "shinkai-node-xlsx-parsing".to_string(),
                mount_files: vec![xlsx_file_path.clone()],
                ..Default::default()
            },
            force_runner_type: Some(RunnerType::Host),
            ..Default::default()
        }),
    );

    let result = deno_runner
        .run(
            None,
            json!({
                "file": xlsx_file_path.to_str().unwrap(),
            }),
            None,
        )
        .await
        .inspect_err(|e| {
            println!("Error: {:?}", e);
        })
        .map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;

    let rows: Vec<Vec<String>> = result.data["rows"]
        .as_array()
        .ok_or(ShinkaiFsError::FailedXLSXParsing)?
        .iter()
        .map(|row| -> Result<Vec<String>, ShinkaiFsError> {
            Ok(row
                .as_array()
                .ok_or(ShinkaiFsError::FailedXLSXParsing)?
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

    Ok(rows)
}

impl LocalFileParser {
    pub async fn parse_xlsx(file_path: PathBuf) -> Result<Vec<String>, ShinkaiFsError> {
        let xlsx_lines = parse_xlsx_to_matrix(file_path)
            .await
            .map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;
        let parsed_lines = xlsx_lines.into_iter().map(|row| row.join("|")).collect::<Vec<String>>();
        Ok(parsed_lines)
    }

    pub async fn process_xlsx_file(
        file_path: PathBuf,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let xlsx_lines = Self::parse_xlsx(file_path)
            .await
            .map_err(|_| ShinkaiFsError::FailedXLSXParsing)?;
        println!("xlsx_lines: {:?}", xlsx_lines[0]);
        Self::process_table_rows(xlsx_lines, max_node_text_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;
    use std::path;
    use std::path::Path;

    #[tokio::test]
    async fn test_parse_xlsx_to_matrix() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("./src/test_data/test.xlsx"))
            .unwrap()
            .to_path_buf();
        let rows = parse_xlsx_to_matrix(xlsx_file_path).await.unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].len(), 2);
    }

    #[tokio::test]
    async fn test_parse_xls_to_matrix() {
        let xlsx_file_path = path::absolute(Path::new("./src/test_data/test.xls"))
            .unwrap()
            .to_path_buf();
        let rows = parse_xlsx_to_matrix(xlsx_file_path).await.unwrap();
        assert_eq!(rows.len(), 11);
        assert_eq!(rows[0].len(), 8);
    }

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
