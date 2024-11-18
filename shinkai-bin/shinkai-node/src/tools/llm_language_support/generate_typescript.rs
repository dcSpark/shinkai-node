// Example output:
/*
    /**
     * Analyzes text and provides statistics
     * @param text - (required, The text to analyze)
     * @param include_sentiment - (optional, Whether to include sentiment analysis) , default: undefined
     * @returns {{
     *   word_count: integer - Number of words in the text
     *   character_count: integer - Number of characters in the text
     *   sentiment_score: number - Sentiment score (-1 to 1) if requested
     * }}
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
use super::language_helpers::to_camel_case;
use crate::utils::environment::fetch_node_environment;
use serde_json::Value;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiToolHeader;

fn json_type_to_typescript(type_value: &Value, items_value: Option<&Value>) -> String {
    match type_value.as_str() {
        Some("array") => {
            if let Some(items) = items_value {
                if let Some(item_type) = items.get("type") {
                    format!("{}[]", json_type_to_typescript(item_type, items.get("items")))
                } else {
                    "any[]".to_string()
                }
            } else {
                "any[]".to_string()
            }
        }
        Some(t) => t.to_string(),
        None => "any".to_string(),
    }
}

pub fn generate_typescript_definition(tool: ShinkaiToolHeader, generate_dts: bool) -> String {
    let mut typescript_output = String::new();
    let node_env = fetch_node_environment();
    let api_port = node_env.api_listen_address.port();

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

    let function_name = to_camel_case(&tool.name);

    // Add 'export' and 'declare' for .d.ts files
    if generate_dts {
        typescript_output.push_str(&format!("export async function {}(", function_name));
    } else {
        typescript_output.push_str(&format!("async function {}(", function_name));
    }

    // Generate function parameters
    let params: Vec<String> = tool
        .input_args
        .iter()
        .map(|arg| {
            let type_str = &arg.arg_type;
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
        typescript_output.push_str(&format!(
            "    const _url = 'http://localhost:{}/v2/tool_execution';\n",
            api_port
        ));
        typescript_output.push_str("    const data = {\n");
        typescript_output.push_str(&format!("        tool_router_key: '{}',\n", tool.tool_router_key));
        typescript_output.push_str(&format!("        tool_type: '{}',\n", tool.tool_type.to_lowercase()));
        typescript_output.push_str("        parameters: {\n");
        for arg in &tool.input_args {
            typescript_output.push_str(&format!("            {}: {},\n", arg.name, arg.name));
        }
        typescript_output.push_str("        },\n");
        typescript_output.push_str("    };\n");
        typescript_output.push_str("    const response = await axios.post(_url, data, {\n");
        typescript_output.push_str("        headers: {\n");
        typescript_output.push_str("            'Authorization': `Bearer ${process.env.BEARER}`,\n");
        typescript_output.push_str("            'x-shinkai-tool-id': process.env.X_SHINKAI_TOOL_ID,\n");
        typescript_output.push_str("            'x-shinkai-app-id': process.env.X_SHINKAI_APP_ID\n");
        typescript_output.push_str("        }\n");
        typescript_output.push_str("    });\n");
        typescript_output.push_str("    return response.data;\n");
        typescript_output.push_str("}\n");
    }

    typescript_output.push_str("\n");
    typescript_output
}
