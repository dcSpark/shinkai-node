use super::language_helpers::to_snake_case;
use serde_json::Value;
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolHeader, tool_playground::ToolPlayground};

// Example output:
/*

from typing import Optional, Any, Dict, List, Union
import os
import requests
async def shinkai_download_pages(input: Dict[str, Any]) -> Dict[str, Any]:
    """Downloads one or more URLs and converts their HTML content to Markdown

    Args:
        input: Dict[str, Any]:
            urls: List[Any] (required) -

    Returns:
        Dict[str, Any]: {
            markdowns: List[str]
        }
    """
    _url = os.environ.get('SHINKAI_NODE_LOCATION', '') + '/v2/tool_execution'
    data = {
        'tool_router_key': 'local:::shinkai_tool_download_pages:::shinkai__download_pages',
        'tool_type': 'deno',
        'llm_provider': os.environ.get('X_SHINKAI_LLM_PROVIDER', ''),
        'parameters': input
    }
    try:
        response = requests.post(
            _url,
            json=data,
            headers={
                'Authorization': f"Bearer {os.environ.get('BEARER', '')}",
                'x-shinkai-tool-id': os.environ.get('X_SHINKAI_TOOL_ID', ''),
                'x-shinkai-app-id': os.environ.get('X_SHINKAI_APP_ID', ''),
                'x-shinkai-llm-provider': os.environ.get('X_SHINKAI_LLM_PROVIDER', '')
            }
        )
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        error_message = '::NETWORK_ERROR:: '
        if hasattr(e, 'response') and e.response is not None:
            error_message += f"Status: {e.response.status_code}, "
            try:
                error_message += f"Response: {e.response.json()}"
            except:
                error_message += f"Response: {e.response.text}"
        else:
            error_message += str(e)
        raise Exception(error_message)

*/

fn json_type_to_python(type_value: &Value, items_value: Option<&Value>) -> String {
    match type_value.as_str() {
        Some("array") => {
            if let Some(items) = items_value {
                if let Some(item_type) = items.get("type") {
                    let base_type = match item_type.as_str() {
                        Some("string") => "str",
                        Some("number") => "float",
                        Some("integer") => "int",
                        Some("boolean") => "bool",
                        Some("object") => "Dict[str, Any]",
                        Some("array") => "List[Any]",
                        Some("any") => "Any",
                        _ => "Any",
                    };
                    format!("List[{}]", base_type)
                } else {
                    "List[Any]".to_string()
                }
            } else {
                "List[Any]".to_string()
            }
        }
        Some("string") => "str".to_string(),
        Some("number") => "float".to_string(),
        Some("integer") => "int".to_string(),
        Some("boolean") => "bool".to_string(),
        Some("object") => "Dict[str, Any]".to_string(),
        Some("any") => "Any".to_string(),
        Some("Any") => "Any".to_string(),
        Some("any[]") => "List[Any]".to_string(),
        Some("function") => "str".to_string(),

        Some(t) => {
            // Check if this is actually an object with a "type" field
            if let Some(obj_type) = type_value.get("type") {
                json_type_to_python(obj_type, type_value.get("items"))
            } else {
                t.to_string()
            }
        }
        None => "Any".to_string(),
    }
}

pub fn create_function_name_set(tool: &ShinkaiToolHeader) -> String {
    to_snake_case(&tool.name)
}

pub fn python_common_code() -> String {
    "
from typing import Optional, Any, Dict, List, Union
import os
import requests
"
    .to_string()
}

fn generate_parameters(tool: &ShinkaiToolHeader) -> String {
    let mut param_types: Vec<String> = Vec::new();

    for (key, property) in &tool.input_args.properties {
        let is_required = tool.input_args.required.contains(key);
        let type_str = json_type_to_python(&Value::String(property.property_type.clone()), None);
        if is_required {
            param_types.push(format!("{}: {}", key, type_str));
        } else {
            param_types.push(format!("{}: Optional[{}] = None", key, type_str));
        }
    }

    // Format as TypeScript-style input object
    format!("input: Dict[str, Any]")
}

fn generate_docstring(tool: &ShinkaiToolHeader, indent: &str) -> String {
    let mut doc = String::new();

    // Main description
    doc.push_str(&format!("{}\"\"\"{}\n\n", indent, tool.description));

    // Input schema documentation
    doc.push_str(&format!("{}Args:\n", indent));
    doc.push_str(&format!("{}    input: Dict[str, Any]:\n", indent));

    // Document each parameter in the input dictionary
    for (key, property) in &tool.input_args.properties {
        let is_required = tool.input_args.required.contains(key);

        let type_str = json_type_to_python(&Value::String(property.property_type.clone()), None);
        let required_str = if is_required { "required" } else { "optional" };
        doc.push_str(&format!(
            "{}        {}: {} ({}) - {}\n",
            indent, key, type_str, required_str, property.description
        ));
    }

    // Returns documentation
    doc.push_str(&format!("\n{}Returns:\n{}    Dict[str, Any]: {{\n", indent, indent));
    if let Ok(output_schema) = serde_json::from_str::<Value>(&tool.output_arg.json) {
        if let Some(properties) = output_schema.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_value) in properties {
                let type_str = json_type_to_python(
                    prop_value.get("type").unwrap_or(&Value::String("Any".to_string())),
                    prop_value.get("items"),
                );
                let desc = prop_value.get("description").and_then(|d| d.as_str()).unwrap_or("");
                doc.push_str(&format!(
                    "{}        {}: {} {}\n",
                    indent,
                    prop_name,
                    type_str,
                    if !desc.is_empty() {
                        format!("- {}", desc)
                    } else {
                        String::new()
                    }
                ));
            }
        }
    }
    doc.push_str(&format!("{}    }}\n{}\"\"\"", indent, indent));
    doc
}

pub fn generate_python_definition(
    tool: ShinkaiToolHeader,
    generate_pyi: bool,
    tool_playground: Option<ToolPlayground>,
) -> String {
    let mut python_output = String::new();
    let function_name = create_function_name_set(&tool);

    if generate_pyi {
        // Generate .pyi stub file
        python_output.push_str(&format!("async def {}(", function_name));
        python_output.push_str(&generate_parameters(&tool));
        python_output.push_str(") -> Dict[str, Any]:\n");

        // Add docstring to .pyi
        python_output.push_str(&generate_docstring(&tool, "    "));
        python_output.push_str("\n    ...\n");

        // If SQL tables exist, generate query function stub with docs
        if let Some(playground) = tool_playground {
            if !playground.metadata.sql_tables.is_empty() {
                python_output.push_str("\n\n");
                python_output.push_str(&format!(
                    "async def query_{}(query: str, params: Optional[List[Any]] = None) -> List[Dict[str, Any]]:\n",
                    function_name
                ));

                // Add query function documentation
                python_output.push_str(&format!(
                    "    \"\"\"Query the SQL database for results from {}\n\n",
                    function_name
                ));
                python_output.push_str("    Available SQL Tables:\n");
                for table in &playground.metadata.sql_tables {
                    python_output.push_str(&format!("    {}:\n        {}\n", table.name, table.definition));
                }

                if !playground.metadata.sql_queries.is_empty() {
                    python_output.push_str("\n    Example / Reference SQL Queries:\n");
                    for query in &playground.metadata.sql_queries {
                        python_output.push_str(&format!("    {}:\n        {}\n", query.name, query.query));
                    }
                }

                python_output.push_str(
                    r#"
    Args:
        query (str): SQL query to execute
        params (Optional[List[Any]], optional): Query parameters. Defaults to None.

    Returns:
        List[Dict[str, Any]]: Query results
    """
    ...
"#,
                );
            }
        }

        return python_output;
    } else {
        // Original implementation for .py file
        python_output.push_str(&format!("async def {}(", function_name));
        python_output.push_str(&generate_parameters(&tool));
        python_output.push_str(") -> Dict[str, Any]:\n");
        python_output.push_str(&generate_docstring(&tool, "    "));

        // Add the implementation
        python_output.push_str(
            r#"
    _url = os.environ.get('SHINKAI_NODE_LOCATION', '') + '/v2/tool_execution'
    data = {
        'tool_router_key': '"#,
        );
        python_output.push_str(&tool.tool_router_key);
        python_output.push_str(
            r#"',
        'tool_type': '"#,
        );
        python_output.push_str(&tool.tool_type.to_lowercase());
        python_output.push_str(
            r#"',
        'llm_provider': os.environ.get('X_SHINKAI_LLM_PROVIDER', ''),
        'parameters': input
    }
"#,
        );

        // Rest of implementation...
        python_output.push_str(
            r#"    try:
        response = requests.post(
            _url,
            json=data,
            headers={
                'Authorization': f"Bearer {os.environ.get('BEARER', '')}",
                'x-shinkai-tool-id': os.environ.get('X_SHINKAI_TOOL_ID', ''),
                'x-shinkai-app-id': os.environ.get('X_SHINKAI_APP_ID', ''),
                'x-shinkai-llm-provider': os.environ.get('X_SHINKAI_LLM_PROVIDER', '')
            }
        )
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        error_message = '::NETWORK_ERROR:: '
        if hasattr(e, 'response') and e.response is not None:
            error_message += f"Status: {e.response.status_code}, "
            try:
                error_message += f"Response: {e.response.json()}"
            except:
                error_message += f"Response: {e.response.text}"
        else:
            error_message += str(e)
        raise Exception(error_message)
"#,
        );
    }

    python_output
}
