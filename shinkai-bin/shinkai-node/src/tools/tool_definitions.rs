pub mod definitions_built_in_tools;
pub mod definitions_custom;

use shinkai_http_api::api_v2::api_v2_handlers_tools::Language;
use shinkai_sqlite::SqliteManager;
use reqwest::StatusCode;
use shinkai_http_api::node_api_router::APIError;
use std::sync::Arc;

use super::llm_language_support::generate_typescript::generate_typescript_definition;
use super::tool_definitions::definitions_built_in_tools::get_built_in_tools;
use super::tool_definitions::definitions_custom::get_custom_tools;

#[derive(Debug)]
struct ToolExecutionResult {
    name: String,
    result: String,
    error: Option<String>,
}

fn generic_error_str(e: &str) -> APIError {
    APIError {
        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        error: "Internal Server Error".to_string(),
        message: format!("Error receiving result: {}", e),
    }
}

pub async fn generate_tool_definitions(
    language: Language,
    sqlite_manager: Arc<SqliteManager>,
    only_headers: bool,
) -> Result<String, APIError> {
    let mut all_tools = match sqlite_manager.get_all_tool_headers() {
        Ok(data) => data,
        Err(_) => Vec::new(),
    };

    all_tools.extend(get_custom_tools());

    let mut output = String::new();
    match language {
        Language::Typescript => {
            if !only_headers {
                output.push_str("import axios from 'npm:axios';\n\n");
            }
        }
        Language::Python => {
            output.push_str("import os\nimport requests\nfrom typing import TypedDict, Optional\n\n");
            // output.push_str(&generate_python_definition(name, &runner_def));
        }
    }
    for tool in all_tools {
        match language {
            Language::Typescript => {
                output.push_str(&generate_typescript_definition(tool, only_headers));
            }
            Language::Python => {
                output.push_str("import os\nimport requests\nfrom typing import TypedDict, Optional\n\n");
            }
        }
    }

    Ok(output)
}
