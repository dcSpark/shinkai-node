use crate::tools::deno_tools::DenoTool;
use crate::tools::error::ToolError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use shinkai_tools_runner::tools::tool_definition::ToolDefinition;
use shinkai_vector_resources::embeddings::Embedding;

use super::{
    argument::{ToolArgument, ToolOutputArg},
    deno_tools::DenoToolResult,
    tool_config::{BasicConfig, ToolConfig},
};

/// A JSToolkit is a collection of JSTools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JSToolkit {
    pub name: String,
    pub tools: Vec<DenoTool>,
    pub author: String,
    pub version: String,
}

impl JSToolkit {
    /// Creates a new JSToolkit with the provided name and definitions, and default values for other fields.
    pub fn new(name: &str, definitions: Vec<ToolDefinition>) -> Self {
        let tools = definitions
            .clone()
            .into_iter()
            .map(|def| Self::create_js_tool(name, def))
            .collect();

        Self {
            name: name.to_string(),
            tools,
            author: definitions.first().map_or("".to_string(), |d| d.author.clone()),
            version: "1.0.0".to_string(), // Dummy version
        }
    }

    fn create_js_tool(toolkit_name: &str, definition: ToolDefinition) -> DenoTool {
        let input_args = Self::extract_input_args(&definition);
        let output_arg = Self::extract_output_arg(&definition);
        let config = Self::extract_config(&definition);
        let tool_name = Self::generate_tool_name(&definition.name);

        let result = DenoToolResult {
            r#type: definition.result["type"].as_str().unwrap_or("object").to_string(),
            properties: definition.result["properties"].clone(),
            required: definition.result["required"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        };

        DenoTool {
            toolkit_name: toolkit_name.to_string(),
            name: tool_name,
            author: definition.author.clone(),
            config,
            js_code: definition.code.clone().unwrap_or_default(),
            tools: None,
            description: definition.description.clone(),
            keywords: definition.keywords.clone(),
            input_args,
            output_arg,
            activated: false,
            embedding: definition.embedding_metadata.clone().map(|meta| Embedding {
                id: "".to_string(),
                vector: meta.embeddings,
            }),
            result,
            sql_tables: None,
            sql_queries: None,
        }
    }

    fn generate_tool_name(name: &str) -> String {
        let name_pattern = Regex::new(r"[^a-zA-Z0-9_-]").unwrap();
        name_pattern.replace_all(name, "_").to_lowercase()
    }

    fn extract_output_arg(definition: &ToolDefinition) -> ToolOutputArg {
        ToolOutputArg {
            json: definition.result.to_string(),
        }
    }

    fn extract_input_args(definition: &ToolDefinition) -> Vec<ToolArgument> {
        if let Some(parameters) = definition.parameters.as_object() {
            parameters["properties"].as_object().map_or(vec![], |props| {
                props
                    .iter()
                    .map(|(key, value)| ToolArgument {
                        name: key.clone(),
                        arg_type: value["type"].as_str().unwrap_or("string").to_string(),
                        description: value["description"].as_str().unwrap_or("").to_string(),
                        is_required: definition.parameters["required"]
                            .as_array()
                            .map_or(false, |req| req.iter().any(|r| r == key)),
                    })
                    .collect()
            })
        } else {
            vec![]
        }
    }

    fn extract_config(definition: &ToolDefinition) -> Vec<ToolConfig> {
        if let Some(configurations) = definition.configurations.as_object() {
            configurations["properties"].as_object().map_or(vec![], |props| {
                props
                    .iter()
                    .map(|(key, value)| {
                        ToolConfig::BasicConfig(BasicConfig {
                            key_name: key.clone(),
                            description: value["description"].as_str().unwrap_or("").to_string(),
                            required: definition.configurations["required"]
                                .as_array()
                                .map_or(false, |req| req.iter().any(|r| r == key)),
                            key_value: None,
                        })
                    })
                    .collect()
            })
        } else {
            vec![]
        }
    }

    /// Generate the key that this toolkit will be stored under in the tool router
    pub fn gen_router_key(name: &str, author: &str) -> String {
        // We replace any `/` in order to not have the names break VRPaths
        format!("{}:::{}", author, name).replace('/', "|")
    }

    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_tools_runner::tools::tool_definition::ToolDefinition;

    #[test]
    fn test_new_jstoolkit() {
        let definition = ToolDefinition {
            id: "shinkai-tool-weather-by-city".to_string(),
            name: "Shinkai: Weather By City".to_string(),
            description: "Get weather information for a city name".to_string(),
            configurations: json!({
                "type": "object",
                "properties": {
                    "apiKey": {
                        "type": "string"
                    }
                },
                "required": ["apiKey"]
            }),
            parameters: json!({
                "type": "object",
                "properties": {
                    "city": {
                        "type": "string"
                    }
                },
                "required": ["city"]
            }),
            result: json!({
                "type": "object",
                "properties": {
                    "weather": {
                        "type": "string"
                    }
                },
                "required": ["weather"]
            }),
            author: "".to_string(),
            keywords: vec![],
            code: Some("var tool;\n/******/ (() => { // webpackBootstrap\n/*".to_string()),
            embedding_metadata: None,
        };

        let toolkit = JSToolkit::new("Weather Toolkit", vec![definition]);

        assert_eq!(toolkit.name, "Weather Toolkit");
        assert_eq!(toolkit.tools.len(), 1);
        let tool = &toolkit.tools[0];
        assert_eq!(tool.name, "shinkai__weather_by_city");
        assert_eq!(tool.description, "Get weather information for a city name");
        assert_eq!(tool.js_code, "var tool;\n/******/ (() => { // webpackBootstrap\n/*");
        assert_eq!(tool.input_args.len(), 1);
        assert_eq!(tool.input_args[0].name, "city");
        assert_eq!(tool.input_args[0].arg_type, "string");
        assert!(tool.input_args[0].is_required);

        // Check for config
        assert_eq!(tool.config.len(), 1);
        let config = &tool.config[0];
        if let ToolConfig::BasicConfig(basic_config) = config {
            assert_eq!(basic_config.key_name, "apiKey");
            assert_eq!(basic_config.description, "");
            assert!(basic_config.required);
            assert_eq!(basic_config.key_value, None);
        } else {
            panic!("Expected BasicConfig");
        }
    }
}
