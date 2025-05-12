use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Debug, Serialize)]
pub struct Input {
    file_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub text: String,
}

pub async fn parse_docx(file_path: PathBuf) -> Result<Output, RunError> {
    println!("parsing docx file: {:?}", file_path);
    let code = r#"
            import mammoth from 'npm:mammoth';
            import TurndownService from 'npm:turndown';
            import turndownPluginGfm from 'npm:turndown-plugin-gfm';
            const gfm = turndownPluginGfm.gfm;
            const tables = turndownPluginGfm.tables;

            const turndownService = new TurndownService();
            turndownService.use(gfm);
            turndownService.use([tables]);



            async function run(configurations, params) {
                console.log(params.file_path);
                const htmlResult = await mammoth.convertToHtml({ path: params.file_path });
                const markdownResult = turndownService.turndown(htmlResult.value);
                return {
                    text: markdownResult
                };
            }
            "#
    .to_string();
    let runner = NonRustCodeRunnerFactory::new("parse_docx", code, vec![file_path.clone()])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(Input { file_path }).await
}

#[cfg(test)]
mod tests {
    use crate::functions::parse_docx::parse_docx;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;
    use std::path;
    use std::path::Path;

    #[tokio::test]
    async fn test_parse_xlsx() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let file_path = path::absolute(Path::new("../../files/decision_log.docx"))
            .unwrap()
            .to_path_buf();
        let parsed_docx = parse_docx(file_path).await.unwrap();
        println!("parsed_docx: {:?}", parsed_docx);
        assert!(parsed_docx.text.contains("Approved backend languages are Go, Python"));
    }
}
