pub mod definitions_built_in_tools;
pub mod definitions_custom;

use shinkai_http_api::api_v2::api_v2_handlers_tools::Language;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;

use super::llm_language_support::generate_python::generate_python_definition;
use super::llm_language_support::generate_typescript::generate_typescript_definition;
use super::tool_definitions::definitions_built_in_tools::get_built_in_tools;
use super::tool_definitions::definitions_custom::get_custom_tools;

#[derive(Debug)]
struct ToolExecutionResult {
    name: String,
    result: String,
    error: Option<String>,
}

pub async fn generate_tool_definitions(language: Language, sqlite_manager: Arc<SqliteManager>) -> String {
    let mut tools = get_built_in_tools();
    tools.extend(get_custom_tools());

    let tools_data = match sqlite_manager.get_all_tool_headers() {
        Ok(data) => data,
        Err(_) => Vec::new(),
    };

    let mut output = String::new();

    match language {
        Language::Typescript => {
            output.push_str("import axios from 'axios';\n\n");
        }
        Language::Python => {
            output.push_str("import os\nimport requests\nfrom typing import TypedDict, Optional\n\n");
        }
        _ => return "Unsupported language".to_string(),
    }

    for (name, runner_def) in tools {
        let tool_result = tools_data.iter().find(|header| header.toolkit_name == name);
        match language {
            Language::Typescript => {
                output.push_str(&generate_typescript_definition(name, &runner_def, tool_result));
            }
            Language::Python => {
                output.push_str(&generate_python_definition(name, &runner_def));
            }
            _ => unreachable!(),
        }
    }

    output
}
