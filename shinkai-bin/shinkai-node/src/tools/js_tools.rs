use std::thread;

use super::js_toolkit_headers::ToolConfig;
use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;
use shinkai_tools_runner::tools::run_result::RunResult;
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
    pub config_set: bool,
    pub activated: bool,
    pub embedding: Option<Embedding>,
    pub result: JSToolResult,
}

impl JSTool {
    pub fn run(&self, input_json: JsonValue) -> Result<RunResult, ToolError> {
        eprintln!("Running JSTool named: {}", self.name);
        eprintln!("Running JSTool with input: {}", input_json);

        let code = self.js_code.clone();
        let config = serde_json::to_string(&self.config).map_err(|e| ToolError::SerializationError(e.to_string()))?;
        let input = serde_json::to_string(&input_json).map_err(|e| ToolError::SerializationError(e.to_string()))?;

        // Create a new thread with its own Tokio runtime
        let js_tool_thread = thread::Builder::new().stack_size(8 * 1024 * 1024); // 8 MB
        js_tool_thread
            .spawn(move || {
                let rt = Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(async {
                    let mut tool = Tool::new();
                    tool.load_from_code(&code, &config)
                        .await
                        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
                    tool.run(&input)
                        .await
                        .map_err(|e| ToolError::ExecutionError(e.to_string()))
                })
            })
            .join()
            .expect("Thread panicked")
    }

    /// Convert to JSON string
    pub fn to_json_string(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|e| ToolError::SerializationError(e.to_string()))
    }

    /// Convert to JSToolWithoutCode
    pub fn to_without_code(&self) -> JSToolWithoutCode {
        JSToolWithoutCode {
            toolkit_name: self.toolkit_name.clone(),
            name: self.name.clone(),
            author: self.author.clone(),
            config: self.config.clone(),
            description: self.description.clone(),
            keywords: self.keywords.clone(),
            input_args: self.input_args.clone(),
            config_set: self.config_set,
            activated: self.activated,
            embedding: self.embedding.clone(),
            result: self.result.clone(),
        }
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSToolWithoutCode {
    pub toolkit_name: String,
    pub name: String,
    pub author: String,
    pub config: Vec<ToolConfig>,
    pub description: String,
    pub keywords: Vec<String>,
    pub input_args: Vec<ToolArgument>,
    pub config_set: bool,
    pub activated: bool,
    pub embedding: Option<Embedding>,
    pub result: JSToolResult,
}

impl JSToolWithoutCode {
    pub fn from_jstool(tool: &JSTool) -> Self {
        JSToolWithoutCode {
            toolkit_name: tool.toolkit_name.clone(),
            name: tool.name.clone(),
            author: tool.author.clone(),
            config: tool.config.clone(),
            description: tool.description.clone(),
            keywords: tool.keywords.clone(),
            input_args: tool.input_args.clone(),
            config_set: tool.config_set,
            activated: tool.activated,
            embedding: tool.embedding.clone(),
            result: tool.result.clone(),
        }
    }
}
