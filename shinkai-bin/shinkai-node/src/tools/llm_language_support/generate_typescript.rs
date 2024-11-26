// Example output:
/*
    /**
     * Analyzes text and provides statistics
     * @param text - (required, The text to analyze)
     * @param include_sentiment - (optional, Whether to include sentiment analysis) , default: undefined
     * @returns {
     *   word_count: integer - Number of words in the text
     *   character_count: integer - Number of characters in the text
     *   sentiment_score: number - Sentiment score (-1 to 1) if requested
     * }
     */
    async function textAnalyzer(text: string, include_sentiment?: boolean): Promise<{
        word_count: integer;
        character_count: integer;
        sentiment_score: number;
    }> {
        const _url = 'http://localhost:9950/v2/tool_execution';
        const data = {
            tool_router_key: 'internal:::text_analyzer',
            tool_type: 'js',
            parameters: {
                text: text,
                include_sentiment: include_sentiment,
            },
        };
        const response = await axios.post(_url, data, {
            headers: {
                'Authorization': `Bearer ${process.env.BEARER}`
            }
        });
        return response.data;
    }
*/
use crate::utils::environment::fetch_node_environment;
use serde_json::Value;
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolHeader, tool_playground::ToolPlayground};

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
        Some("object") => {
            // Handle object types with properties if available
            if let Some(properties) = type_value.get("properties") {
                "object".to_string() // Could be expanded to generate interface
            } else {
                "object".to_string()
            }
        }
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
    generate_dts: bool,
    tool_playground: Option<ToolPlayground>,
) -> String {
    let mut typescript_output = String::new();
    let node_env = fetch_node_environment();
    let api_port = node_env.api_listen_address.port();

    // Create function name before using tool
    let function_name = create_function_name_set(&tool);

    typescript_output.push_str("/**\n");
    typescript_output.push_str(&format!(" * {}\n", tool.description));

    // Generate parameter documentation
    for arg in &tool.input_args {
        typescript_output.push_str(&format!(
            " * @param {} - ({}{}) {}\n",
            arg.name,
            if arg.is_required { "required" } else { "optional" },
            if !arg.description.is_empty() {
                format!(", {}", arg.description)
            } else {
                String::new()
            },
            if arg.is_required { "" } else { ", default: undefined" }
        ));
    }

    // Generate return type documentation
    typescript_output.push_str(" * @returns {\n");
    // Parse the output_arg.json to get the return type properties
    if let Ok(output_schema) = serde_json::from_str::<Value>(&tool.output_arg.json) {
        if let Some(properties) = output_schema.get("properties").and_then(|v| v.as_object()) {
            for (prop_name, prop_value) in properties {
                let type_str = json_type_to_typescript(
                    prop_value.get("type").unwrap_or(&Value::String("any".to_string())),
                    prop_value.get("items"),
                );
                let desc = prop_value.get("description").and_then(|d| d.as_str()).unwrap_or("");
                typescript_output.push_str(&format!(
                    " *   {}: {} {}\n",
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
    typescript_output.push_str(" * }\n");

    typescript_output.push_str(" */\n");

    typescript_output.push_str(&format!("export async function {}(", function_name));

    // Generate function parameters
    let params: Vec<String> = tool
        .input_args
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

    // Generate return type inline
    typescript_output.push_str("): Promise<{\n");
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
        typescript_output.push_str(r#"    const _url = `${Deno.env.get('SHINKAI_NODE_LOCATION')}/v2/tool_execution`;"#);
        typescript_output.push_str("");
        typescript_output.push_str("    const data = {\n");
        typescript_output.push_str(&format!("        tool_router_key: '{}',\n", tool.tool_router_key));
        typescript_output.push_str(&format!("        tool_type: '{}',\n", tool.tool_type.to_lowercase()));
        typescript_output.push_str("        llm_provider: `${Deno.env.get('X_SHINKAI_LLM_PROVIDER')}`,\n");
        typescript_output.push_str("        parameters: {\n");
        for arg in &tool.input_args {
            typescript_output.push_str(&format!("            {}: {},\n", arg.name, arg.name));
        }
        typescript_output.push_str("        },\n");
        typescript_output.push_str("    };\n");
        typescript_output.push_str("    try {\n");
        typescript_output.push_str("    const response = await axios.post(_url, data, {\n");
        typescript_output.push_str("        headers: {\n");
        typescript_output.push_str("            'Authorization': `Bearer ${Deno.env.get('BEARER')}`,\n");
        typescript_output.push_str("            'x-shinkai-tool-id': `${Deno.env.get('X_SHINKAI_TOOL_ID')}`,\n");
        typescript_output.push_str("            'x-shinkai-app-id': `${Deno.env.get('X_SHINKAI_APP_ID')}`,\n");
        typescript_output
            .push_str("            'x-shinkai-llm-provider': `${Deno.env.get('X_SHINKAI_LLM_PROVIDER')}`\n");
        typescript_output.push_str("        }\n");
        typescript_output.push_str("    });\n");
        typescript_output.push_str("    return response.data;\n");
        typescript_output.push_str("    } catch (error) {\n");
        typescript_output.push_str("        return manageAxiosError(error);\n");
        typescript_output.push_str("    }\n");
        typescript_output.push_str("}\n");
    }

    // If SQL tables exist, generate a query function
    if let Some(playground) = &tool_playground {
        if !playground.metadata.sql_tables.is_empty() {
            typescript_output.push_str("\n");
            typescript_output.push_str("/**\n");
            typescript_output.push_str(&format!(
                " * Query the SQL database for results from {}\n",
                function_name
            ));
            typescript_output.push_str(" * \n");
            typescript_output.push_str(" * Available SQL Tables:\n");
            for table in &playground.metadata.sql_tables {
                typescript_output.push_str(&format!(" * {}\n", table.name));
                typescript_output.push_str(&format!(" * {}\n", table.definition));
            }

            if !playground.metadata.sql_queries.is_empty() {
                typescript_output.push_str(" * \n");
                typescript_output.push_str(" * Example / Reference SQL Queries:\n");
                for query in &playground.metadata.sql_queries {
                    typescript_output.push_str(&format!(" * {}\n", query.name));
                    typescript_output.push_str(&format!(" * {}\n", query.query));
                }
            }
            typescript_output.push_str(" * \n");
            typescript_output.push_str(" * @param query - SQL query to execute\n");
            typescript_output.push_str(" * @param params - Optional array of parameters for the query\n");
            typescript_output.push_str(" * @returns Query results\n");
            typescript_output.push_str(" */\n");

            let function_name = create_function_name_set(&tool);
            typescript_output.push_str(&format!(
                "async function query_{}(query: string, params?: any[]) {{\n",
                function_name
            ));
            // TODO: make this dynamic. This should be defined by the tool key path (?)
            let db_name = "default";
            typescript_output.push_str(&format!(
                "    return localRustToolkitShinkaiSqliteQueryExecutor('{}', query, params);\n",
                db_name
            ));
            typescript_output.push_str("}\n");
        }
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
