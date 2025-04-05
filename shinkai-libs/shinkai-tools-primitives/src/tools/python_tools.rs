use super::parameters::Parameters;
use super::shared_execution::update_result_with_modified_files;
use super::tool_config::{OAuth, ToolConfig};
use super::tool_output_arg::ToolOutputArg;
use super::tool_playground::{SqlQuery, SqlTable};
use super::tool_types::{OperatingSystem, RunnerType, ToolResult};
use crate::tools::error::ToolError;
use crate::tools::shared_execution::get_files_after_with_protocol;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_tools_runner::tools::code_files::CodeFiles;
use shinkai_tools_runner::tools::execution_context::ExecutionContext;
use shinkai_tools_runner::tools::execution_error::ExecutionError;
use shinkai_tools_runner::tools::python_runner::PythonRunner;
use shinkai_tools_runner::tools::python_runner_options::PythonRunnerOptions;
use shinkai_tools_runner::tools::run_result::RunResult;
use shinkai_tools_runner::tools::shinkai_node_location::ShinkaiNodeLocation;
use std::collections::HashMap;
use std::env;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PythonTool {
    pub version: String,
    pub name: String,
    pub homepage: Option<String>,
    pub author: String,
    pub mcp_enabled: Option<bool>,
    pub py_code: String,
    #[serde(default)]
    #[serde(deserialize_with = "ToolRouterKey::deserialize_tool_router_keys")]
    #[serde(serialize_with = "ToolRouterKey::serialize_tool_router_keys")]
    pub tools: Vec<ToolRouterKey>,
    pub config: Vec<ToolConfig>,
    pub description: String,
    pub keywords: Vec<String>,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub activated: bool,
    pub embedding: Option<Vec<f32>>,
    pub result: ToolResult,
    pub sql_tables: Option<Vec<SqlTable>>,
    pub sql_queries: Option<Vec<SqlQuery>>,
    pub file_inbox: Option<String>,
    pub oauth: Option<Vec<OAuth>>,
    pub assets: Option<Vec<String>>,
    pub runner: RunnerType,
    pub operating_system: Vec<OperatingSystem>,
    pub tool_set: Option<String>,
}

impl PythonTool {
    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }

    pub async fn check_code(
        &self,
        code: String,
        support_files: HashMap<String, String>,
    ) -> Result<Vec<String>, ToolError> {
        // Create map with file name and source code
        let mut code_files = HashMap::new();
        code_files.insert("index.py".to_string(), code);
        support_files.iter().for_each(|(file_name, file_code)| {
            code_files.insert(format!("{}.py", file_name), file_code.clone());
        });

        let empty_hash_map: HashMap<String, String> = HashMap::new();
        let config_json =
            serde_json::to_value(empty_hash_map).map_err(|e| ToolError::SerializationError(e.to_string()))?;

        let tool = PythonRunner::new(
            CodeFiles {
                files: code_files.clone(),
                entrypoint: "index.py".to_string(),
            },
            config_json,
            Some(PythonRunnerOptions {
                uv_binary_path: PathBuf::from(
                    env::var("SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH")
                        .unwrap_or_else(|_| "./shinkai-tools-runner-resources/uv".to_string()),
                ),
                ..Default::default()
            }),
        );

        let result = tool.check().await;
        println!("[Checking PythonTool] Result: {:?}", result);
        result.map_err(|e| ToolError::ExecutionError(e.to_string()))
    }

    pub async fn run(
        &self,
        envs: HashMap<String, String>,
        api_ip: String,
        api_port: u16,
        support_files: HashMap<String, String>,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Vec<ToolConfig>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        node_name: ShinkaiName,
        is_temporary: bool,
        files_tool_router_key: Option<String>,
        mounts: Option<Vec<String>>,
    ) -> Result<RunResult, ToolError> {
        // Construct the list of asset files that should be made available to the Python tool.
        // These files are typically static resources or dependencies that the tool needs to function,
        // such as model files, configuration files, or other data files. The paths are resolved
        // relative to the tool's storage directory in the node's filesystem.
        let assets_files = match files_tool_router_key {
            Some(tool_router_key) => {
                let tool_key = ToolRouterKey::from_string(&tool_router_key)?;
                let path = PathBuf::from(&node_storage_path)
                    .join(".tools_storage")
                    .join("tools")
                    .join(tool_key.convert_to_path());

                let assets_files_: Vec<PathBuf> = self
                    .assets
                    .clone()
                    .unwrap_or(vec![])
                    .iter()
                    .map(|asset| path.clone().join(asset))
                    .collect();
                println!("[Running PythonTool] Assets files: {:?}", assets_files_);
                let full_path: PathBuf = Path::new(&node_storage_path).join("tools_storage");
                let home_path = full_path.clone().join(app_id.clone()).join("home");

                let mut assets_files = Vec::new();
                if path.exists() {
                    let _ = create_dir_all(&home_path);
                    for entry in std::fs::read_dir(&path)
                        .map_err(|e| ToolError::ExecutionError(format!("Failed to read assets directory: {}", e)))?
                    {
                        let entry = entry
                            .map_err(|e| ToolError::ExecutionError(format!("Failed to read directory entry: {}", e)))?;
                        let file_path = entry.path();
                        if file_path.is_file() {
                            assets_files.push(file_path.clone());
                            // In case of docker the files should be located in the home directory
                            let _ = std::fs::copy(&file_path, &home_path.join(file_path.file_name().unwrap()));
                        }
                    }
                }
                assets_files
            }
            None => vec![],
        };

        self.run_on_demand(
            envs,
            api_ip,
            api_port,
            support_files,
            parameters,
            extra_config,
            node_storage_path,
            app_id,
            tool_id,
            node_name,
            is_temporary,
            assets_files,
            mounts,
            false,
        )
        .await
    }

    pub async fn run_on_demand(
        &self,
        envs: HashMap<String, String>,
        api_ip: String,
        api_port: u16,
        support_files: HashMap<String, String>,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Vec<ToolConfig>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        node_name: ShinkaiName,
        is_temporary: bool,
        assets_files: Vec<PathBuf>,
        mounts: Option<Vec<String>>,
        is_playground: bool,
    ) -> Result<RunResult, ToolError> {
        println!(
            "[Running PythonTool] Named: {}, Input: {:?}, Extra Config: {:?}",
            self.name, parameters, self.config
        );

        let code = self.py_code.clone();

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

        fn print_result(result: &Result<RunResult, ExecutionError>) {
            match result {
                Ok(result) => println!("[Running PythonTool] Result: {:?}", result.data),
                Err(e) => println!("[Running PythonTool] Error: {:?}", e),
            }
        }

        println!(
            "[Running PythonTool] Config: {:?}. Parameters: {:?}",
            config_json, parameters
        );
        println!(
            "[Running PythonTool] Code: {} ... {}",
            code.chars().take(120).collect::<String>(),
            code.chars()
                .rev()
                .take(400)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        );
        println!(
            "[Running PythonTool] Config JSON: {}. Parameters: {:?}",
            config_json, parameters
        );

        // Create the directory structure for the tool
        let full_path: PathBuf = Path::new(&node_storage_path).join("tools_storage");
        let home_path = full_path.clone().join(app_id.clone()).join("home");
        let logs_path = full_path.clone().join(app_id.clone()).join("logs");

        // Ensure the root directory exists. Subdirectories will be handled by the engine
        std::fs::create_dir_all(full_path.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create directory structure: {}", e)))?;
        println!(
            "[Running PythonTool] Full path: {:?}. App ID: {}. Tool ID: {}",
            full_path, app_id, tool_id
        );

        // If the tool is temporary, create a .temporal file
        if is_temporary {
            // TODO: Garbage collector will delete the tool folder after some time
            let temporal_path = full_path.join(".temporal");
            std::fs::write(temporal_path, "")
                .map_err(|e| ToolError::ExecutionError(format!("Failed to create .temporal file: {}", e)))?;
        }

        // Get the start time, this is used to check if the files were modified after the tool was executed
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create map with file name and source code
        let mut code_files = HashMap::new();
        code_files.insert("index.py".to_string(), code);
        support_files.iter().for_each(|(file_name, file_code)| {
            code_files.insert(format!("{}.py", file_name), file_code.clone());
        });

        let original_path = assets_files;
        let mut assets_files = vec![];
        if is_playground {
            for asset in original_path {
                // Copy each asset file to the home directory
                let file_name = asset
                    .file_name()
                    .ok_or_else(|| ToolError::ExecutionError("Invalid asset filename".to_string()))?
                    .to_string_lossy()
                    .into_owned();

                let dest_path = home_path.join(&file_name);
                let _ = create_dir_all(&home_path);
                std::fs::copy(&asset, &dest_path)
                    .map_err(|e| ToolError::ExecutionError(format!("Failed to copy asset {}: {}", file_name, e)))?;
                assets_files.push(dest_path);
            }
        } else {
            assets_files = original_path;
        }

        // Setup the engine with the code files and config
        let tool = PythonRunner::new(
            CodeFiles {
                files: code_files.clone(),
                entrypoint: "index.py".to_string(),
            },
            config_json,
            Some(PythonRunnerOptions {
                context: ExecutionContext {
                    context_id: app_id.clone(),
                    execution_id: tool_id.clone(),
                    code_id: "".to_string(),
                    storage: full_path.clone(),
                    assets_files,
                    mount_files: mounts
                        .clone()
                        .unwrap_or_default()
                        .iter()
                        .map(|mount| PathBuf::from(mount))
                        .collect(),
                },
                uv_binary_path: PathBuf::from(
                    env::var("SHINKAI_TOOLS_RUNNER_UV_BINARY_PATH")
                        .unwrap_or_else(|_| "./shinkai-tools-runner-resources/uv".to_string()),
                ),
                shinkai_node_location: ShinkaiNodeLocation {
                    protocol: String::from("http"),
                    host: api_ip,
                    port: api_port,
                },
                ..Default::default()
            }),
        );

        // Run the tool with Python
        let result = tool
            .run(Some(envs), serde_json::Value::Object(parameters.clone()), None)
            .await;
        print_result(&result);

        match result {
            Ok(result) => {
                update_result_with_modified_files(result, start_time, &home_path, &logs_path, &node_name, &app_id)
            }
            Err(e) => {
                let files = get_files_after_with_protocol(start_time, &home_path, &logs_path, &node_name, &app_id)
                    .into_iter()
                    .map(|file| file.as_str().unwrap_or_default().to_string())
                    .collect::<Vec<String>>()
                    .join(" ");

                Err(ToolError::ExecutionError(format!(
                    "Error: {}. Files: {}",
                    e.message().to_string(),
                    files
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_tool_with_runner_type() {
        let tool = PythonTool {
            version: "1.0".to_string(),
            name: "test_tool".to_string(),
            homepage: None,
            author: "test_author".to_string(),
            mcp_enabled: Some(false),
            py_code: "print('hello')".to_string(),
            tools: vec![],
            config: vec![],
            description: "test description".to_string(),
            keywords: vec!["test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        assert_eq!(tool.runner, RunnerType::OnlyHost);
    }

    #[test]
    fn test_python_tool_with_operating_systems() {
        let tool = PythonTool {
            version: "1.0".to_string(),
            name: "test_tool".to_string(),
            homepage: None,
            author: "test_author".to_string(),
            mcp_enabled: Some(false),
            py_code: "print('hello')".to_string(),
            tools: vec![],
            config: vec![],
            description: "test description".to_string(),
            keywords: vec!["test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::Any,
            operating_system: vec![OperatingSystem::Linux, OperatingSystem::Windows],
            tool_set: None,
        };

        assert_eq!(tool.operating_system.len(), 2);
        assert!(tool.operating_system.contains(&OperatingSystem::Linux));
        assert!(tool.operating_system.contains(&OperatingSystem::Windows));
    }

    #[test]
    fn test_python_tool_with_tool_set() {
        let tool = PythonTool {
            version: "1.0".to_string(),
            name: "test_tool".to_string(),
            homepage: None,
            author: "test_author".to_string(),
            mcp_enabled: Some(false),
            py_code: "print('hello')".to_string(),
            tools: vec![],
            config: vec![],
            description: "test description".to_string(),
            keywords: vec!["test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Linux],
            tool_set: Some("test_set".to_string()),
        };

        assert_eq!(tool.tool_set, Some("test_set".to_string()));
    }

    #[test]
    fn test_python_tool_serialization() {
        let tool = PythonTool {
            version: "1.0".to_string(),
            name: "test_tool".to_string(),
            homepage: None,
            author: "test_author".to_string(),
            mcp_enabled: Some(false),
            py_code: "print('hello')".to_string(),
            tools: vec![],
            config: vec![],
            description: "test description".to_string(),
            keywords: vec!["test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Linux],
            tool_set: Some("test_set".to_string()),
        };

        let json = tool.to_json().unwrap();
        let deserialized = PythonTool::from_json(&json).unwrap();

        assert_eq!(tool.runner, deserialized.runner);
        assert_eq!(tool.operating_system, deserialized.operating_system);
        assert_eq!(tool.tool_set, deserialized.tool_set);
    }
}
