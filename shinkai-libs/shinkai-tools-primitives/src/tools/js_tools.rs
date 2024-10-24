use std::collections::HashMap;
use std::path::PathBuf;
use std::{env, thread};

use super::js_toolkit_headers::ToolConfig;
use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;
use shinkai_tools_runner::tools::run_result::RunResult;
use shinkai_tools_runner::tools::shinkai_tools_backend_options::ShinkaiToolsBackendOptions;
use shinkai_tools_runner::tools::tool::Tool;
use shinkai_vector_resources::embeddings::Embedding;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSTool {
    pub toolkit_name: String,
    pub name: String,
    pub author: String,
    pub js_code: String,
    pub config: Vec<ToolConfig>,
    pub description: String,
    pub keywords: Vec<String>,
    pub input_args: Vec<ToolArgument>,
    pub activated: bool,
    pub embedding: Option<Embedding>,
    pub result: JSToolResult,
}

impl JSTool {
    pub fn run(&self, parameters: serde_json::Map<String, serde_json::Value>, extra_config: Option<String>) -> Result<RunResult, ToolError> {
        println!("Running JSTool named: {}", self.name);
        println!("Running JSTool with input: {:?}", parameters);
        println!("Running JSTool with extra_config: {:?}", extra_config);

        let code = self.js_code.clone();

        // Create a hashmap with key_name and key_value
        let mut config: HashMap<String, String> = self
            .config
            .iter()
            .filter_map(|c| {
                if let ToolConfig::BasicConfig(basic_config) = c {
                    basic_config
                        .key_value
                        .clone()
                        .map(|value| (basic_config.key_name.clone(), value))
                } else {
                    None
                }
            })
            .collect();

        // Merge extra_config into the config hashmap
        if let Some(extra_config_str) = extra_config {
            let extra_config_map: HashMap<String, String> =
                serde_json::from_str(&extra_config_str).map_err(|e| ToolError::SerializationError(e.to_string()))?;
            config.extend(extra_config_map);
        }

        // Convert the config hashmap to a JSON value
        let config_json = serde_json::to_value(&config).map_err(|e| ToolError::SerializationError(e.to_string()))?;

        // Create a new thread with its own Tokio runtime
        let js_tool_thread = thread::Builder::new().stack_size(8 * 1024 * 1024); // 8 MB
        js_tool_thread
            .spawn(move || {
                let rt = Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(async {
                    println!("Running JSTool with config: {:?}", config);
                    println!("Running JSTool with input: {:?}", parameters);
                    let tool = Tool::new(
                        code,
                        config_json,
                        Some(ShinkaiToolsBackendOptions {
                            binary_path: PathBuf::from(env::var("SHINKAI_TOOLS_BACKEND_BINARY_PATH").unwrap_or_else(
                                |_| "./shinkai-tools-runner-resources/shinkai-tools-backend".to_string(),
                            )),
                            api_port: env::var("SHINKAI_TOOLS_BACKEND_API_PORT")
                                .unwrap_or_else(|_| "9650".to_string())
                                .parse::<u16>()
                                .unwrap_or(9650),
                        }),
                    );
                    // TODO: Fix this object wrap after update tools library to have th right typification
                    tool.run(serde_json::Value::Object(parameters), None)
                        .await
                        .map_err(|e| ToolError::ExecutionError(e.to_string()))
                })
            })
            .unwrap()
            .join()
            .expect("Thread panicked")
    }

    /// Check if all required config fields are set
    pub fn check_required_config_fields(&self) -> bool {
        for config in &self.config {
            if let ToolConfig::BasicConfig(basic_config) = config {
                if basic_config.required && basic_config.key_value.is_none() {
                    return false;
                }
            }
        }
        true
    }

    /// Convert to JSON string
    pub fn to_json_string(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|e| ToolError::SerializationError(e.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JSToolResult {
    pub result_type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}

impl Serialize for JSToolResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let properties_str = serde_json::to_string(&self.properties).map_err(serde::ser::Error::custom)?;

        let helper = Helper {
            result_type: self.result_type.clone(),
            properties: properties_str,
            required: self.required.clone(),
        };

        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for JSToolResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = Helper::deserialize(deserializer)?;
        let properties: JsonValue = serde_json::from_str(&helper.properties).map_err(serde::de::Error::custom)?;

        Ok(JSToolResult {
            result_type: helper.result_type,
            properties,
            required: helper.required,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Helper {
    result_type: String,
    properties: String,
    required: Vec<String>,
}

impl JSToolResult {
    pub fn new(result_type: String, properties: serde_json::Value, required: Vec<String>) -> Self {
        JSToolResult {
            result_type,
            properties,
            required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::js_toolkit_headers::BasicConfig;
    use serde_json::json;

    #[test]
    fn test_check_required_config_fields() {
        // Tool without config
        let tool_without_config = JSTool {
            toolkit_name: "test_toolkit".to_string(),
            name: "test_tool".to_string(),
            author: "author".to_string(),
            js_code: "console.log('Hello, world!');".to_string(),
            config: vec![],
            description: "A test tool".to_string(),
            keywords: vec![],
            input_args: vec![],
            activated: false,
            embedding: None,
            result: JSToolResult::new("object".to_string(), json!({}), vec![]),
        };
        assert!(tool_without_config.check_required_config_fields());

        // Tool with config but without the required params
        let tool_with_missing_config = JSTool {
            config: vec![ToolConfig::BasicConfig(BasicConfig {
                key_name: "apiKey".to_string(),
                description: "API Key".to_string(),
                required: true,
                key_value: None,
            })],
            ..tool_without_config.clone()
        };
        assert!(!tool_with_missing_config.check_required_config_fields());

        // Tool with config and with the required params
        let tool_with_config = JSTool {
            config: vec![ToolConfig::BasicConfig(BasicConfig {
                key_name: "apiKey".to_string(),
                description: "API Key".to_string(),
                required: true,
                key_value: Some("12345".to_string()),
            })],
            ..tool_without_config.clone()
        };
        assert!(tool_with_config.check_required_config_fields());
    }
}
