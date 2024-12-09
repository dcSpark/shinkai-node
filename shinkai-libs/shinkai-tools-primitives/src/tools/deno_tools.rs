use std::collections::HashMap;
use std::fs::DirEntry;
use std::hash::RandomState;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, io, thread};

use super::argument::ToolOutputArg;
use super::tool_config::{OAuth, ToolConfig};
use super::tool_playground::{SqlQuery, SqlTable};
use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_tools_runner::tools::code_files::CodeFiles;
use shinkai_tools_runner::tools::deno_runner::DenoRunner;
use shinkai_tools_runner::tools::deno_runner_options::DenoRunnerOptions;
use shinkai_tools_runner::tools::execution_context::ExecutionContext;
use shinkai_tools_runner::tools::run_result::RunResult;
use shinkai_tools_runner::tools::shinkai_node_location::ShinkaiNodeLocation;
use shinkai_vector_resources::embeddings::Embedding;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DenoTool {
    pub toolkit_name: String,
    pub name: String,
    pub author: String,
    pub js_code: String,
    pub tools: Option<Vec<String>>,
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
    pub file_inbox: Option<String>,
    pub oauth: Option<Vec<OAuth>>,
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
        envs: HashMap<String, String, RandomState>,
        api_ip: String,
        api_port: u16,
        support_files: HashMap<String, String>,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Vec<ToolConfig>,
        oauth: Vec<OAuth>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        node_name: ShinkaiName,
        is_temporary: bool,
    ) -> Result<RunResult, ToolError> {
        self.run_on_demand(
            envs,
            api_ip,
            api_port,
            support_files,
            parameters,
            extra_config,
            oauth,
            node_storage_path,
            app_id,
            tool_id,
            node_name,
            is_temporary,
        )
    }

    pub fn run_on_demand(
        &self,
        envs: HashMap<String, String>,
        api_ip: String,
        api_port: u16,
        support_files: HashMap<String, String>,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Vec<ToolConfig>,
        oauth: Vec<OAuth>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        node_name: ShinkaiName,
        is_temporary: bool,
    ) -> Result<RunResult, ToolError> {
        println!(
            "[Running DenoTool] Named: {}, Input: {:?}, Extra Config: {:?}",
            self.name, parameters, self.config
        );

        let code = self.js_code.clone();

        // Create a hashmap with key_name and key_value
        let mut config: HashMap<String, String> = self
            .config
            .iter()
            .filter_map(|c| {
                let ToolConfig::BasicConfig(basic_config) = c;
                basic_config
                    .key_value
                    .clone()
                    .map(|value| (basic_config.key_name.clone(), value))
            })
            .collect();

        // Merge extra_config into the config hashmap
        for c in extra_config {
            let ToolConfig::BasicConfig(basic_config) = c;
            if let Some(value) = basic_config.key_value {
                config.insert(basic_config.key_name.clone(), value);
            }
        }

        // Convert the config hashmap to a JSON value
        let config_json = serde_json::to_value(&config).map_err(|e| ToolError::SerializationError(e.to_string()))?;

        // Create a new thread with its own Tokio runtime
        let js_tool_thread = thread::Builder::new().stack_size(8 * 1024 * 1024); // 8 MB
        js_tool_thread
            .spawn(move || {
                fn get_files_in_directory(directories: Vec<PathBuf>) -> io::Result<Vec<DirEntry>> {
                    let mut files = Vec::new();

                    for directory in directories {
                        let entries = fs::read_dir(directory)?;

                        for entry in entries {
                            let entry = entry?;
                            let path = entry.path();

                            if path.is_file() {
                                files.push(entry);
                            } else if path.is_dir() {
                                // Recursively get files from subdirectories
                                let sub_files = get_files_in_directory(vec![path])?;
                                files.extend(sub_files);
                            }
                        }
                    }

                    Ok(files)
                }

                fn get_files_after(start_time: u64, files: Vec<DirEntry>) -> Vec<(String, u64)> {
                    files
                        .iter()
                        .map(|file| {
                            let name = file.path().to_str().unwrap_or_default().to_string();
                            let modified = file
                                .metadata()
                                .ok()
                                .map(|m| m.modified().ok())
                                .unwrap_or_default()
                                .unwrap_or(SystemTime::now())
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            (name, modified)
                        })
                        .filter(|(_, modified)| *modified > start_time)
                        .collect()
                }

                fn print_result(result: &Result<RunResult, ToolError>) {
                    match result {
                        Ok(result) => println!("[Running DenoTool] Result: {:?}", result.data),
                        Err(e) => println!("[Running DenoTool] Error: {:?}", e),
                    }
                }

                let rt = Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(async {
                    println!(
                        "[Running DenoTool] Config: {:?}. Parameters: {:?}",
                        config_json, parameters
                    );
                    println!(
                        "[Running DenoTool] Code: {} ... {}",
                        &code[..120.min(code.len())],
                        &code[code.len().saturating_sub(400)..]
                    );
                    println!(
                        "[Running DenoTool] Config JSON: {}. Parameters: {:?}",
                        config_json, parameters
                    );

                    // Create the directory structure for the tool
                    let full_path: PathBuf = Path::new(&node_storage_path).join("tools_storage");
                    let home_path = full_path.clone().join(app_id.clone()).join("home");
                    let logs_path = full_path.clone().join(app_id.clone()).join("logs");

                    // Ensure the root directory exists. Subdirectories will be handled by the engine
                    std::fs::create_dir_all(full_path.clone()).map_err(|e| {
                        ToolError::ExecutionError(format!("Failed to create directory structure: {}", e))
                    })?;
                    println!(
                        "[Running DenoTool] Full path: {:?}. App ID: {}. Tool ID: {}",
                        full_path, app_id, tool_id
                    );

                    // If the tool is temporary, create a .temporal file
                    if is_temporary {
                        // TODO: Garbage collector will delete the tool folder after some time
                        let temporal_path = full_path.join(".temporal");
                        std::fs::write(temporal_path, "").map_err(|e| {
                            ToolError::ExecutionError(format!("Failed to create .temporal file: {}", e))
                        })?;
                    }

                    // Get the start time, this is used to check if the files were modified after the tool was executed
                    let start_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    // Create map with file name and source code
                    let mut code_files = HashMap::new();
                    code_files.insert("index.ts".to_string(), code);
                    support_files.iter().for_each(|(file_name, file_code)| {
                        code_files.insert(format!("{}.ts", file_name), file_code.clone());
                    });

                    // Setup the engine with the code files and config
                    let tool = DenoRunner::new(
                        CodeFiles {
                            files: code_files.clone(),
                            entrypoint: "index.ts".to_string(),
                        },
                        config_json,
                        Some(DenoRunnerOptions {
                            context: ExecutionContext {
                                context_id: app_id.clone(),
                                execution_id: tool_id.clone(),
                                code_id: "".to_string(),
                                storage: full_path.clone(),
                                assets_files: vec![],
                                mount_files: vec![],
                            },
                            deno_binary_path: PathBuf::from(
                                env::var("SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH")
                                    .unwrap_or_else(|_| "./shinkai-tools-runner-resources/deno".to_string()),
                            ),
                            shinkai_node_location: ShinkaiNodeLocation {
                                protocol: String::from("http"),
                                host: api_ip,
                                port: api_port,
                            },
                            ..Default::default()
                        }),
                    );

                    // Run the tool with DENO
                    let result = tool
                        .run(Some(envs), serde_json::Value::Object(parameters.clone()), None)
                        .await
                        .map_err(|e| ToolError::ExecutionError(e.to_string()));
                    print_result(&result);

                    pub fn convert_to_shinkai_file_protocol(
                        node_name: &ShinkaiName,
                        path: &str,
                        app_id: &str,
                    ) -> String {
                        // Find the position after app_id in the path
                        if let Some(pos) = path.find(&format!("tools_storage/{}/", app_id)) {
                            // Get the relative path after app_id
                            let relative_path = &path[pos + format!("tools_storage/{}", app_id).len()..];
                            // Construct the shinkai URL preserving the path structure
                            format!("shinkai://{}/{}{}", node_name, app_id, relative_path)
                        } else {
                            return "".to_string();
                        }
                    }

                    // Add modified files to the result data
                    match result {
                        Ok(mut result) => {
                            if let serde_json::Value::Object(ref mut data) = result.data {
                                let modified_files = get_files_after(
                                    start_time,
                                    get_files_in_directory(vec![home_path, logs_path]).unwrap_or_default(),
                                );
                                data.insert(
                                    "__created_files__".to_string(),
                                    serde_json::Value::Array(
                                        modified_files
                                            .into_iter()
                                            .map(|(name, _)| {
                                                serde_json::Value::String(convert_to_shinkai_file_protocol(
                                                    &node_name, &name, &app_id,
                                                ))
                                            })
                                            .collect(),
                                    ),
                                );
                            } else {
                                println!("[Running DenoTool] Result is not an object, skipping modified files");
                                return Err(ToolError::ExecutionError(
                                    "Result is not an object, skipping modified files".to_string(),
                                ));
                            }
                            Ok(result)
                        }
                        Err(e) => Err(e),
                    }
                })
            })
            .unwrap()
            .join()
            .expect("Thread panicked")
    }

    pub fn check(
        &self,
        api_ip: String,
        api_port: u16,
        support_files: HashMap<String, String>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
    ) -> Result<Vec<String>, ToolError> {
        println!("[Checking DenoTool] Named: {}, Input: {:?}", self.name, self.config);

        let code = self.js_code.clone();

        // Create a new thread with its own Tokio runtime
        let js_tool_thread = thread::Builder::new().stack_size(8 * 1024 * 1024); // 8 MB
        js_tool_thread
            .spawn(move || {
                let rt = Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(async {
                    // Create map with file name and source code
                    let mut code_files = HashMap::new();
                    code_files.insert("index.ts".to_string(), code);
                    support_files.iter().for_each(|(file_name, file_code)| {
                        code_files.insert(format!("{}.ts", file_name), file_code.clone());
                    });

                    // Setup the engine with the code files and config
                    let mut tool = DenoRunner::new(
                        CodeFiles {
                            files: code_files.clone(),
                            entrypoint: "index.ts".to_string(),
                        },
                        serde_json::Value::Object(Map::new()),
                        Some(DenoRunnerOptions {
                            context: ExecutionContext {
                                context_id: app_id.clone(),
                                execution_id: tool_id.clone(),
                                code_id: "".to_string(),
                                storage: PathBuf::from(node_storage_path.clone()),
                                assets_files: vec![],
                                mount_files: vec![],
                            },
                            deno_binary_path: PathBuf::from(
                                env::var("SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH")
                                    .unwrap_or_else(|_| "./shinkai-tools-runner-resources/deno".to_string()),
                            ),
                            shinkai_node_location: ShinkaiNodeLocation {
                                protocol: String::from("http"),
                                host: api_ip,
                                port: api_port,
                            },
                            ..Default::default()
                        }),
                    );

                    // Run the check method
                    let result = tool.check().await.map_err(|e| ToolError::ExecutionError(e.to_string()));

                    match result {
                        Ok(warnings) => {
                            println!("[Checking DenoTool] Warnings: {:?}", warnings);
                            Ok(warnings)
                        }
                        Err(e) => {
                            println!("[Checking DenoTool] Error: {:?}", e);
                            Err(e)
                        }
                    }
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
