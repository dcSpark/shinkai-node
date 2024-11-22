use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_sqlite::{SqliteManager, SqliteManagerError};
use shinkai_tools_primitives::tools::tool_playground::ToolPlayground;
use std::collections::HashSet;
use std::sync::Arc;

use crate::tools::llm_language_support::generate_typescript::generate_typescript_definition;

use super::definitions_custom::get_custom_tools;

/// Generates tool definitions for a specified programming language.
///
/// # Arguments
///
/// * `language` - The target programming language for which the tool definitions are generated.
///   It can be either `Language::Typescript` or `Language::Python`.
///
/// * `sqlite_manager` - An `Arc` wrapped `SqliteManager` instance used to fetch tool headers
///   from the SQLite database. This manager provides access to the database operations.
///
/// * `only_headers` - A boolean flag indicating whether to generate only the headers of the tool
///   definitions. If `true`, only the headers are generated; otherwise, full definitions are
///   included.
///
/// # Returns
///
/// Returns a `Result` containing a `String` with the generated tool definitions or an `APIError`
/// if an error occurs during the process.
pub async fn generate_tool_definitions(
    language: CodeLanguage,
    sqlite_manager: Arc<SqliteManager>,
    only_headers: bool,
) -> Result<String, APIError> {
    let mut all_tools = match sqlite_manager.get_all_tool_headers() {
        Ok(data) => data,
        Err(_) => Vec::new(),
    };

    all_tools.extend(get_custom_tools());

    let mut output = String::new();
    let mut generated_names = HashSet::new();

    match language {
        CodeLanguage::Typescript => {
            if !only_headers {
                output.push_str("import axios from 'npm:axios';\n\n");
            }
        }
        CodeLanguage::Python => {
            output.push_str("import os\nimport requests\nfrom typing import TypedDict, Optional\n\n");
        }
    }

    for tool in all_tools {
        let tool_playground: Option<ToolPlayground> = match sqlite_manager.get_tool_playground(&tool.tool_router_key) {
            Ok(tool_playground) => Some(tool_playground),
            Err(SqliteManagerError::ToolPlaygroundNotFound(_)) => None,
            Err(e) => return Err(APIError::from(e.to_string())),
        };

        match language {
            CodeLanguage::Typescript => {
                let function_name =
                    crate::tools::llm_language_support::language_helpers::to_camel_case(&tool.tool_router_key);
                if generated_names.contains(&function_name) {
                    eprintln!(
                        "Warning: Duplicate function name '{}' found for tool '{}'. Skipping generation.",
                        function_name, tool.name
                    );
                    continue;
                }
                generated_names.insert(function_name);
                output.push_str(&generate_typescript_definition(tool, only_headers, tool_playground));
            }
            CodeLanguage::Python => {
                output.push_str("import os\nimport requests\nfrom typing import TypedDict, Optional\n\n");
            }
        }
    }

    Ok(output)
}
