use std::{collections::HashMap, env, path::PathBuf};

use serde_json::json;
use shinkai_tools_runner::tools::{
    code_files::CodeFiles, deno_runner::DenoRunner, deno_runner_options::DenoRunnerOptions, execution_context::ExecutionContext, runner_type::RunnerType
};

use crate::shinkai_fs_error::ShinkaiFsError;
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
                    .unwrap_or_else(|_| "/opt/homebrew/bin/deno".to_string()),
            ),
            context: ExecutionContext {
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
                .map(|cell| cell.as_str().unwrap_or("").to_string())
                .collect())
        })
        .collect::<Result<Vec<Vec<String>>, ShinkaiFsError>>()?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use std::path::{self, Path};

    use super::*;

    #[tokio::test]
    async fn test_parse_xlsx_to_matrix() {
        let xlsx_file_path = path::absolute(Path::new("./src/test_data/test.xlsx"))
            .unwrap()
            .to_path_buf();
        let rows = parse_xlsx_to_matrix(xlsx_file_path).await.unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].len(), 2);
    }
}
