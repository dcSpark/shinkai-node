pub mod definitions_custom;
pub mod definitions_built_in_tools;

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use shinkai_tools_runner::tools::tool_definition::ToolDefinition;
use tokio::sync::RwLock;

use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;
use super::llm_language_support::generate_typescript::generate_typescript_definition;
use super::llm_language_support::generate_python::generate_python_definition;
use super::tool_definitions::definitions_custom::get_custom_tools;
use super::tool_definitions::definitions_built_in_tools::get_built_in_tools;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiToolHeader;

#[derive(Debug)]
struct ToolExecutionResult {
    name: String,
    result: String,
    error: Option<String>,
}

pub async fn generate_tool_definitions(language: &str,
             lance_db: Arc<RwLock<LanceShinkaiDb>>,
) -> String {
    let mut tools = get_built_in_tools();
    tools.extend(get_custom_tools());

    let tools_data = match lance_db.read().await.get_all_tools(true).await {
        Ok(data) => data,
        Err(_) => Vec::new(),
    };
    
    let mut output = String::new();
    
    match language.to_lowercase().as_str() {
        "typescript" | "ts" => {
            output.push_str("import axios from 'axios';\n\n");
        }
        "python" | "py" => {
            output.push_str("import os\nimport requests\nfrom typing import TypedDict, Optional\n\n");
        }
        _ => return "Unsupported language".to_string(),
    }

    for (name, runner_def) in tools {
        
        let tool_result = tools_data.iter().find(|header| header.toolkit_name == name);
        
        // match tool_result {
        //     Some(header) => {
        //         results.push(ToolExecutionResult {
        //             name: name.clone(),
        //             result: header.result.clone(),
        //             error: header.error.clone(),
        //         });
        //     }
        //     None => {
        //         results.push(ToolExecutionResult {
        //             name: name.clone(),
        //             result: String::new(),
        //             error: Some("No result found for tool".to_string()),
        //         });
        //     }
        // }

        match language.to_lowercase().as_str() {
            "typescript" | "ts" => {
                output.push_str(&generate_typescript_definition(name, &runner_def, tool_result));
            }
            "python" | "py" => {
                output.push_str(&generate_python_definition(name, &runner_def));
            }
            _ => unreachable!(),
        }
    }

    output
}
