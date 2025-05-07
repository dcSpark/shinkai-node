use std::{collections::HashMap, env, path::PathBuf};

use serde_json::json;
use shinkai_tools_runner::tools::{
    code_files::CodeFiles, deno_runner::DenoRunner, deno_runner_options::DenoRunnerOptions, execution_context::ExecutionContext, python_runner::PythonRunner, python_runner_options::PythonRunnerOptions, runner_type::RunnerType
};

use crate::{
    shinkai_fs_error::ShinkaiFsError, simple_parser::{file_parser_helper::ShinkaiFileParser, text_group::TextGroup}
};

use super::LocalFileParser;

#[derive(serde::Deserialize)]
pub struct PDFParseResult {
    pub pages: Vec<Page>,
}

#[derive(serde::Deserialize)]
pub struct Page {
    pub metadata: Metadata,
    pub text: String,
}

#[derive(serde::Deserialize)]
pub struct Metadata {
    pub page: u32,
}

pub async fn parse_pdf_file(file_path: PathBuf) -> Result<PDFParseResult, ShinkaiFsError> {
    println!("parsing PDF file: {:?}", file_path);
    let code_files = CodeFiles {
        files: HashMap::from([(
            "main.py".to_string(),
            r#"
# /// script
# requires-python = ">=3.10,<3.13"
# dependencies = [
#   "pymupdf4llm",
# ]
# ///
import pymupdf4llm

class CONFIG:
    pass

class INPUTS:
    file_path: str

class OUTPUT:
    pages: object

async def run(c: CONFIG, p: INPUTS) -> OUTPUT:
    parsed_pages = pymupdf4llm.to_markdown(p.file_path, page_chunks=True)
    output = OUTPUT()
    output.pages = parsed_pages
    return output
"#
            .to_string(),
        )]),
        entrypoint: "main.py".to_string(),
    };
    let deno_runner = PythonRunner::new(
        code_files,
        json!({}),
        Some(PythonRunnerOptions {
            uv_binary_path: PathBuf::from(
                env::var("SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH")
                    .unwrap_or_else(|_| "./shinkai-tools-runner-resources/uv".to_string()),
            ),
            context: ExecutionContext {
                storage: PathBuf::from(env::var("NODE_STORAGE_PATH").unwrap_or_else(|_| "./".to_string()))
                    .join("internal_tools_storage"),

                context_id: "shinkai-node-pdf-parsing".to_string(),
                mount_files: vec![file_path.clone()],
                ..Default::default()
            },
            force_runner_type: Some(RunnerType::Host),
            ..Default::default()
        }),
    );

    let start = std::time::Instant::now();
    let result = deno_runner
        .run(
            None,
            json!({
                "file_path": file_path.to_str().unwrap(),
            }),
            None,
        )
        .await
        .inspect_err(|e| {
            println!("Error: {:?}", e);
        })
        .map_err(|_| ShinkaiFsError::FailedPDFParsing)?;
    println!("PDF parsing took: {:?}", start.elapsed());
    let data = serde_json::from_value::<PDFParseResult>(result.data).map_err(|_| ShinkaiFsError::FailedPDFParsing)?;
    Ok(data)
}

impl LocalFileParser {
    pub async fn process_pdf_file(
        file_path: PathBuf,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let parsed_pages = parse_pdf_file(file_path)
            .await
            .map_err(|_| ShinkaiFsError::FailedPDFParsing)?;

        let mut text_groups = Vec::new();

        for page in parsed_pages.pages {
            ShinkaiFileParser::push_text_group_by_depth(
                &mut text_groups,
                0,
                page.text,
                max_node_text_size,
                Some(page.metadata.page.try_into().unwrap_or_default()),
            );
        }

        Ok(text_groups)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    use super::*;
    use std::path;
    use std::path::Path;

    #[tokio::test]
    async fn test_parse_pdf_file() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let file_path = path::absolute(Path::new("../../files/Shinkai_Protocol_Whitepaper.pdf"))
            .unwrap()
            .to_path_buf();
        let result = parse_pdf_file(file_path).await;
        assert!(result.is_ok());
        let parsed_pdf = result.unwrap();
        assert!(parsed_pdf.pages.len() == 14);
        assert!(parsed_pdf.pages[13]
            .text
            .contains("Essential for MAC\ncomputation, defined as"));
    }
}
