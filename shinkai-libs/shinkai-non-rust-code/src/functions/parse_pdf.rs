use std::path::PathBuf;

use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(serde::Serialize)]
pub struct Input {
    file_path: PathBuf,
}

#[derive(serde::Deserialize)]
pub struct Output {
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

pub async fn parse_pdf(file_path: PathBuf) -> Result<Output, RunError> {
    println!("parsing pdf file: {:?}", file_path);
    let code = r#"
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
    .to_string();
    let runner = NonRustCodeRunnerFactory::new("parse_pdf", code, vec![file_path.clone()])
        .with_runtime(NonRustRuntime::Python)
        .create_runner(json!({}));
    runner.run::<_, Output>(Input { file_path }).await
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
        let result = parse_pdf(file_path).await;
        assert!(result.is_ok());
        let parsed_pdf = result.unwrap();
        assert!(parsed_pdf.pages.len() == 14);
        assert!(parsed_pdf.pages[13]
            .text
            .contains("Essential for MAC\ncomputation, defined as"));
    }
}
