use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiToolHeader;
use shinkai_tools_primitives::tools::tool_playground::ToolPlayground;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::tools::llm_language_support::file_support_py::generate_file_support_py;
use crate::tools::llm_language_support::file_support_ts::generate_file_support_ts;
use crate::tools::llm_language_support::generate_python::{generate_python_definition, python_common_code};
use crate::tools::llm_language_support::generate_typescript::{generate_typescript_definition, typescript_common_code};
use crate::tools::tool_implementation;

// TODO keep in sync with execution_custom.rs
pub fn get_rust_tools() -> Vec<ShinkaiToolHeader> {
    let mut custom_tools = Vec::new();
    custom_tools.push(tool_implementation::native_tools::llm_prompt_processor::LmPromptProcessorTool::new().tool);
    custom_tools.push(tool_implementation::native_tools::sql_processor::SQLProcessorTool::new().tool);
    custom_tools.push(tool_implementation::native_tools::tool_knowledge::KnowledgeTool::new().tool);
    custom_tools
}

pub async fn get_all_deno_tools(sqlite_manager: Arc<SqliteManager>) -> Vec<ShinkaiToolHeader> {
    let mut all_tools = match sqlite_manager.get_all_tool_headers() {
        Ok(data) => data,
        Err(_) => Vec::new(),
    };
    all_tools.extend(get_rust_tools());
    return all_tools;
}

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
    tools: Vec<ToolRouterKey>,
    language: CodeLanguage,
    sqlite_manager: Arc<SqliteManager>,
    only_headers: bool,
) -> Result<HashMap<String, String>, APIError> {
    let mut support_files = HashMap::new();
    match language {
        CodeLanguage::Typescript => {
            support_files.insert(
                "shinkai-local-support".to_string(),
                generate_file_support_ts(only_headers),
            );
        }
        CodeLanguage::Python => {
            support_files.insert(
                "shinkai_local_support".to_string(),
                generate_file_support_py(only_headers),
            );
        }
    };
    // Filter tools
    let all_tools: Vec<ShinkaiToolHeader> = get_all_deno_tools(sqlite_manager.clone())
        .await
        .into_iter()
        .filter(|tool| {
            tools.iter().any(|t| {
                let version = t.version.clone();
                match version {
                    Some(v) => t.to_string_without_version() == tool.tool_router_key && v == tool.version,
                    None => t.to_string_without_version() == tool.tool_router_key,
                }
            })
        })
        .collect();

    if all_tools.is_empty() {
        return Ok(support_files);
    }
    let mut output = String::new();
    let mut generated_names = HashSet::new();

    if !only_headers {
        match language {
            CodeLanguage::Typescript => {
                output.push_str(&typescript_common_code());
            }
            CodeLanguage::Python => {
                output.push_str(&python_common_code());
            }
        };
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
                    crate::tools::llm_language_support::generate_typescript::create_function_name_set(&tool);
                if generated_names.contains(&function_name) {
                    eprintln!(
                        "Warning: Duplicate function name '{}' found for tool '{}'. Skipping generation.",
                        function_name,
                        tool.name.clone()
                    );
                    continue;
                }
                generated_names.insert(function_name);
                output.push_str(&generate_typescript_definition(tool, only_headers, tool_playground));
            }
            CodeLanguage::Python => {
                let function_name =
                    crate::tools::llm_language_support::generate_python::create_function_name_set(&tool);
                if generated_names.contains(&function_name) {
                    eprintln!(
                        "Warning: Duplicate function name '{}' found for tool '{}'. Skipping generation.",
                        function_name,
                        tool.name.clone()
                    );
                    continue;
                }
                generated_names.insert(function_name);
                output.push_str(&generate_python_definition(tool, only_headers, tool_playground));
            }
        }
    }

    match language {
        CodeLanguage::Typescript => {
            support_files.insert("shinkai-local-tools".to_string(), output);
        }
        CodeLanguage::Python => {
            support_files.insert("shinkai_local_tools".to_string(), output);
        }
    };

    Ok(support_files)
}
