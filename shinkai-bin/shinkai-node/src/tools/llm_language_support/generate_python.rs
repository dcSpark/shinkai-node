// TODO These definitions are not correct.
// Example output:
/*
class GetWeatherResponse(TypedDict):
    temperature: float
    conditions: str

def get_weather(location: str, units: Optional[str] = None) -> GetWeatherResponse:
    """
    Get the current weather for a location

    Args:
        location: The city or location to get weather for
        units: Temperature units (celsius/fahrenheit)

    Returns:
        Response object
    """
    url = 'http://localhost:9550/v2/tool_execution'
    data = {
        'tool_router_key': f'local:::shinkai-tool-{location.lower()}:::shinkai_{location.lower()}',
        'parameters': {
            'location': location,
            'units': units,
        },
    }
    headers = {
        'Authorization': f'Bearer {os.environ.get("BEARER")}'
    }
    response = requests.post(url, json=data, headers=headers)
    response.raise_for_status()
    return response.json()
*/

use serde_json::Value;
use shinkai_tools_runner::tools::tool_definition::ToolDefinition;
use super::language_helpers::to_camel_case;
use super::language_helpers::to_snake_case;

pub fn generate_python_definition(name: String, runner_def: &ToolDefinition) -> String {
    let mut python_output = String::new();
    
    let response_class = format!("{}Response", 
        to_camel_case(&runner_def.name)
            .replace(' ', "")
            .replace('-', "")
            .replace(':', "")
    );
    
    python_output.push_str(&format!("class {}(TypedDict):\n", response_class));
    if let Some(properties) = runner_def.result.get("properties").and_then(|v| v.as_object()) {
        for (prop_name, prop_value) in properties {
            let type_str = match prop_value.get("type").and_then(|t| t.as_str()) {
                Some("string") => "str",
                Some("number") => "float",
                Some("integer") => "int",
                Some("boolean") => "bool",
                Some("array") => "list",
                Some("object") => "dict",
                _ => "any",
            };
            python_output.push_str(&format!("    {}: {}\n", prop_name, type_str));
        }
    }
    python_output.push_str("\n");

    let function_name = to_snake_case(
        &runner_def.name
            .replace("Shinkai: ", "")
            .replace(':', "_")
    );
    
    python_output.push_str(&format!("def {}(", function_name));
    
    if let Some(properties) = runner_def.parameters.get("properties").and_then(|v| v.as_object()) {
        let required = runner_def.parameters.get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>())
            .unwrap_or_default();

        let params: Vec<String> = properties.iter()
            .map(|(param_name, param_value)| {
                let type_str = match param_value.get("type").and_then(|t| t.as_str()) {
                    Some("string") => "str",
                    Some("number") => "float",
                    Some("integer") => "int",
                    Some("boolean") => "bool",
                    Some("array") => "list",
                    Some("object") => "dict",
                    _ => "any",
                };
                
                if required.contains(&param_name.as_str()) {
                    format!("{}: {}", param_name, type_str)
                } else {
                    format!("{}: Optional[{}] = None", param_name, type_str)
                }
            })
            .collect();
        python_output.push_str(&params.join(", "));
    }
    python_output.push_str(&format!(") -> {response_class}:\n"));

    python_output.push_str("    \"\"\"\n");
    python_output.push_str(&format!("    {}\n\n", runner_def.description));
    
    if let Some(properties) = runner_def.parameters.get("properties").and_then(|v| v.as_object()) {
        for (param_name, param_value) in properties {
            let desc = param_value.get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            python_output.push_str(&format!("    Args:\n        {}: {}\n", param_name, desc));
        }
    }
    python_output.push_str("\n    Returns:\n        Response object\n    \"\"\"\n");

    let url_function_name = function_name.replace(' ', "_");
    python_output.push_str(&format!("    url = 'http://localhost:9550/v2/tool_execution'\n"));
    python_output.push_str("    data = {\n");
    python_output.push_str(&format!("        'tool_router_key': 'local:::shinkai-tool-{}:::shinkai__{}',\n",
        name.to_lowercase(),
        name.to_lowercase()
    ));
    python_output.push_str("        'parameters': {\n");
    if let Some(properties) = runner_def.parameters.get("properties").and_then(|v| v.as_object()) {
        for (param_name, _) in properties {
            python_output.push_str(&format!("            '{}': {},\n", param_name, param_name));
        }
    }
    python_output.push_str("        },\n");
    python_output.push_str("    }\n");
    python_output.push_str("    headers = {\n");
    python_output.push_str("        'Authorization': f'Bearer {os.environ.get(\"BEARER\")}'\n");
    python_output.push_str("    }\n");
    python_output.push_str("    response = requests.post(url, json=data, headers=headers)\n");
    python_output.push_str("    response.raise_for_status()\n");
    python_output.push_str("    return response.json()\n\n");

    python_output
} 