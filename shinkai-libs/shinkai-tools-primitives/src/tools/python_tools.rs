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
use super::tool_playground::ToolPlaygroundMetadata;
use std::collections::HashMap;
use std::env;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub struct PythonTool {
    pub version: String,
    pub name: String,
    pub tool_router_key: Option<ToolRouterKey>,
    pub homepage: Option<String>,
    pub author: String,
    pub mcp_enabled: Option<bool>,
    pub py_code: String,
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
        let mut config: HashMap<String, serde_json::Value> = self
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
                config.insert(basic_config.key_name.clone(), value.clone());
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
                    .collect::<Vec<String>>();

                // Python execution logs might include the version. virtual environment and warning.
                // These are not part of the runtime execution, so we clean them up.
                let invalid_lines = [
                    regex::Regex::new(r"^INFO add_decision: Id::<PubGrubPackage>\(\d+\) @ [\da-z]+\.[\da-z]+\.?[\da-z]+? without checking dependencies\s+$").unwrap(),
                    regex::Regex::new(r"^Installed \d+ packages in \d+ms$").unwrap(),
                    regex::Regex::new(r"^Using CPython \d+\.\d+\.\d+$").unwrap(),
                    regex::Regex::new(r"^Creating virtual environment at: [\S]*\.venv$").unwrap(),
                    regex::Regex::new(r"^warning: Failed to hardlink files; falling back to full copy. This may lead to degraded performance\.").unwrap(),
                    regex::Regex::new(r"^If the cache and target directories are on different filesystems, hardlinking may not be supported\.$").unwrap(),
                    regex::Regex::new(r"^If this is intentional, set `export UV_LINK_MODE=copy` or use `--link-mode=copy` to suppress this warning\.$").unwrap(),
                ];

                let error_message = e
                    .message()
                    .to_string()
                    .split("\n")
                    .map(|line| line.to_string())
                    .filter(|line| !line.is_empty())
                    .filter(|line| !invalid_lines.iter().any(|re| re.is_match(line)))
                    .collect::<Vec<String>>()
                    .join("\n");

                Ok(RunResult {
                    data: serde_json::json!({
                        "status": "error",
                        "message": format!("Tool {} execution failed.", self.name),
                        "error": error_message,
                        "__created_files__": files,
                    }),
                })
            }
        }
    }

    pub fn get_metadata(&self) -> ToolPlaygroundMetadata {
        ToolPlaygroundMetadata {
            name: self.name.clone(),
            description: self.description.clone(),
            keywords: self.keywords.clone(),
            homepage: self.homepage.clone(),
            author: self.author.clone(),
            version: self.version.clone(),
            configurations: self.config.clone(),
            parameters: self.input_args.clone(),
            result: self.result.clone(),
            sql_tables: self.sql_tables.clone().unwrap_or_default(),
            sql_queries: self.sql_queries.clone().unwrap_or_default(),
            tools: Some(self.tools.clone()),
            oauth: self.oauth.clone(),
            runner: self.runner.clone(),
            operating_system: self.operating_system.clone(),
            tool_set: self.tool_set.clone(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for PythonTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            name: String,
            #[serde(default)]
            tool_router_key: Option<String>,
            homepage: Option<String>,
            author: String,
            version: String,
            mcp_enabled: Option<bool>,
            py_code: String,
            #[serde(default)]
            tools: Vec<ToolRouterKey>,
            config: Vec<ToolConfig>,
            description: String,
            keywords: Vec<String>,
            input_args: Parameters,
            output_arg: ToolOutputArg,
            activated: bool,
            embedding: Option<Vec<f32>>,
            result: ToolResult,
            sql_tables: Option<Vec<SqlTable>>,
            sql_queries: Option<Vec<SqlQuery>>,
            file_inbox: Option<String>,
            oauth: Option<Vec<OAuth>>,
            assets: Option<Vec<String>>,
            runner: RunnerType,
            operating_system: Vec<OperatingSystem>,
            tool_set: Option<String>,
        }

        let helper = Helper::deserialize(deserializer)?;

        let tool_router_key = match helper.tool_router_key {
            Some(key_str) => Some(ToolRouterKey::from_string(&key_str).map_err(serde::de::Error::custom)?),
            None => Some(ToolRouterKey::new(
                "local".to_string(),
                helper.author.clone(),
                helper.name.clone(),
                None,
            )),
        };

        Ok(PythonTool {
            name: helper.name,
            tool_router_key,
            homepage: helper.homepage,
            author: helper.author,
            version: helper.version,
            mcp_enabled: helper.mcp_enabled,
            py_code: helper.py_code,
            tools: helper.tools,
            config: helper.config,
            description: helper.description,
            keywords: helper.keywords,
            input_args: helper.input_args,
            output_arg: helper.output_arg,
            activated: helper.activated,
            embedding: helper.embedding,
            result: helper.result,
            sql_tables: helper.sql_tables,
            sql_queries: helper.sql_queries,
            file_inbox: helper.file_inbox,
            oauth: helper.oauth,
            assets: helper.assets,
            runner: helper.runner,
            operating_system: helper.operating_system,
            tool_set: helper.tool_set,
        })
    }
}

impl serde::Serialize for PythonTool {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("PythonTool", 24)?;
        state.serialize_field("name", &self.name)?;
        if let Some(key) = &self.tool_router_key {
            state.serialize_field("tool_router_key", &key.to_string_with_version())?;
        } else {
            state.serialize_field("tool_router_key", &None::<String>)?;
        }
        state.serialize_field("homepage", &self.homepage)?;
        state.serialize_field("author", &self.author)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("mcp_enabled", &self.mcp_enabled)?;
        state.serialize_field("py_code", &self.py_code)?;
        let tools_strings: Vec<String> = self.tools.iter().map(|k| k.to_string_with_version()).collect();
        state.serialize_field("tools", &tools_strings)?;
        state.serialize_field("config", &self.config)?;
        state.serialize_field("description", &self.description)?;
        state.serialize_field("keywords", &self.keywords)?;
        state.serialize_field("input_args", &self.input_args)?;
        state.serialize_field("output_arg", &self.output_arg)?;
        state.serialize_field("activated", &self.activated)?;
        state.serialize_field("embedding", &self.embedding)?;
        state.serialize_field("result", &self.result)?;
        state.serialize_field("sql_tables", &self.sql_tables)?;
        state.serialize_field("sql_queries", &self.sql_queries)?;
        state.serialize_field("file_inbox", &self.file_inbox)?;
        state.serialize_field("oauth", &self.oauth)?;
        state.serialize_field("assets", &self.assets)?;
        state.serialize_field("runner", &self.runner)?;
        state.serialize_field("operating_system", &self.operating_system)?;
        state.serialize_field("tool_set", &self.tool_set)?;
        state.end()
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
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "test_author".to_string(),
                "test_tool".to_string(),
                None,
            )),
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
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "test_author".to_string(),
                "test_tool".to_string(),
                None,
            )),
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
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "test_author".to_string(),
                "test_tool".to_string(),
                None,
            )),
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
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "test_author".to_string(),
                "test_tool".to_string(),
                None,
            )),
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
        assert_eq!(tool.tool_router_key, deserialized.tool_router_key);
    }
}
