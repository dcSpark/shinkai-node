use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Debug, Serialize)]
pub struct Input {
    file_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub rows: Vec<Vec<Value>>,
}

pub async fn parse_xlsx(file_path: PathBuf) -> Result<Output, RunError> {
    println!("parsing xlsx file: {:?}", file_path);
    let code = r#"
            // @deno-types="https://cdn.sheetjs.com/xlsx-0.20.3/package/types/index.d.ts"
            import * as XLSX from 'https://cdn.sheetjs.com/xlsx-0.20.3/package/xlsx.mjs';

            async function run(configurations, params) {
                console.log(params.file_path);
                const workbook = XLSX.read(params.file_path, {type: 'file'});
                const firstSheetName = workbook.SheetNames[0];
                const worksheet = workbook.Sheets[firstSheetName];
                const rows = XLSX.utils.sheet_to_json(worksheet, { header: 1, defval: null});
                console.log("Sheet name: ", firstSheetName);
                return {
                    rows
                };
            }
            "#
    .to_string();
    let runner = NonRustCodeRunnerFactory::new("parse_xlsx", code, vec![file_path.clone()])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(Input { file_path }).await
}

#[cfg(test)]
mod tests {
    use crate::functions::parse_xlsx::parse_xlsx;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;
    use std::path;
    use std::path::Path;

    #[tokio::test]
    async fn test_parse_xlsx() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("../shinkai-fs/src/test_data/test.xlsx"))
            .unwrap()
            .to_path_buf();
        let rows = parse_xlsx(xlsx_file_path).await.unwrap();
        assert_eq!(rows.rows.len(), 4);
        assert_eq!(rows.rows[0].len(), 2);
    }

    #[tokio::test]
    async fn test_parse_xls() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let xlsx_file_path = path::absolute(Path::new("../shinkai-fs/src/test_data/test.xls"))
            .unwrap()
            .to_path_buf();
        let rows = parse_xlsx(xlsx_file_path).await.unwrap();
        assert_eq!(rows.rows.len(), 11);
        assert_eq!(rows.rows[0].len(), 8);
    }
}
