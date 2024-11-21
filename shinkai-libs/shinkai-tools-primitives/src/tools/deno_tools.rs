use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, thread};

use super::argument::ToolOutputArg;
use super::tool_config::ToolConfig;
use super::tool_playground::{SqlQuery, SqlTable};
use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;
use shinkai_tools_runner::tools::deno_runner_options::DenoRunnerOptions;
use shinkai_tools_runner::tools::execution_context::ExecutionContext;
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
    pub result: DenoToolResult,
    pub sql_tables: Option<Vec<SqlTable>>,
    pub sql_queries: Option<Vec<SqlQuery>>,
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
        envs: HashMap<String, String>,
        header_code: String,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Option<String>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        is_temporary: bool,
    ) -> Result<RunResult, ToolError> {
        self.run_on_demand(
            envs,
            header_code,
            parameters,
            extra_config,
            node_storage_path,
            app_id,
            tool_id,
            is_temporary,
        )
    }

    pub fn run_on_demand(
        &self,
        envs: HashMap<String, String>,
        header_code: String,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Option<String>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        is_temporary: bool,
    ) -> Result<RunResult, ToolError> {
        println!(
            "[Running DenoTool] Named: {}, Input: {:?}, Extra Config: {:?}",
            self.name, parameters, extra_config
        );

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
                    println!("[Running DenoTool] Config: {:?}. Parameters: {:?}", config, parameters);
                    // Remove axios import, as it's also created in the header code
                    let step_1 = if !header_code.is_empty() {
                        let regex_axios = regex::Regex::new(r#"import\s+axios\s+.*"#)?;
                        regex_axios.replace_all(&code, "").into_owned()
                    } else {
                        code
                    };
                    // Remove library import, it is expected to be provided, but might not be generated.
                    let regex = regex::Regex::new(r#"import\s+\{.+?from\s+["']@shinkai/local-tools['"]\s*;"#)?;
                    let step_2 = regex.replace_all(&step_1, "").into_owned();
                    // Add the library import and the header code in the beginning of the code
                    let final_code = format!("{} {}", header_code, step_2);
                    println!(
                        "[Running DenoTool] Final Code: {} ... {} ",
                        &final_code[..120.min(final_code.len())],
                        &final_code[final_code.len().saturating_sub(400)..]
                    );
                    println!(
                        "[Running DenoTool] Config JSON: {}. Parameters: {:?}",
                        config_json, parameters
                    );

                    let full_path: PathBuf = Path::new(&node_storage_path).join("tools_storage");

                    // Ensure directory exists
                    std::fs::create_dir_all(full_path.clone()).map_err(|e| {
                        ToolError::ExecutionError(format!("Failed to create directory structure: {}", e))
                    })?;

                    if is_temporary {
                        // Create .temporal file for temporary tools
                        // TODO: Garbage collector will delete the tool folder after some time
                        let temporal_path = full_path.join(".temporal");
                        std::fs::write(temporal_path, "").map_err(|e| {
                            ToolError::ExecutionError(format!("Failed to create .temporal file: {}", e))
                        })?;
                    }

                    let tool = Tool::new(
                        final_code,
                        config_json,
                        Some(DenoRunnerOptions {
                            context: ExecutionContext {
                                context_id: app_id.clone(),
                                execution_id: tool_id.clone(),
                                code_id: "".to_string(),
                                storage: full_path.clone(),
                            },
                            deno_binary_path: PathBuf::from(
                                env::var("SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH")
                                    .unwrap_or_else(|_| "./shinkai-tools-runner-resources/deno".to_string()),
                            ),
                            ..Default::default()
                        }),
                    );
                    // This is just a workaround to fix the parameters object.
                    // App is sending Parameters: {"0": Object {"url": String("https://jhftss.github.io/")}}
                    let binding = serde_json::Value::Object(parameters.clone());
                    let fixed_parameters = parameters.get("0").unwrap_or(&binding);
                    tool.run(Some(envs), fixed_parameters.clone(), None)
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
pub struct DenoToolResult {
    pub r#type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}

impl Serialize for DenoToolResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper = Helper {
            result_type: self.r#type.clone(),
            properties: self.properties.clone(),
            required: self.required.clone(),
        };

        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DenoToolResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = Helper::deserialize(deserializer)?;

        Ok(DenoToolResult {
            r#type: helper.result_type,
            properties: helper.properties,
            required: helper.required,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Helper {
    #[serde(rename = "type", alias = "result_type")]
    result_type: String,
    properties: JsonValue,
    required: Vec<String>,
}

impl DenoToolResult {
    pub fn new(result_type: String, properties: serde_json::Value, required: Vec<String>) -> Self {
        DenoToolResult {
            r#type: result_type,
            properties,
            required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_jstool_result_with_hashmap_properties() {
        let json_data = r#"
    {
        "type": "object",
        "properties": {
            "walletId": {"type": "string", "nullable": true},
            "seed": {"type": "string", "nullable": true},
            "address": {"type": "string", "nullable": true}
        },
        "required": []
    }
    "#;

        let deserialized: DenoToolResult = serde_json::from_str(json_data).expect("Failed to deserialize JSToolResult");

        assert_eq!(deserialized.r#type, "object");
        assert!(deserialized.properties.is_object());
        assert_eq!(deserialized.required, Vec::<String>::new());

        if let Some(wallet_id) = deserialized.properties.get("walletId") {
            assert_eq!(wallet_id.get("type").and_then(|v| v.as_str()), Some("string"));
            assert_eq!(wallet_id.get("nullable").and_then(|v| v.as_bool()), Some(true));
        } else {
            panic!("walletId property missing");
        }

        if let Some(seed) = deserialized.properties.get("seed") {
            assert_eq!(seed.get("type").and_then(|v| v.as_str()), Some("string"));
            assert_eq!(seed.get("nullable").and_then(|v| v.as_bool()), Some(true));
        } else {
            panic!("seed property missing");
        }

        if let Some(address) = deserialized.properties.get("address") {
            assert_eq!(address.get("type").and_then(|v| v.as_str()), Some("string"));
            assert_eq!(address.get("nullable").and_then(|v| v.as_bool()), Some(true));
        } else {
            panic!("address property missing");
        }
    }
}
