use super::language_helpers::to_snake_case;
use serde_json::Value;
use shinkai_tools_primitives::tools::{
    shinkai_tool::ShinkaiToolHeader,
    tool_playground::{SqlQuery, SqlTable},
    tool_types::ToolResult,
};

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

fn generate_docstring(tool: &ShinkaiToolHeader, tool_result: ToolResult, indent: &str) -> String {
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
    if let Some(properties) = tool_result.properties.as_object() {
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
    doc.push_str(&format!("{}    }}\n{}\"\"\"", indent, indent));
    doc
}

pub fn generate_python_definition(
    tool: ShinkaiToolHeader,
    tool_result: ToolResult,
    sql_tables: Vec<SqlTable>,
    sql_queries: Vec<SqlQuery>,
    generate_pyi: bool,
) -> String {
    let mut python_output = String::new();
    let function_name = create_function_name_set(&tool);

    if generate_pyi {
        // Generate .pyi stub file
        python_output.push_str(&format!("async def {}(", function_name));
        python_output.push_str(&generate_parameters(&tool));
        python_output.push_str(") -> Dict[str, Any]:\n");

        // Add docstring to .pyi
        python_output.push_str(&generate_docstring(&tool, tool_result, "    "));
        python_output.push_str("\n    pass\n");

        return python_output;
    } else {
        // Original implementation for .py file
        python_output.push_str(&format!("async def {}(", function_name));
        python_output.push_str(&generate_parameters(&tool));
        python_output.push_str(") -> Dict[str, Any]:\n");
        python_output.push_str(&generate_docstring(&tool, tool_result, "    "));

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
            },
            timeout=360
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

    // Add SQL query function if tables exist
    if !sql_tables.is_empty() {
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
        for table in sql_tables {
            python_output.push_str(&format!("    {}:\n        {}\n", table.name, table.definition));
        }

        if !sql_queries.is_empty() {
            python_output.push_str("\n    Example / Reference SQL Queries:\n");
            for query in sql_queries {
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
"#,
        );

        if generate_pyi {
            python_output.push_str("    pass\n");
        } else {
            python_output.push_str("    return shinkai_sqlite_query_executor(query, params)\n");
        }
    }

    python_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_tools_primitives::tools::parameters::{Parameters, Property};

    #[test]
    fn test_generate_python_definition() {
        // Create a test tool header
        let tool = ShinkaiToolHeader {
            name: "Test Tool".to_string(),
            description: "A test tool for unit testing".to_string(),
            tool_router_key: "local:::test:::test_tool".to_string(),
            tool_type: "Python".to_string(),
            formatted_tool_summary_for_ui: "Test Tool Summary".to_string(),
            author: "test_author".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            input_args: {
                let mut params = Parameters::new();
                params.properties.insert(
                    "string_param".to_string(),
                    Property::new("string".to_string(), "A string parameter".to_string()),
                );
                params.properties.insert(
                    "number_param".to_string(),
                    Property::new("number".to_string(), "A number parameter".to_string()),
                );
                params.required.push("string_param".to_string());
                params
            },
            output_arg: shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg { json: "{}".to_string() },
            config: None,
            usage_type: None,
            tool_offering: None,
        };

        // Create a test tool result
        let tool_result = ToolResult::new(
            "object".to_string(),
            json!({
                "result": { "type": "string", "description": "The result" },
                "count": { "type": "number", "description": "Count value" }
            }),
            vec!["result".to_string()],
        );

        // Generate Python definition
        let py_def = generate_python_definition(
            tool,
            tool_result,
            vec![], // No SQL tables
            vec![], // No SQL queries
            false,  // Not generating .pyi
        );

        // Verify the output contains expected elements
        assert!(py_def.contains("\"\"\"A test tool for unit testing"));
        assert!(py_def.contains("async def test_tool"));
        assert!(py_def.contains("input: Dict[str, Any]"));
        assert!(py_def.contains("string_param: str (required)"));
        assert!(py_def.contains("number_param: float (optional)"));
        assert!(py_def.contains("result: str"));
        assert!(py_def.contains("count: float"));
        assert!(py_def.contains("Dict[str, Any]"));
        assert!(py_def.contains("'tool_router_key': 'local:::test:::test_tool'"));
    }

    #[test]
    fn test_generate_python_definition_with_sql() {
        // Create a test tool header similar to previous test
        let tool = ShinkaiToolHeader {
            name: "SQL Test Tool".to_string(),
            description: "A test tool with SQL capabilities".to_string(),
            tool_router_key: "local:::test:::sql_test_tool".to_string(),
            tool_type: "Python".to_string(),
            formatted_tool_summary_for_ui: "SQL Test Tool Summary".to_string(),
            author: "test_author".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            input_args: Parameters::new(),
            output_arg: shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg { json: "{}".to_string() },
            config: None,
            usage_type: None,
            tool_offering: None,
        };

        // Create SQL tables and queries
        let sql_tables = vec![SqlTable {
            name: "users".to_string(),
            definition: "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
        }];

        let sql_queries = vec![SqlQuery {
            name: "get_users".to_string(),
            query: "SELECT * FROM users".to_string(),
        }];

        let tool_result = ToolResult::new("object".to_string(), json!({}), vec![]);

        // Generate Python definition
        let py_def = generate_python_definition(tool, tool_result, sql_tables, sql_queries, false);

        // Verify SQL-related content
        assert!(py_def.contains("Query the SQL database"));
        assert!(py_def.contains("CREATE TABLE users"));
        assert!(py_def.contains("SELECT * FROM users"));
        assert!(py_def.contains("async def query_sql_test_tool"));
    }

    #[test]
    fn test_generate_python_pyi() {
        // Test generating .pyi stub file
        let tool = ShinkaiToolHeader {
            name: "PYI Test Tool".to_string(),
            description: "A test tool for .pyi generation".to_string(),
            tool_router_key: "local:::test:::pyi_test_tool".to_string(),
            tool_type: "Python".to_string(),
            formatted_tool_summary_for_ui: "PYI Test Tool Summary".to_string(),
            author: "test_author".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            input_args: Parameters::new(),
            output_arg: shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg { json: "{}".to_string() },
            config: None,
            usage_type: None,
            tool_offering: None,
        };

        let tool_result = ToolResult::new(
            "object".to_string(),
            json!({
                "result": { "type": "string" }
            }),
            vec!["result".to_string()],
        );

        let py_def = generate_python_definition(
            tool,
            tool_result,
            vec![],
            vec![],
            true, // Generate .pyi
        );

        // Verify .pyi specific content
        assert!(py_def.contains("async def pyi_test_tool"));
        assert!(py_def.contains("Dict[str, Any]"));
        assert!(py_def.contains("pass")); // Stub implementation
        assert!(!py_def.contains("requests.post")); // Implementation should not be included
        assert!(!py_def.contains("os.environ")); // Implementation should not be included
    }

    #[test]
    fn test_json_type_to_python() {
        // Test various JSON type conversions
        assert_eq!(json_type_to_python(&json!("string"), None), "str");
        assert_eq!(json_type_to_python(&json!("number"), None), "float");
        assert_eq!(json_type_to_python(&json!("integer"), None), "int");
        assert_eq!(json_type_to_python(&json!("boolean"), None), "bool");
        assert_eq!(json_type_to_python(&json!("object"), None), "Dict[str, Any]");
        assert_eq!(json_type_to_python(&json!("any"), None), "Any");

        // Test array types
        assert_eq!(
            json_type_to_python(&json!("array"), Some(&json!({"type": "string"}))),
            "List[str]"
        );
        assert_eq!(
            json_type_to_python(&json!("array"), Some(&json!({"type": "number"}))),
            "List[float]"
        );
        assert_eq!(json_type_to_python(&json!("array"), None), "List[Any]");
    }
}
