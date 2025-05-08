use std::path::PathBuf;

use shinkai_non_rust_code::functions::parse_docx::parse_docx;

use crate::{
    shinkai_fs_error::ShinkaiFsError, simple_parser::{file_parser_helper::ShinkaiFileParser, text_group::TextGroup}
};

use super::LocalFileParser;

impl LocalFileParser {
    pub async fn process_docx_file(
        file_path: PathBuf,
        max_node_text_size: u64,
    ) -> Result<Vec<TextGroup>, ShinkaiFsError> {
        let parsed_docx = parse_docx(file_path)
            .await
            .map_err(|_| ShinkaiFsError::FailedDOCXParsing)?;

        let mut text_groups = Vec::new();
        ShinkaiFileParser::push_text_group_by_depth(&mut text_groups, 0, parsed_docx.text, max_node_text_size, None);
        Ok(text_groups)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{self, Path};

    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    use super::*;

    #[tokio::test]
    async fn test_process_docx_json() -> Result<(), Box<dyn std::error::Error>> {
        let _dir = testing_create_tempdir_and_set_env_var();

        let file_path = path::absolute(Path::new("../../files/decision_log.docx"))
            .unwrap()
            .to_path_buf();
        let text_groups = LocalFileParser::process_docx_file(file_path, 1000).await?;

        // Debug print all groups
        println!("\nDebug output of all text groups:");
        for (i, group) in text_groups.iter().enumerate() {
            println!("Group {}: text='{}', metadata={:?}", i, group.text, group.metadata);
        }

        // Basic validation of the results
        assert!(!text_groups.is_empty(), "Should extract at least one text group");

        // Verify the content of text groups
        assert_eq!(text_groups.len(), 2, "Should have extracted two text groups");

        assert!(
            text_groups
                .iter()
                .any(|group| group.text.contains("Approved backend languages are Go, Python")),
            "Should contain text about approved backend languages"
        );

        Ok(())
    }
}
