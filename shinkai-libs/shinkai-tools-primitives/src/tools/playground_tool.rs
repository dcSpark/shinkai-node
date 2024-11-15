use super::{argument::ToolArgument, deno_tools::JSToolResult, tool_config::{BasicConfig, ToolConfig}};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaygroundTool {
    pub metadata: PlaygroundToolMetadata,
    pub tool_router_key: Option<String>,
    pub job_id: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaygroundToolMetadata {
    pub name: String,
    pub description: String,
    pub author: String,
    pub keywords: Vec<String>,
    #[serde(deserialize_with = "deserialize_configurations")]
    pub configurations: Vec<ToolConfig>,
    #[serde(deserialize_with = "deserialize_parameters")]
    pub parameters: Vec<ToolArgument>,
    pub result: JSToolResult,
}

fn deserialize_configurations<'de, D>(deserializer: D) -> Result<Vec<ToolConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
        JsonValue::Array(configs) => {
            // If it's already an array, assume it's a list of ToolConfig objects
            let tool_configs: Vec<ToolConfig> = configs
                .into_iter()
                .map(|config| {
                    // Assuming each config is a valid ToolConfig JSON object
                    serde_json::from_value(config).map_err(serde::de::Error::custom)
                })
                .collect::<Result<_, _>>()?;
            Ok(tool_configs)
        }
        JsonValue::Object(config_obj) => {
            if let Some(JsonValue::Object(properties)) = config_obj.get("properties") {
                let required_keys: Vec<String> = config_obj
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let configs = properties
                    .iter()
                    .map(|(key, val)| {
                        let description = val.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let required = required_keys.contains(key);
                        let basic_config = BasicConfig {
                            key_name: key.clone(),
                            description,
                            required,
                            key_value: None, // or extract a default value if needed
                        };
                        ToolConfig::BasicConfig(basic_config)
                    })
                    .collect();

                return Ok(configs);
            }
            Err(serde::de::Error::custom("Invalid object structure for configurations"))
        }
        _ => Err(serde::de::Error::custom("Invalid type for configurations")),
    }
}

fn deserialize_parameters<'de, D>(deserializer: D) -> Result<Vec<ToolArgument>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
        JsonValue::Array(params) => {
            // If it's already an array, assume it's a list of ToolArgument objects
            let tool_arguments: Vec<ToolArgument> = params
                .into_iter()
                .map(|param| {
                    // Assuming each param is a valid ToolArgument JSON object
                    serde_json::from_value(param).map_err(serde::de::Error::custom)
                })
                .collect::<Result<_, _>>()?;
            Ok(tool_arguments)
        }
        JsonValue::Object(param_obj) => {
            if let Some(JsonValue::Object(properties)) = param_obj.get("properties") {
                let required_keys: Vec<String> = param_obj
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let arguments = properties
                    .iter()
                    .map(|(key, val)| {
                        let arg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let description = val.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let is_required = required_keys.contains(key);
                        ToolArgument::new(key.clone(), arg_type, description, is_required)
                    })
                    .collect();

                return Ok(arguments);
            }
            Err(serde::de::Error::custom("Invalid object structure for parameters"))
        }
        _ => Err(serde::de::Error::custom("Invalid type for parameters")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_deserialize_playground_tool() {
        let json_data = r#"
        {
            "metadata": {
                "name": "Example Tool",
                "description": "An example tool for testing",
                "author": "Author Name",
                "keywords": ["example", "test"],
                "configurations": [],
                "parameters": [],
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                }
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "code": "console.log('Hello, world!');"
        }
        "#;

        let deserialized: PlaygroundTool = serde_json::from_str(json_data).expect("Failed to deserialize");
        
        assert_eq!(deserialized.metadata.name, "Example Tool");
        assert_eq!(deserialized.tool_router_key, Some("example_key".to_string()));
        assert_eq!(deserialized.job_id, "job_123");
        assert_eq!(deserialized.code, "console.log('Hello, world!');");
    }

    #[test]
    fn test_deserialize_playground_tool_with_coinbase_data() {
        let json_data = r#"
        {
            "tool_router_key": null,
            "metadata": {
                "id": "shinkai-tool-coinbase-create-wallet",
                "name": "Shinkai: Coinbase Wallet Creator",
                "description": "Tool for creating a Coinbase wallet",
                "author": "Shinkai",
                "keywords": [
                    "coinbase",
                    "wallet",
                    "creator",
                    "shinkai"
                ],
                "configurations": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string"
                        },
                        "privateKey": {
                            "type": "string"
                        },
                        "useServerSigner": {
                            "type": "string",
                            "default": "false",
                            "nullable": true
                        }
                    },
                    "required": [
                        "name",
                        "privateKey"
                    ]
                },
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "result": {
                    "type": "object",
                    "properties": {
                        "walletId": {
                            "type": "string",
                            "nullable": true
                        },
                        "seed": {
                            "type": "string",
                            "nullable": true
                        },
                        "address": {
                            "type": "string",
                            "nullable": true
                        }
                    },
                    "required": []
                }
            },
            "job_id": "123",
            "code": "import { shinkaiDownloadPages } from '@shinkai/local-tools'; type CONFIG = {}; type INPUTS = { urls: string[] }; type OUTPUT = { markdowns: string[] }; export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> { const { urls } = inputs; if (!urls || urls.length === 0) { throw new Error('URL list is required'); } return shinkaiDownloadPages(urls); }"
        }
        "#;

        let deserialized: PlaygroundTool = serde_json::from_str(json_data).expect("Failed to deserialize");

        assert_eq!(deserialized.metadata.name, "Shinkai: Coinbase Wallet Creator");
        assert_eq!(deserialized.metadata.description, "Tool for creating a Coinbase wallet");
        assert_eq!(deserialized.metadata.author, "Shinkai");
        assert_eq!(deserialized.metadata.keywords, vec!["coinbase", "wallet", "creator", "shinkai"]);
        assert_eq!(deserialized.tool_router_key, None);
        assert_eq!(deserialized.job_id, "123");
        assert_eq!(deserialized.code, "import { shinkaiDownloadPages } from '@shinkai/local-tools'; type CONFIG = {}; type INPUTS = { urls: string[] }; type OUTPUT = { markdowns: string[] }; export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> { const { urls } = inputs; if (!urls || urls.length === 0) { throw new Error('URL list is required'); } return shinkaiDownloadPages(urls); }");
    }
}

