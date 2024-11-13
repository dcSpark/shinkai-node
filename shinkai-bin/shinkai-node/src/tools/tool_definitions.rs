pub mod definitions_built_in_tools;
pub mod definitions_custom;

use reqwest::StatusCode;
use shinkai_http_api::api_v2::api_v2_handlers_tools::{Language, ToolType};
use shinkai_http_api::node_api_router::APIError;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::llm_language_support::generate_python::generate_python_definition;
use super::llm_language_support::generate_typescript::generate_typescript_definition;
use super::tool_definitions::definitions_built_in_tools::get_built_in_tools;
use super::tool_definitions::definitions_custom::get_custom_tools;
use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;

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
    lance_db: Arc<RwLock<LanceShinkaiDb>>,
    only_headers: bool,
) -> Result<String, APIError> {
    // let mut tools = get_built_in_tools();
    // tools.extend(get_custom_tools());

    let all_tools = lance_db
        .read()
        .await
        .get_all_tools(true) // true to include network tools
        .await
        .map_err(|e| generic_error_str(&format!("Failed to fetch tools: {}", e)))?;

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
