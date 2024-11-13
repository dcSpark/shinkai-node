use std::collections::HashMap;
use std::path::PathBuf;
use std::{env, thread};

use super::argument::ToolOutputArg;
use super::js_toolkit_headers::ToolConfig;
use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;
use shinkai_tools_runner::tools::deno_runner_options::DenoRunnerOptions;
use shinkai_tools_runner::tools::run_result::RunResult;
use shinkai_tools_runner::tools::tool::Tool;
use shinkai_vector_resources::embeddings::Embedding;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DenoTool {
    pub toolkit_name: String,
    pub name: String,
    pub author: String,
    pub js_code: String,
    pub config: Vec<ToolConfig>,
    pub description: String,
    pub keywords: Vec<String>,
    pub input_args: Vec<ToolArgument>,
    pub output_arg: ToolOutputArg,
    pub activated: bool,
    pub embedding: Option<Embedding>,
    pub result: JSToolResult,
    pub output: String,
}

impl DenoTool {
    /// Default name of the rust toolkit
    pub fn toolkit_name(&self) -> String {
        "deno-toolkit".to_string()
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }

    pub fn run(
        &self,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Option<String>,
    ) -> Result<RunResult, ToolError> {
        self.run_on_demand(String::new(), String::new(), parameters, extra_config)
    }

    pub fn run_on_demand(
        &self,
        bearer: String,
        header_code: String,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Option<String>,
    ) -> Result<RunResult, ToolError> {
        println!("Running DenoTool named: {}", self.name);
        println!("Running DenoTool with input: {:?}", parameters);
        println!("Running DenoTool with extra_config: {:?}", extra_config);

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
                    println!("Running DenoTool with config: {:?}", config);
                    println!("Running DenoTool with input: {:?}", parameters);
                    let final_code = if !bearer.is_empty() {
                        let regex = regex::Regex::new(r#"import\s+\{.+?from\s+["']@shinkai/local-tools['"]\s*;"#)?;
                        let code_with_header = format!("{} {}", header_code, regex.replace_all(&code, "").into_owned());
                        code_with_header.replace("process.env.BEARER", &format!("\"{}\"", &bearer))
                    } else {
                        code
                    };
                    // println!("Final code: {}", final_code);
                    let tool = Tool::new(
                        final_code,
                        config_json,
                        Some(DenoRunnerOptions {
                            binary_path: PathBuf::from(
                                env::var("SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH")
                                    .unwrap_or_else(|_| "./shinkai-tools-runner-resources/deno".to_string()),
                            ),
                        }),
                    );
                    // TODO: Fix this object wrap after update tools library to have the right typification
                    tool.run(None, serde_json::Value::Object(parameters), None)
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
