// Example output:
/*
/**
 * Downloads one or more URLs and converts their HTML content to Markdown
 * @param input - {
 *   urls: any[]
 *
 * @returns {
 *   markdowns: string[];
 * }
 */
export async function shinkaiDownloadPages(input: {urls: any[]}): Promise<{
    markdowns: string[];
}> {

    const _url = `${Deno.env.get('SHINKAI_NODE_LOCATION')}/v2/tool_execution`;
    const data = {
        tool_router_key: 'local:::shinkai_tool_download_pages:::shinkai__download_pages',
        tool_type: 'deno',
        llm_provider: `${Deno.env.get('X_SHINKAI_LLM_PROVIDER')}`,
        parameters: input,
    };
    try {
        const response = await axios.post(_url, data, {
            headers: {
                'Authorization': `Bearer ${Deno.env.get('BEARER')}`,
                'x-shinkai-tool-id': `${Deno.env.get('X_SHINKAI_TOOL_ID')}`,
                'x-shinkai-app-id': `${Deno.env.get('X_SHINKAI_APP_ID')}`,
                'x-shinkai-llm-provider': `${Deno.env.get('X_SHINKAI_LLM_PROVIDER')}`,
                'x-shinkai-agent-id': `${Deno.env.get('X_SHINKAI_AGENT_ID')}`
            }
        });
        return response.data;
    } catch (error) {
        return manageAxiosError(error);
    }
}
*/
use serde_json::Value;
use shinkai_tools_primitives::tools::{
    shinkai_tool::ShinkaiToolHeader,
    tool_playground::{SqlQuery, SqlTable},
    tool_types::ToolResult,
};

pub fn create_function_name_set(tool: &ShinkaiToolHeader) -> String {
    crate::tools::llm_language_support::language_helpers::to_camel_case(&tool.name)
}

fn json_type_to_typescript(type_value: &Value, items_value: Option<&Value>) -> String {
    match type_value.as_str() {
        Some("array") => {
            if let Some(items) = items_value {
                if let Some(item_type) = items.get("type") {
                    let base_type = match item_type.as_str() {
                        Some("string") => "string",
                        Some("number") => "number",
                        Some("integer") => "number",
                        Some("boolean") => "boolean",
                        Some("object") => "object",
                        Some("array") => "any[]",
                        _ => "any",
                    };
                    format!("{}[]", base_type)
                } else if let Some(ref_type) = items.get("$ref") {
                    // Handle $ref types
                    format!("{}[]", ref_type.as_str().unwrap_or("any"))
                } else {
                    "any[]".to_string()
                }
            } else {
                "any[]".to_string()
            }
        }
        Some("string") => "string".to_string(),
        Some("number") => "number".to_string(),
        Some("integer") => "number".to_string(),
        Some("boolean") => "boolean".to_string(),
        Some("object") => "object".to_string(),
        Some(t) => t.to_string(),
        None => "any".to_string(),
    }
}

// Add this helper function
fn arg_type_to_typescript(arg_type: &str) -> String {
    // Handle array types
    if arg_type == "array" || arg_type.ends_with("[]") {
        "any[]".to_string()
    } else {
        match arg_type {
            "string" => "string".to_string(),
            "number" => "number".to_string(),
            "integer" => "number".to_string(),
            "boolean" => "boolean".to_string(),
            "object" => "object".to_string(),
            _ => arg_type.to_string(),
        }
    }
}

pub fn generate_typescript_definition(
    tool: ShinkaiToolHeader,
    tool_result: ToolResult,
    sql_tables: Vec<SqlTable>,
    sql_queries: Vec<SqlQuery>,
    generate_dts: bool,
) -> String {
    let mut typescript_output = String::new();
    let function_name = create_function_name_set(&tool);

    // Combine JSDoc comment generation
    typescript_output.push_str(&format!("/**\n * {}\n", tool.description));

    // Generate input schema documentation
    typescript_output.push_str(" * @param input - {\n");
    if let Ok(input_schema) = serde_json::from_str::<Value>(&serde_json::to_string(&tool.input_args).unwrap()) {
        if let Some(properties) = input_schema.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_value) in properties {
                let type_str = json_type_to_typescript(
                    prop_value.get("type").unwrap_or(&Value::String("any".to_string())),
                    prop_value.get("items"),
                );
                let desc = prop_value.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let required = tool.input_args.required.contains(prop_name);
                typescript_output.push_str(&format!(
                    " *   {}{}: {} {}\n",
                    prop_name,
                    if required { "" } else { "?" },
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

    // Generate return type documentation
    typescript_output.push_str(" *\n");
    typescript_output.push_str(" * @returns {\n");
    if let Some(properties) = tool_result.properties.as_object() {
        for (prop_name, prop_value) in properties {
            let type_str = json_type_to_typescript(
                prop_value.get("type").unwrap_or(&Value::String("any".to_string())),
                prop_value.get("items"),
            );
            typescript_output.push_str(&format!(" *   {}: {};\n", prop_name, type_str));
        }
    }
    typescript_output.push_str(" * }\n */\n");

    // Function signature with single input parameter
    typescript_output.push_str(&format!("export async function {}(input: {{", function_name));

    // Generate input type inline
    let params: Vec<String> = tool
        .input_args
        .to_deprecated_arguments()
        .iter()
        .map(|arg| {
            let type_str = arg_type_to_typescript(&arg.arg_type);
            if arg.is_required {
                format!("{}: {}", arg.name, type_str)
            } else {
                format!("{}?: {}", arg.name, type_str)
            }
        })
        .collect();
    typescript_output.push_str(&params.join(", "));
    typescript_output.push_str("}): Promise<{\n");

    // Generate return type inline
    if let Some(properties) = tool_result.properties.as_object() {
        for (prop_name, prop_value) in properties {
            let type_str = json_type_to_typescript(
                prop_value.get("type").unwrap_or(&Value::String("any".to_string())),
                prop_value.get("items"),
            );
            typescript_output.push_str(&format!("    {}: {};\n", prop_name, type_str));
        }
    }
    typescript_output.push_str("}>"); // Close the return type

    if generate_dts {
        typescript_output.push_str(";\n"); // End the declaration with semicolon
    } else {
        // Only include implementation if not generating .d.ts
        typescript_output.push_str(" {\n");
        typescript_output.push_str(&format!(
            "
    const _url = `${{Deno.env.get('SHINKAI_NODE_LOCATION')}}/v2/tool_execution`;
    const data = {{
        tool_router_key: '{}',
        tool_type: '{}',
        llm_provider: `${{Deno.env.get('X_SHINKAI_LLM_PROVIDER')}}`,
        parameters: input,
    }};
    try {{
        const response = await axios.post(_url, data, {{
            timeout: 1000 * 60 * 6, // 6 minutes timeout
            headers: {{
                'Authorization': `Bearer ${{Deno.env.get('BEARER')}}`,
                'x-shinkai-tool-id': `${{Deno.env.get('X_SHINKAI_TOOL_ID')}}`,
                'x-shinkai-app-id': `${{Deno.env.get('X_SHINKAI_APP_ID')}}`,
                'x-shinkai-llm-provider': `${{Deno.env.get('X_SHINKAI_LLM_PROVIDER')}}`,
                'x-shinkai-agent-id': `${{Deno.env.get('X_SHINKAI_AGENT_ID')}}`
            }}
        }});
        return response.data;
    }} catch (error) {{
        return manageAxiosError(error);
    }}
}}
",
            tool.tool_router_key,
            tool.tool_type.to_lowercase()
        ));
    }

    // If SQL tables exist, generate a query function
    if !sql_tables.is_empty() {
        // Combine SQL documentation into a single format! macro
        typescript_output.push_str(&format!(
            "/**
 * Query the SQL database for results from {}
 * 
 * Available SQL Tables:
",
            function_name
        ));

        for table in sql_tables {
            typescript_output.push_str(&format!(" * {}\n * {}\n", table.name, table.definition));
        }

        if !sql_queries.is_empty() {
            typescript_output.push_str(" *\n * Example / Reference SQL Queries:\n");
            for query in sql_queries {
                typescript_output.push_str(&format!(" * {}\n * {}\n\n", query.name, query.query));
            }
        }

        // Combine parameter documentation
        typescript_output.push_str(
            " * 
* @param query - SQL query to execute
* @param params - Optional array of parameters for the query
* @returns Query results
*/
",
        );

        // Combine function definition and implementation
        typescript_output.push_str(&format!(
            "
async function query_{}(query: string, params?: any[]){}",
            function_name,
            if generate_dts {
                ";"
            } else {
                " {\n    return shinkaiSqliteQueryExecutor({query, params})\n}"
            }
        ));
    }

    typescript_output.push_str("\n");
    typescript_output
}

pub fn typescript_common_code() -> String {
    return r#"
import axios from 'npm:axios';
// deno-lint-ignore no-explicit-any
const tryToParseError = (data: any) => { try { return JSON.stringify(data); } catch (_) { return data; } };
// deno-lint-ignore no-explicit-any
const manageAxiosError = (error: any) => {
    // axios error management
    let message = '::NETWORK_ERROR::';
    if (error.response) {
        // The request was made and the server responded with a status code
        // that falls out of the range of 2xx
        message += ' ' + tryToParseError(error.response.data);
        message += ' ' + tryToParseError(error.response.status);
        message += ' ' + tryToParseError(error.response.headers);
    } else if (error.request) {
        // The request was made but no response was received
        // `error.request` is an instance of XMLHttpRequest in the browser and an instance of
        // http.ClientRequest in node.js
        message += ' ' + tryToParseError(error.request);
    } else {
        // Something happened in setting up the request that triggered an Error
        message += ' ' + tryToParseError(error.message);
    }
    message += ' ' + tryToParseError(error.config);
    throw new Error(message);
};
"#
    .to_owned();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_tools_primitives::tools::parameters::{Parameters, Property};

    #[test]
    fn test_generate_typescript_definition() {
        // Create a test tool header
        let tool = ShinkaiToolHeader {
            name: "Test Tool".to_string(),
            description: "A test tool for unit testing".to_string(),
            tool_router_key: "local:::test:::test_tool".to_string(),
            tool_type: "Deno".to_string(),
            mcp_enabled: Some(false),
            formatted_tool_summary_for_ui: "Test Tool Summary".to_string(),
            author: "test_author".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            input_args: {
                let mut params = Parameters::new();
                params.properties.insert(
                    "stringParam".to_string(),
                    Property::new("string".to_string(), "A string parameter".to_string(), None),
                );
                params.properties.insert(
                    "numberParam".to_string(),
                    Property::new("number".to_string(), "A number parameter".to_string(), None),
                );
                params.required.push("stringParam".to_string());
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

        // Generate TypeScript definition
        let ts_def = generate_typescript_definition(
            tool,
            tool_result,
            vec![], // No SQL tables
            vec![], // No SQL queries
            false,  // Not generating .d.ts
        );

        // Verify the output contains expected elements
        assert!(ts_def.contains("/**"));
        assert!(ts_def.contains("* A test tool for unit testing"));
        assert!(ts_def.contains("export async function testTool"));
        assert!(ts_def.contains("stringParam: string"));
        assert!(ts_def.contains("numberParam?: number"));
        assert!(ts_def.contains("result: string"));
        assert!(ts_def.contains("count: number"));
        assert!(ts_def.contains("Promise<{"));
        assert!(ts_def.contains("tool_router_key: 'local:::test:::test_tool'"));
    }

    #[test]
    fn test_generate_typescript_definition_with_sql() {
        // Create a test tool header similar to previous test
        let tool = ShinkaiToolHeader {
            name: "SQL Test Tool".to_string(),
            description: "A test tool with SQL capabilities".to_string(),
            tool_router_key: "local:::test:::sql_test_tool".to_string(),
            tool_type: "Deno".to_string(),
            mcp_enabled: Some(false),
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

        // Generate TypeScript definition
        let ts_def = generate_typescript_definition(tool, tool_result, sql_tables, sql_queries, false);

        // Verify SQL-related content
        assert!(ts_def.contains("Query the SQL database"));
        assert!(ts_def.contains("CREATE TABLE users"));
        assert!(ts_def.contains("SELECT * FROM users"));
        assert!(ts_def.contains("async function query_sqlTestTool"));
    }

    #[test]
    fn test_generate_typescript_dts() {
        // Test generating .d.ts declarations
        let tool = ShinkaiToolHeader {
            name: "DTS Test Tool".to_string(),
            description: "A test tool for .d.ts generation".to_string(),
            tool_router_key: "local:::test:::dts_test_tool".to_string(),
            tool_type: "Deno".to_string(),
            mcp_enabled: Some(false),
            formatted_tool_summary_for_ui: "DTS Test Tool Summary".to_string(),
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

        let ts_def = generate_typescript_definition(
            tool,
            tool_result,
            vec![],
            vec![],
            true, // Generate .d.ts
        );

        // Verify .d.ts specific content
        assert!(ts_def.contains("export async function dtsTestTool"));
        assert!(ts_def.contains("Promise<{"));
        assert!(!ts_def.contains("const _url")); // Implementation should not be included
        assert!(!ts_def.contains("axios.post")); // Implementation should not be included
    }

    #[test]
    fn test_json_type_to_typescript() {
        // Test basic types
        assert_eq!(json_type_to_typescript(&json!("string"), None), "string");
        assert_eq!(json_type_to_typescript(&json!("number"), None), "number");
        assert_eq!(json_type_to_typescript(&json!("integer"), None), "number");
        assert_eq!(json_type_to_typescript(&json!("boolean"), None), "boolean");
        assert_eq!(json_type_to_typescript(&json!("object"), None), "object");
        assert_eq!(json_type_to_typescript(&json!("any"), None), "any");

        // Test array types with different item types
        assert_eq!(
            json_type_to_typescript(&json!("array"), Some(&json!({"type": "string"}))),
            "string[]"
        );
        assert_eq!(
            json_type_to_typescript(&json!("array"), Some(&json!({"type": "number"}))),
            "number[]"
        );
        assert_eq!(
            json_type_to_typescript(&json!("array"), Some(&json!({"type": "boolean"}))),
            "boolean[]"
        );
        assert_eq!(json_type_to_typescript(&json!("array"), None), "any[]");

        // Test with $ref types
        assert_eq!(
            json_type_to_typescript(&json!("array"), Some(&json!({"$ref": "CustomType"}))),
            "CustomType[]"
        );
    }

    #[test]
    fn test_complex_nested_types() {
        let tool = ShinkaiToolHeader {
            name: "Complex Types Tool".to_string(),
            description: "A tool with complex nested types".to_string(),
            tool_router_key: "local:::test:::complex_types_tool".to_string(),
            tool_type: "Deno".to_string(),
            mcp_enabled: Some(false),
            formatted_tool_summary_for_ui: "Complex Types Tool".to_string(),
            author: "test_author".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            input_args: {
                let mut params = Parameters::new();
                params.properties.insert(
                    "arrayOfObjects".to_string(),
                    Property::new("array".to_string(), "Array of objects".to_string(), None),
                );
                params.properties.insert(
                    "nestedObject".to_string(),
                    Property::new("object".to_string(), "A nested object".to_string(), None),
                );
                params
            },
            output_arg: shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg { json: "{}".to_string() },
            config: None,
            usage_type: None,
            tool_offering: None,
        };

        let tool_result = ToolResult::new(
            "object".to_string(),
            json!({
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object"
                    }
                },
                "metadata": {
                    "type": "object"
                }
            }),
            vec![],
        );

        let ts_def = generate_typescript_definition(tool, tool_result, vec![], vec![], false);

        assert!(ts_def.contains("arrayOfObjects?: any[]"));
        assert!(ts_def.contains("nestedObject?: object"));
        assert!(ts_def.contains("items: object[]"));
        assert!(ts_def.contains("metadata: object"));
    }

    #[test]
    fn test_empty_and_optional_parameters() {
        let tool = ShinkaiToolHeader {
            name: "Optional Params Tool".to_string(),
            description: "A tool with optional parameters".to_string(),
            tool_router_key: "local:::test:::optional_params_tool".to_string(),
            tool_type: "Deno".to_string(),
            mcp_enabled: Some(false),
            formatted_tool_summary_for_ui: "Optional Params Tool".to_string(),
            author: "test_author".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            input_args: {
                let mut params = Parameters::new();
                // No required parameters
                params.properties.insert(
                    "optionalString".to_string(),
                    Property::new("string".to_string(), "Optional string".to_string(), None),
                );
                params.properties.insert(
                    "optionalNumber".to_string(),
                    Property::new("number".to_string(), "Optional number".to_string(), None),
                );
                params
            },
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
            vec![],
        );

        let ts_def = generate_typescript_definition(tool, tool_result, vec![], vec![], false);

        assert!(ts_def.contains("optionalString?: string"));
        assert!(ts_def.contains("optionalNumber?: number"));
        assert!(!ts_def.contains("required")); // No required parameters
    }

    #[test]
    fn test_function_name_generation() {
        // Test various tool names and their expected function names
        let test_cases = vec![
            ("Simple Tool", "simpleTool"),
            ("complex-tool-name", "complexToolName"),
            ("UPPERCASE_TOOL", "uppercaseTool"),
            ("snake_case_tool", "snakeCaseTool"),
            ("Tool With Spaces", "toolWithSpaces"),
            ("tool123", "tool123"),
            ("123tool", "fn23tool"),
            ("tool-with-hyphens", "toolWithHyphens"),
        ];

        for (input, expected) in test_cases {
            let tool = ShinkaiToolHeader {
                name: input.to_string(),
                description: "Test tool".to_string(),
                tool_router_key: "test".to_string(),
                tool_type: "Deno".to_string(),
                formatted_tool_summary_for_ui: "Test".to_string(),
                author: "test".to_string(),
                version: "1.0.0".to_string(),
                mcp_enabled: Some(false),
                enabled: true,
                input_args: Parameters::new(),
                output_arg: shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg { json: "{}".to_string() },
                config: None,
                usage_type: None,
                tool_offering: None,
            };

            let function_name = create_function_name_set(&tool);
            assert_eq!(function_name, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_typescript_common_code() {
        let common_code = typescript_common_code();

        // Verify essential imports and utilities
        assert!(common_code.contains("import axios from 'npm:axios'"));
        assert!(common_code.contains("const tryToParseError"));
        assert!(common_code.contains("const manageAxiosError"));

        // Verify error handling
        assert!(common_code.contains("::NETWORK_ERROR::"));
        assert!(common_code.contains("error.response"));
        assert!(common_code.contains("error.request"));
        assert!(common_code.contains("error.message"));

        // Verify TypeScript-specific elements
        assert!(common_code.contains("// deno-lint-ignore no-explicit-any"));
    }
}
