use super::parameters::Parameters;
use super::tool_config::{OAuth, ToolConfig};
use super::tool_output_arg::ToolOutputArg;
use super::tool_playground::ToolPlaygroundMetadata;
use super::tool_playground::{SqlQuery, SqlTable};
use super::tool_types::{OperatingSystem, RunnerType, ToolResult};
use crate::tools::error::ToolError;
use crate::tools::shared_execution::{get_files_after_with_protocol, update_result_with_modified_files};
use serde_json::Map;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_tools_runner::tools::code_files::CodeFiles;
use shinkai_tools_runner::tools::deno_runner::DenoRunner;
use shinkai_tools_runner::tools::deno_runner_options::DenoRunnerOptions;
use shinkai_tools_runner::tools::execution_context::ExecutionContext;
use shinkai_tools_runner::tools::execution_error::ExecutionError;
use shinkai_tools_runner::tools::run_result::RunResult;
use shinkai_tools_runner::tools::shinkai_node_location::ShinkaiNodeLocation;
use std::collections::HashMap;
use std::env;
use std::fs::create_dir_all;
use std::hash::RandomState;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub struct DenoTool {
    pub name: String,
    pub tool_router_key: Option<ToolRouterKey>,
    pub homepage: Option<String>,
    pub author: String,
    pub version: String,
    pub mcp_enabled: Option<bool>,
    pub js_code: String,
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

impl<'de> serde::Deserialize<'de> for DenoTool {
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
            js_code: String,
            #[serde(default)]
            #[serde(deserialize_with = "ToolRouterKey::deserialize_tool_router_keys")]
            #[serde(serialize_with = "ToolRouterKey::serialize_tool_router_keys")]
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

        Ok(DenoTool {
            name: helper.name,
            tool_router_key,
            homepage: helper.homepage,
            author: helper.author,
            version: helper.version,
            mcp_enabled: helper.mcp_enabled,
            js_code: helper.js_code,
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

impl serde::Serialize for DenoTool {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("DenoTool", 24)?;
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
        state.serialize_field("js_code", &self.js_code)?;
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

impl DenoTool {
    pub fn new(
        name: String,
        homepage: Option<String>,
        author: String,
        version: String,
        mcp_enabled: Option<bool>,
        js_code: String,
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
    ) -> Self {
        let tool_router_key = ToolRouterKey::new("local".to_string(), author.clone(), name.clone(), None);

        DenoTool {
            name,
            tool_router_key: Some(tool_router_key),
            homepage,
            author,
            version,
            mcp_enabled,
            js_code,
            tools,
            config,
            description,
            keywords,
            input_args,
            output_arg,
            activated,
            embedding,
            result,
            sql_tables,
            sql_queries,
            file_inbox,
            oauth,
            assets,
            runner,
            operating_system,
            tool_set,
        }
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

    pub async fn check_code(
        &self,
        code: String,
        support_files: HashMap<String, String>,
    ) -> Result<Vec<String>, ToolError> {
        // Create map with file name and source code
        let mut code_files = HashMap::new();
        code_files.insert("index.ts".to_string(), code);
        support_files.iter().for_each(|(file_name, file_code)| {
            code_files.insert(format!("{}.ts", file_name), file_code.clone());
        });
        let empty_hash_map: HashMap<String, String> = HashMap::new();
        let config_json =
            serde_json::to_value(empty_hash_map).map_err(|e| ToolError::SerializationError(e.to_string()))?;
        let tool = DenoRunner::new(
            CodeFiles {
                files: code_files.clone(),
                entrypoint: "index.ts".to_string(),
            },
            config_json,
            Some(DenoRunnerOptions {
                deno_binary_path: PathBuf::from(
                    env::var("SHINKAI_TOOLS_RUNNER_DENO_BINARY_PATH")
                        .unwrap_or_else(|_| "./shinkai-tools-runner-resources/deno".to_string()),
                ),
                ..Default::default()
            }),
        );
        let result = tool.check().await;
        println!("[Checking DenoTool] Result: {:?}", result);
        result.map_err(|e| ToolError::ExecutionError(e.to_string()))
    }

    async fn run_internal(
        &self,
        envs: HashMap<String, String>,
        api_ip: String,
        api_port: u16,
        support_files: HashMap<String, String>,
        parameters: Map<String, serde_json::Value>,
        extra_config: Vec<ToolConfig>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        node_name: ShinkaiName,
        is_temporary: bool,
        assets_files: Vec<PathBuf>,
        mount_files: Vec<PathBuf>,
    ) -> Result<RunResult, ToolError> {
        println!(
            "[Running DenoTool] Named: {}, Input: {:?}, Extra Config: {:?}",
            self.name, parameters, self.config
        );

        let code = self.js_code.clone();

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
                Ok(result) => println!("[Running DenoTool] Result: {:?}", result.data),
                Err(e) => println!("[Running DenoTool] Error: {:?}", e),
            }
        }

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
        std::fs::create_dir_all(full_path.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create directory structure: {}", e)))?;
        println!(
            "[Running DenoTool] Full path: {:?}. App ID: {}. Tool ID: {}",
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
                    assets_files,
                    mount_files,
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

                // Errors from the runner include the downloaded libraries
                // on the first run. So we clean them up, as the are not part of the execution it self.
                // Remove download links from the error message
                let re = regex::Regex::new(r"^Download https:[\S]*$").unwrap();
                // Shinkai UI requires double line breaks to display a line break.
                let error_message: String = e
                    .message()
                    .to_string()
                    .split("\n")
                    .map(|line| line.to_string())
                    .filter(|line| !line.is_empty())
                    .filter(|line| match re.captures(line) {
                        Some(_) => false,
                        None => true,
                    })
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

    pub async fn run(
        &self,
        envs: HashMap<String, String, RandomState>,
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
        let mount_files = mounts
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|mount| PathBuf::from(mount))
            .collect();

        // Get assets files from tool router key
        let assets_files = match &files_tool_router_key {
            Some(tool_router_key) => {
                let tool_key = ToolRouterKey::from_string(tool_router_key)?;
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
                println!("[Running DenoTool] Assets files: {:?}", assets_files_);

                let mut assets_files = Vec::new();
                if path.exists() {
                    let home_path = PathBuf::from(&node_storage_path)
                        .join("tools_storage")
                        .join(app_id.clone())
                        .join("home");
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

        self.run_internal(
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
            mount_files,
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
        playground_assets_files: Vec<PathBuf>,
        mounts: Option<Vec<String>>,
    ) -> Result<RunResult, ToolError> {
        let mount_files = mounts
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|mount| PathBuf::from(mount))
            .collect();

        let original_path = playground_assets_files;
        let mut assets_files = vec![];
        for asset in original_path {
            // Copy each asset file to the home directory
            let file_name = asset
                .file_name()
                .ok_or_else(|| ToolError::ExecutionError("Invalid asset filename".to_string()))?
                .to_string_lossy()
                .into_owned();

            let home_path = PathBuf::from(&node_storage_path)
                .join("tools_storage")
                .join(app_id.clone())
                .join("home");
            let dest_path = home_path.join(&file_name);
            let _ = create_dir_all(&home_path);
            std::fs::copy(&asset, &dest_path)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to copy asset {}: {}", file_name, e)))?;
            assets_files.push(dest_path);
        }

        self.run_internal(
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
            mount_files,
        )
        .await
    }

    pub async fn check(
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
    }

    /// Check if all required config fields are set
    pub fn check_required_config_fields(&self) -> bool {
        for config in &self.config {
            let ToolConfig::BasicConfig(basic_config) = config;
            if basic_config.required && basic_config.key_value.is_none() {
                return false;
            }
        }
        true
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

#[cfg(test)]
mod tests {
    use crate::tools::tool_config::BasicConfig;

    use super::*;
    use serde_json::json;

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

        let deserialized: ToolResult = serde_json::from_str(json_data).expect("Failed to deserialize JSToolResult");

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

    #[test]
    fn test_deserialize_deno_tool() {
        let json_data = r#"{
            "author": "Shinkai",
            "homepage": "http://example.com",
            "config": [
                {
                    "BasicConfig": {
                        "description": "",
                        "key_name": "name",
                        "key_value": null,
                        "required": true,
                        "type_name": null
                    }
                },
                {
                    "BasicConfig": {
                        "description": "",
                        "key_name": "privateKey",
                        "key_value": null,
                        "required": true,
                        "type_name": null
                    }
                },
                {
                    "BasicConfig": {
                        "description": "",
                        "key_name": "useServerSigner",
                        "key_value": null,
                        "required": false,
                        "type_name": null
                    }
                }
            ],
            "description": "Tool for creating a Coinbase wallet",
            "input_args": {
                "properties": {},
                "required": [],
                "type": "object"
            },
            "name": "Coinbase Wallet Creator",
            "output_arg": {
                "json": ""
            },
            "version": "1.0.0",
            "js_code": "",
            "keywords": [],
            "activated": false,
            "tools": [],
            "runner": "any",
            "tool_set": null,
            "operating_system": [],
            "result": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }"#;

        let deserialized: DenoTool = serde_json::from_str(json_data).expect("Failed to deserialize DenoTool");

        assert_eq!(deserialized.author, "Shinkai");
        assert_eq!(deserialized.name, "Coinbase Wallet Creator");
        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.description, "Tool for creating a Coinbase wallet");
        assert_eq!(deserialized.homepage, Some("http://example.com".to_string()));
        assert_eq!(deserialized.runner, RunnerType::Any);
        assert_eq!(deserialized.tool_set, None);
        assert_eq!(
            deserialized.tool_router_key,
            Some(ToolRouterKey::new(
                "local".to_string(),
                "Shinkai".to_string(),
                "Coinbase Wallet Creator".to_string(),
                None
            ))
        );

        // Verify config entries
        assert_eq!(deserialized.config.len(), 3);
        let ToolConfig::BasicConfig(config) = &deserialized.config[0];
        assert_eq!(config.key_name, "name");
        assert!(config.required);

        let ToolConfig::BasicConfig(config) = &deserialized.config[1];
        assert_eq!(config.key_name, "privateKey");
        assert!(config.required);

        let ToolConfig::BasicConfig(config) = &deserialized.config[2];
        assert_eq!(config.key_name, "useServerSigner");
        assert!(!config.required);
    }

    #[test]
    fn test_email_fetcher_tool_config() {
        let tool = DenoTool {
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "Shinkai".to_string(),
                "Email Fetcher".to_string(),
                None,
            )),
            name: "Email Fetcher".to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Shinkai".to_string(),
            version: "1.0.0".to_string(),
            description: "Fetches emails from an IMAP server".to_string(),
            mcp_enabled: Some(false),
            keywords: vec!["email".to_string(), "imap".to_string()],
            js_code: "".to_string(),
            tools: vec![],
            config: vec![
                ToolConfig::BasicConfig(BasicConfig {
                    key_name: "imap_server".to_string(),
                    description: "The IMAP server address".to_string(),
                    required: true,
                    type_name: Some("string".to_string()),
                    key_value: None,
                }),
                ToolConfig::BasicConfig(BasicConfig {
                    key_name: "username".to_string(),
                    description: "The username for the IMAP account".to_string(),
                    required: true,
                    type_name: Some("string".to_string()),
                    key_value: None,
                }),
                ToolConfig::BasicConfig(BasicConfig {
                    key_name: "password".to_string(),
                    description: "The password for the IMAP account".to_string(),
                    required: true,
                    type_name: Some("string".to_string()),
                    key_value: None,
                }),
                ToolConfig::BasicConfig(BasicConfig {
                    key_name: "port".to_string(),
                    description: "The port number for the IMAP server (defaults to 993 for IMAPS)".to_string(),
                    required: false,
                    type_name: Some("integer".to_string()),
                    key_value: None,
                }),
                ToolConfig::BasicConfig(BasicConfig {
                    key_name: "ssl".to_string(),
                    description: "Whether to use SSL for the IMAP connection (defaults to true)".to_string(),
                    required: false,
                    type_name: Some("boolean".to_string()),
                    key_value: None,
                }),
            ],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: false,
            embedding: None,
            result: ToolResult::new("object".to_string(), json!({}), vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::Any,
            operating_system: vec![OperatingSystem::Linux],
            tool_set: None,
        };

        let serialized = serde_json::to_string_pretty(&tool).expect("Failed to serialize DenoTool");

        let deserialized: DenoTool = serde_json::from_str(&serialized).expect("Failed to deserialize DenoTool");

        // Test check_required_config_fields with no values set
        assert!(
            !deserialized.check_required_config_fields(),
            "Should fail when required fields have no values"
        );

        // Create a tool with values set for required fields
        let mut tool_with_values = deserialized.clone();
        tool_with_values.config = vec![
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "imap_server".to_string(),
                description: "The IMAP server address".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: Some(serde_json::Value::String("imap.example.com".to_string())),
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "username".to_string(),
                description: "The username for the IMAP account".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: Some(serde_json::Value::String("user@example.com".to_string())),
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "password".to_string(),
                description: "The password for the IMAP account".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: Some(serde_json::Value::String("password123".to_string())),
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "port".to_string(),
                description: "The port number for the IMAP server (defaults to 993 for IMAPS)".to_string(),
                required: false,
                type_name: Some("integer".to_string()),
                key_value: None,
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "ssl".to_string(),
                description: "Whether to use SSL for the IMAP connection (defaults to true)".to_string(),
                required: false,
                type_name: Some("boolean".to_string()),
                key_value: None,
            }),
        ];

        assert!(
            tool_with_values.check_required_config_fields(),
            "Should pass when required fields have values"
        );

        // Test serialization/deserialization
        let serialized = serde_json::to_string(&tool).expect("Failed to serialize DenoTool");
        let deserialized: DenoTool = serde_json::from_str(&serialized).expect("Failed to deserialize DenoTool");

        // Check specific configs
        let imap_server_config = deserialized
            .config
            .iter()
            .find(|c| match c {
                ToolConfig::BasicConfig(bc) => bc.key_name == "imap_server",
                _ => false,
            })
            .unwrap();
        let ToolConfig::BasicConfig(config) = imap_server_config;
        assert_eq!(config.description, "The IMAP server address");
        assert_eq!(config.type_name, Some("string".to_string()));
        assert!(config.required);
        assert_eq!(config.key_value, None);

        let port_config = deserialized
            .config
            .iter()
            .find(|c| match c {
                ToolConfig::BasicConfig(bc) => bc.key_name == "port",
                _ => false,
            })
            .unwrap();
        let ToolConfig::BasicConfig(config) = port_config;
        assert_eq!(
            config.description,
            "The port number for the IMAP server (defaults to 993 for IMAPS)"
        );
        assert_eq!(config.type_name, Some("integer".to_string()));
        assert!(!config.required);
        assert_eq!(config.key_value, None);
    }

    #[test]
    fn test_deno_tool_runner_types() {
        let tool = DenoTool {
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "Test Author".to_string(),
                "Test Tool".to_string(),
                None,
            )),
            name: "Test Tool".to_string(),
            homepage: None,
            author: "Test Author".to_string(),
            version: "1.0.0".to_string(),
            js_code: "".to_string(),
            tools: vec![],
            config: vec![],
            description: "Test description".to_string(),
            keywords: vec![],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: false,
            mcp_enabled: Some(false),
            embedding: None,
            result: ToolResult::new("object".to_string(), json!({}), vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyDocker,
            operating_system: vec![],
            tool_set: None,
        };

        // Test serialization/deserialization with RunnerType
        let serialized = serde_json::to_string(&tool).expect("Failed to serialize DenoTool");
        let deserialized: DenoTool = serde_json::from_str(&serialized).expect("Failed to deserialize DenoTool");

        assert_eq!(deserialized.runner, RunnerType::OnlyDocker);

        // Test different runner types
        let mut tool_any = tool.clone();
        tool_any.runner = RunnerType::Any;
        let serialized = serde_json::to_string(&tool_any).expect("Failed to serialize DenoTool");
        let deserialized: DenoTool = serde_json::from_str(&serialized).expect("Failed to deserialize DenoTool");
        assert_eq!(deserialized.runner, RunnerType::Any);
    }

    #[test]
    fn test_deno_tool_operating_systems() {
        let tool = DenoTool {
            name: "Test Tool".to_string(),
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "Test Author".to_string(),
                "Test Tool".to_string(),
                None,
            )),
            homepage: None,
            author: "Test Author".to_string(),
            version: "1.0.0".to_string(),
            js_code: "".to_string(),
            tools: vec![],
            config: vec![],
            description: "Test description".to_string(),
            keywords: vec![],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: false,
            mcp_enabled: Some(false),
            embedding: None,
            result: ToolResult::new("object".to_string(), json!({}), vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::Any,
            operating_system: vec![OperatingSystem::Linux, OperatingSystem::Windows],
            tool_set: None,
        };

        // Test serialization/deserialization with operating systems
        let serialized = serde_json::to_string(&tool).expect("Failed to serialize DenoTool");
        let deserialized: DenoTool = serde_json::from_str(&serialized).expect("Failed to deserialize DenoTool");

        assert_eq!(deserialized.operating_system.len(), 2);
        assert!(deserialized.operating_system.contains(&OperatingSystem::Linux));
        assert!(deserialized.operating_system.contains(&OperatingSystem::Windows));
    }

    #[test]
    fn test_deno_tool_tool_set() {
        let tool = DenoTool {
            tool_router_key: Some(ToolRouterKey::new(
                "local".to_string(),
                "Test Author".to_string(),
                "Test Tool".to_string(),
                None,
            )),
            name: "Test Tool".to_string(),
            homepage: None,
            author: "Test Author".to_string(),
            version: "1.0.0".to_string(),
            js_code: "".to_string(),
            tools: vec![],
            config: vec![],
            description: "Test description".to_string(),
            keywords: vec![],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg { json: "".to_string() },
            activated: false,
            mcp_enabled: Some(false),
            embedding: None,
            result: ToolResult::new("object".to_string(), json!({}), vec![]),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::Any,
            operating_system: vec![],
            tool_set: Some("test-tool-set".to_string()),
        };

        // Test serialization/deserialization with tool_set
        let serialized = serde_json::to_string(&tool).expect("Failed to serialize DenoTool");
        let deserialized: DenoTool = serde_json::from_str(&serialized).expect("Failed to deserialize DenoTool");

        assert_eq!(deserialized.tool_set, Some("test-tool-set".to_string()));

        // Test with None tool_set
        let mut tool_no_set = tool.clone();
        tool_no_set.tool_set = None;
        let serialized = serde_json::to_string(&tool_no_set).expect("Failed to serialize DenoTool");
        let deserialized: DenoTool = serde_json::from_str(&serialized).expect("Failed to deserialize DenoTool");
        assert_eq!(deserialized.tool_set, None);
    }
}
