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
                'x-shinkai-llm-provider': `${Deno.env.get('X_SHINKAI_LLM_PROVIDER')}`
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
    if let Ok(output_schema) = serde_json::from_str::<Value>(&tool.output_arg.json) {
        if let Some(properties) = output_schema.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_value) in properties {
                let type_str = json_type_to_typescript(
                    prop_value.get("type").unwrap_or(&Value::String("any".to_string())),
                    prop_value.get("items"),
                );
                typescript_output.push_str(&format!(" *   {}: {};\n", prop_name, type_str));
            }
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
    if let Ok(output_schema) = serde_json::from_str::<Value>(&tool.output_arg.json) {
        if let Some(properties) = output_schema.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_value) in properties {
                let type_str = json_type_to_typescript(
                    prop_value.get("type").unwrap_or(&Value::String("any".to_string())),
                    prop_value.get("items"),
                );
                typescript_output.push_str(&format!("    {}: {};\n", prop_name, type_str));
            }
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
                'x-shinkai-llm-provider': `${{Deno.env.get('X_SHINKAI_LLM_PROVIDER')}}`
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
                " {\n    return shinkaiSqliteQueryExecutor(query, params)\n}"
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
