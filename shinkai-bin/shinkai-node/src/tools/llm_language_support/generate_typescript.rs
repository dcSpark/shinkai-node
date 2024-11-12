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
use crate::utils::environment::fetch_node_environment;
use serde_json::Value;
use shinkai_http_api::api_v2::api_v2_handlers_tools::ToolType;
use shinkai_tools_runner::tools::tool_definition::ToolDefinition;

use super::language_helpers::to_camel_case;
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

pub fn generate_typescript_definition(
    tool_type: ToolType,
    name: String,
    runner_def: &ToolDefinition,
    tool_result: Option<&ShinkaiToolHeader>,
) -> String {
    let mut typescript_output = String::new();
    let node_env = fetch_node_environment();
    let api_port = node_env.api_listen_address.port();

    typescript_output.push_str("/**\n");
    typescript_output.push_str(&format!(" * {}\n", runner_def.description));

    if let Some(properties) = runner_def.parameters.get("properties").and_then(|v| v.as_object()) {
        for (param_name, param_value) in properties {
            let required = runner_def
                .parameters
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(param_name)))
                .unwrap_or(false);

            let desc = param_value.get("description").and_then(|d| d.as_str()).unwrap_or("");

            typescript_output.push_str(&format!(
                " * @param {} - ({}{}) {}\n",
                param_name,
                if required { "required" } else { "optional" },
                if !desc.is_empty() {
                    format!(", {}", desc)
                } else {
                    String::new()
                },
                if required { "" } else { ", default: undefined" }
            ));
        }
    }

    // Generate return type documentation
    typescript_output.push_str(" * @returns {{\n");
    if let Some(properties) = runner_def.result.get("properties").and_then(|v| v.as_object()) {
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
    typescript_output.push_str(" * }}\n");
    typescript_output.push_str(" */\n");

    let function_name = to_camel_case(&name);
    typescript_output.push_str(&format!("async function {}(", function_name));

    // Generate function parameters
    if let Some(properties) = runner_def.parameters.get("properties").and_then(|v| v.as_object()) {
        let params: Vec<String> = properties
            .iter()
            .map(|(param_name, param_value)| {
                let required = runner_def
                    .parameters
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.iter().any(|v| v.as_str() == Some(param_name)))
                    .unwrap_or(false);
                let type_str = json_type_to_typescript(
                    param_value.get("type").unwrap_or(&Value::String("any".to_string())),
                    param_value.get("items"),
                );
                if required {
                    format!("{}: {}", param_name, type_str)
                } else {
                    format!("{}?: {}", param_name, type_str)
                }
            })
            .collect();
        typescript_output.push_str(&params.join(", "));
    }

    // Generate return type inline
    typescript_output.push_str("): Promise<{\n");
    if let Some(properties) = runner_def.result.get("properties").and_then(|v| v.as_object()) {
        for (prop_name, prop_value) in properties {
            let type_str = json_type_to_typescript(
                prop_value.get("type").unwrap_or(&Value::String("any".to_string())),
                prop_value.get("items"),
            );
            typescript_output.push_str(&format!("    {}: {};\n", prop_name, type_str));
        }
    }
    typescript_output.push_str("}> {\n");

    let tool_router_key = if let Some(header) = tool_result {
        format!("{}", header.tool_router_key)
    } else {
        format!("internal:::{}", name.to_lowercase())
    };

    typescript_output.push_str(&format!(
        "    const _url = 'http://localhost:{}/v2/tool_execution';\n",
        api_port
    ));
    typescript_output.push_str("    const data = {\n");
    typescript_output.push_str(&format!("        tool_router_key: '{}',\n", tool_router_key));
    typescript_output.push_str(&format!("        tool_type: '{}',\n", tool_type));
    typescript_output.push_str("        parameters: {\n");
    if let Some(properties) = runner_def.parameters.get("properties").and_then(|v| v.as_object()) {
        for (param_name, _) in properties {
            typescript_output.push_str(&format!("            {}: {},\n", param_name, param_name));
        }
    }
    typescript_output.push_str("        },\n");
    typescript_output.push_str("    };\n");
    typescript_output.push_str("    const response = await axios.post(_url, data, {\n");
    typescript_output.push_str("        headers: {\n");
    typescript_output.push_str("            'Authorization': `Bearer ${process.env.BEARER}`\n");
    typescript_output.push_str("        }\n");
    typescript_output.push_str("    });\n");
    typescript_output.push_str("    return response.data;\n");
    typescript_output.push_str("}\n\n");

    typescript_output
}
