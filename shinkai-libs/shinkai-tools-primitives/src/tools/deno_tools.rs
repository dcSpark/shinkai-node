use std::collections::HashMap;
use std::hash::RandomState;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, thread};

use super::parameters::Parameters;
use super::tool_config::{OAuth, ToolConfig};
use super::tool_output_arg::ToolOutputArg;
use super::tool_playground::{SqlQuery, SqlTable};
use crate::tools::error::ToolError;
use crate::tools::shared_execution::{get_files_after_with_protocol, update_result_with_modified_files};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value as JsonValue};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_tools_runner::tools::code_files::CodeFiles;
use shinkai_tools_runner::tools::deno_runner::DenoRunner;
use shinkai_tools_runner::tools::deno_runner_options::DenoRunnerOptions;
use shinkai_tools_runner::tools::execution_context::ExecutionContext;
use shinkai_tools_runner::tools::execution_error::ExecutionError;
use shinkai_tools_runner::tools::run_result::RunResult;
use shinkai_tools_runner::tools::shinkai_node_location::ShinkaiNodeLocation;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DenoTool {
    pub name: String,
    pub homepage: Option<String>,
    pub author: String,
    pub version: String,
    pub js_code: String,
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
}

impl DenoTool {
    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
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
                self.assets
                    .clone()
                    .unwrap_or(vec![])
                    .iter()
                    .map(|asset| path.clone().join(asset))
                    .collect()
            }
            None => vec![],
        };

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
                update_result_with_modified_files(
                    result, start_time, &home_path, &logs_path, &node_name, &app_id,
                )
            }
            Err(e) => {
                let files =
                    get_files_after_with_protocol(start_time, &home_path, &logs_path, &node_name, &app_id)
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

        let mount_files = mounts
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|mount| PathBuf::from(mount))
            .collect();

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
                update_result_with_modified_files(
                    result, start_time, &home_path, &logs_path, &node_name, &app_id,
                )
            }
            Err(e) => {
                let files =
                    get_files_after_with_protocol(start_time, &home_path, &logs_path, &node_name, &app_id)
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub r#type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}

impl Serialize for ToolResult {
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

impl<'de> Deserialize<'de> for ToolResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = Helper::deserialize(deserializer)?;

        Ok(ToolResult {
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

impl ToolResult {
    pub fn new(result_type: String, properties: serde_json::Value, required: Vec<String>) -> Self {
        ToolResult {
            r#type: result_type,
            properties,
            required,
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
            "config": [
                {
                    "BasicConfig": {
                        "description": "",
                        "key_name": "name",
                        "key_value": null,
                        "required": true
                    }
                },
                {
                    "BasicConfig": {
                        "description": "",
                        "key_name": "privateKey",
                        "key_value": null,
                        "required": true
                    }
                },
                {
                    "BasicConfig": {
                        "description": "",
                        "key_name": "useServerSigner",
                        "key_value": null,
                        "required": false
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
            "toolkit_name": "deno-toolkit",
            "version": "1.0.0",
            "js_code": "",
            "keywords": [],
            "activated": false,
            "result": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }"#;

        let deserialized: DenoTool = serde_json::from_str(json_data).expect("Failed to deserialize DenoTool");

        assert_eq!(deserialized.author, "Shinkai");
        assert_eq!(deserialized.name, "Coinbase Wallet Creator");
        assert_eq!(deserialized.toolkit_name, "deno-toolkit");
        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.description, "Tool for creating a Coinbase wallet");

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
            toolkit_name: "deno-toolkit".to_string(),
            name: "Email Fetcher".to_string(),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Shinkai".to_string(),
            version: "1.0.0".to_string(),
            description: "Fetches emails from an IMAP server".to_string(),
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
        };

        // Test check_required_config_fields with no values set
        assert!(
            !tool.check_required_config_fields(),
            "Should fail when required fields have no values"
        );

        // Create a tool with values set for required fields
        let mut tool_with_values = tool.clone();
        tool_with_values.config = vec![
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "imap_server".to_string(),
                description: "The IMAP server address".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: Some("imap.example.com".to_string()),
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "username".to_string(),
                description: "The username for the IMAP account".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: Some("user@example.com".to_string()),
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "password".to_string(),
                description: "The password for the IMAP account".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: Some("password123".to_string()),
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

        assert_eq!(deserialized.config.len(), 5, "Should have 5 configuration items");

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
}
