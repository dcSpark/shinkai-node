use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::{Parameters, Property};
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg};
use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::network::Node;
use crate::tools::tool_execution::execution_header_generator::generate_execution_environment;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;

use tokio::sync::Mutex;

use async_trait::async_trait;
use std::process::{Command, Stdio};
use std::{env, fs};

// LLM Tool
pub struct TypescriptUnsafeProcessorTool {
    pub tool: ShinkaiToolHeader,
    pub tool_embedding: Option<Vec<f32>>,
}

impl TypescriptUnsafeProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai Typescript Unsafe Processor".to_string(),
                description: "Tool for executing Node.js code. This is unsafe and should be used with extreme caution."
                    .to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_typescript_unsafe_processor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Tool for executing Typescript Unsafe".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0.0".to_string(),
                enabled: true,
                input_args: {
                    let mut params = Parameters::new();
                    params.properties.insert(
                        "code".to_string(),
                        Property::new("string".to_string(), "The TypeScript code to execute".to_string()),
                    );
                    params.required.push("code".to_string());

                    params.properties.insert(
                        "package".to_string(),
                        Property::new("string".to_string(), "The package.json contents".to_string()),
                    );
                    params.required.push("package".to_string());

                    params.properties.insert(
                        "parameters".to_string(),
                        Property::new("object".to_string(), "Parameters to pass to the code".to_string()),
                    );
                    params.required.push("parameters".to_string());

                    params.properties.insert(
                        "config".to_string(),
                        Property::new("object".to_string(), "Configuration for the code execution".to_string()),
                    );
                    params.required.push("code".to_string());
                    params
                },
                output_arg: ToolOutputArg {
                    json: r#"{"type": "object", "properties": {"stdout": {"type": "string"}}}"#.to_string(),
                },
                config: None,
                usage_type: None,
                tool_offering: None,
            },
            tool_embedding: None,
        }
    }

    async fn run_node_code(
        code: &str,
        package: Option<&str>,
        parameters_json_string: &str,
        config_json_string: &str,
        envs: &HashMap<String, String>,
    ) -> Result<String, ToolError> {
        // Get npm and node binary locations from env or use defaults
        let npm_binary = env::var("NPM_BINARY_LOCATION").unwrap_or_else(|_| "npm".to_string());
        let node_binary = env::var("NODE_BINARY_LOCATION").unwrap_or_else(|_| "node".to_string());

        let init_code = format!(
            r#"
        const inputs = {};
        const config = {};
        run(config, inputs).then((result) => {{
            console.log("<TYPE_SCRIPT_UNSAFE_PROCESSOR_OUTPUT>");
            console.log(JSON.stringify(result));
            console.log("</TYPE_SCRIPT_UNSAFE_PROCESSOR_OUTPUT>");
        }}).catch((error) => {{
            console.log("<TYPE_SCRIPT_UNSAFE_PROCESSOR_ERROR>");
            console.log(JSON.stringify({{ error }}));
            console.log("</TYPE_SCRIPT_UNSAFE_PROCESSOR_ERROR>");
        }});
        "#,
            parameters_json_string, config_json_string
        );

        // Create temporary directory with random name
        let root_temp_dir = env::temp_dir();
        let random_id = format!("shinkai_stagehand_{}", uuid::Uuid::new_v4());
        let temp_path = root_temp_dir.join(random_id);

        fs::create_dir_all(temp_path.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp dir: {}", e)))?;
        // Write files}
        let final_code = format!("{}\n{}", code, init_code);
        // println!("Running code: {}", final_code);
        fs::write(temp_path.clone().join("index.ts"), final_code)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write index.ts: {}", e)))?;

        if let Some(package) = package {
            // Run npm install
            fs::write(temp_path.clone().join("package.json"), package)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to write package.json: {}", e)))?;

            let npm_output = Command::new(&npm_binary)
                .arg("install")
                .current_dir(temp_path.clone())
                .stdout(Stdio::inherit())
                // .stderr(Stdio::inherit())
                .output()
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        ToolError::ExecutionError(
                            "Node.js is not installed. Please download from https://nodejs.org/en/download".to_string(),
                        )
                    } else {
                        ToolError::ExecutionError(format!("Failed to run npm install: {}", e))
                    }
                })?;
            let npm_output_string = String::from_utf8_lossy(&npm_output.stdout).to_string();

            if !npm_output.status.success() {
                return Err(ToolError::ExecutionError(format!(
                    "npm install failed: {}",
                    String::from_utf8_lossy(&npm_output.stderr)
                )));
            }
        }

        // Run node index.ts
        let node_output = Command::new(&node_binary)
            .envs(envs)
            .arg("--experimental-strip-types")
            .arg("index.ts")
            .current_dir(temp_path.clone())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ToolError::ExecutionError(
                        "Node.js is not installed. Please download from https://nodejs.org/en/download".to_string(),
                    )
                } else {
                    ToolError::ExecutionError(format!("Failed to run node: {}", e))
                }
            })?;
        let stdout = String::from_utf8_lossy(&node_output.stdout).to_string();

        if !node_output.status.success() {
            return Err(ToolError::ExecutionError(format!(
                "Node execution failed: {}\nOutput: {}",
                String::from_utf8_lossy(&node_output.stderr),
                stdout
            )));
        }

        // Extract content between tags
        if let Some(start_idx) = stdout.find("<TYPE_SCRIPT_UNSAFE_PROCESSOR_OUTPUT>") {
            if let Some(end_idx) = stdout.find("</TYPE_SCRIPT_UNSAFE_PROCESSOR_OUTPUT>") {
                let content = stdout[start_idx + "<TYPE_SCRIPT_UNSAFE_PROCESSOR_OUTPUT>".len()..end_idx].trim();
                return Ok(content.to_string());
            }
        }

        // Check for error tags if output tags weren't found
        if let Some(start_idx) = stdout.find("<TYPE_SCRIPT_UNSAFE_PROCESSOR_ERROR>") {
            if let Some(end_idx) = stdout.find("</TYPE_SCRIPT_UNSAFE_PROCESSOR_ERROR>") {
                let content = stdout[start_idx + "<TYPE_SCRIPT_UNSAFE_PROCESSOR_ERROR>".len()..end_idx].trim();
                return Err(ToolError::ExecutionError(content.to_string()));
            }
        }

        // If no tags found, return the raw output
        Ok(stdout)
    }
}

#[async_trait]
impl ToolExecutor for TypescriptUnsafeProcessorTool {
    async fn execute(
        _bearer: String,
        tool_id: String,
        app_id: String,
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        _identity_manager_clone: Arc<Mutex<IdentityManager>>,
        _job_manager_clone: Arc<Mutex<JobManager>>,
        _encryption_secret_key_clone: EncryptionStaticKey,
        _encryption_public_key_clone: EncryptionPublicKey,
        _signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        llm_provider: String,
    ) -> Result<Value, ToolError> {
        let profile_name = ShinkaiName::from_node_and_profile_names(node_name.node_name, "main".to_string())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to get profile name: {}", e)))?;
        let llm_providers = db
            .get_llm_providers_for_profile(profile_name)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to get LLM providers: {}", e)))?;
        let open_ai_key = match llm_providers
            .iter()
            .find(|provider| provider.external_url == Some("https://api.openai.com".to_string()))
        {
            Some(provider) => provider.api_key.clone().unwrap_or_default(),
            None => "".to_string(),
        };
        println!("OpenAI Provider: {:?}", open_ai_key);

        // Extract required parameters
        let code = parameters
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::ExecutionError("Missing code parameter".to_string()))?;

        let package = parameters.get("package").and_then(|v| v.as_str());
        let parameters_json = parameters.get("parameters");
        let config_json = parameters.get("config");

        let parameters_json_string = if let Some(parameters_json) = parameters_json {
            serde_json::to_string(parameters_json)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to serialize parameters: {}", e)))?
        } else {
            "{}".to_string()
        };

        let config_json_string = if let Some(config_json) = config_json {
            serde_json::to_string(config_json)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to serialize config: {}", e)))?
        } else {
            "{}".to_string()
        };

        let mut envs = generate_execution_environment(
            db,
            llm_provider,
            app_id,
            tool_id,
            "".to_string(), // Tool router key needed for oauth validation.
            "unknown".to_string(),
            &None,
        )
        .await?;

        // TODO get this from node_env
        let protocol = "http".to_string();
        let api_ip = env::var("NODE_API_IP").unwrap_or_else(|_| "0.0.0.0".to_string());
        let api_port = env::var("NODE_API_PORT").unwrap_or_else(|_| "9550".to_string());
        envs.insert(
            "SHINKAI_NODE_LOCATION".to_string(),
            format!("{}://{}:{}", protocol, api_ip, api_port),
        );
        envs.insert("OPENAI_KEY".to_string(), open_ai_key);
        envs.insert(
            "CHROME_PATH".to_string(),
            env::var("CHROME_PATH").unwrap_or_else(|_| "".to_string()),
        );

        // Store in temporal storage the code and the parameters
        let temporal_folder = env::temp_dir().join(format!("shinkai_ts_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(temporal_folder.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp dir: {}", e)))?;
        fs::write(temporal_folder.join("code.ts"), code)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write code.ts: {}", e)))?;
        if let Some(package) = package {
            fs::write(temporal_folder.join("package.json"), package)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to write package.json: {}", e)))?;
        }
        fs::write(temporal_folder.join("parameters.json"), parameters_json_string.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write parameters.json: {}", e)))?;
        fs::write(temporal_folder.join("config.json"), config_json_string.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write config.json: {}", e)))?;
        fs::write(
            temporal_folder.join(".env"),
            envs.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join("\n"),
        )
        .map_err(|e| ToolError::ExecutionError(format!("Failed to write env: {}", e)))?;
        println!("[TS Unsafe Processor] Temporal folder: {:?}", temporal_folder);

        // Run the Node.js code and get the output
        let stdout = Self::run_node_code(&code, package, &parameters_json_string, &config_json_string, &envs).await?;
        let result = serde_json::from_str(&stdout).unwrap_or_default();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_node_code() {
        // Test a simple hello world program
        let code = r#"
        async function run(config: any, inputs: any) {
            return { "message": "Hello, World!" };
        }
        "#;

        let package = r#"{
            "name": "test-hello-world",
            "version": "1.0.0",
            "dependencies": {}
        }"#;

        let result =
            TypescriptUnsafeProcessorTool::run_node_code(code, Some(package), "{}", "{}", &HashMap::new()).await;
        // println!("Result: {:?}", result);
        assert!(result.is_ok());
        assert!(result.unwrap().contains(r#"{"message":"Hello, World!"}"#));
    }

    #[tokio::test]
    async fn test_axios_request() {
        let code = r#"
        import axios from 'axios';

        async function run(config: any, inputs: any) {
            const response = await axios.get('https://shinkai.com');
            return response.data;
        }
        "#;

        let package = r#"{
            "name": "test-axios",
            "version": "1.0.0",
            "dependencies": {
                "axios": "^1.6.0"
            }
        }"#;

        let result =
            TypescriptUnsafeProcessorTool::run_node_code(code, Some(package), "{}", "{}", &HashMap::new()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_parameters() {
        let code = r#"
        async function run(config: any, inputs: any) {
            const { name, age } = inputs;
            return `Hello ${name}, you are ${age} years old!`;
        }
        "#;

        let package = r#"{
            "name": "test-parameters",
            "version": "1.0.0",
            "dependencies": {}
        }"#;

        let result = TypescriptUnsafeProcessorTool::run_node_code(
            code,
            Some(package),
            "{ \"name\": \"Alice\", \"age\": 30 }",
            "{}",
            &HashMap::new(),
        )
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Hello Alice, you are 30 years old!"));
    }

    #[tokio::test]
    async fn test_with_config() {
        let code = r#"
        interface Config {
            mode: string;
            maxRetries: number;
        }

        async function run(config: Config, inputs: any) {
            return `Running in ${config.mode} mode with ${config.maxRetries} retries`;
        }
        "#;

        let package = r#"{
            "name": "test-config",
            "version": "1.0.0",
            "dependencies": {}
        }"#;

        let result = TypescriptUnsafeProcessorTool::run_node_code(
            code,
            Some(package),
            "{}",
            "{ \"mode\": \"debug\", \"maxRetries\": 3 }",
            &HashMap::new(),
        )
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Running in debug mode with 3 retries"));
    }

    #[tokio::test]
    async fn test_invalid_typescript() {
        let code = r#"
        // This is intentionally invalid TypeScript code
        async function run(config: any, inputs: any) {
            return z;
        }
        "#;

        let package = r#"{
            "name": "test-invalid-ts",
            "version": "1.0.0",
            "dependencies": {}
        }"#;

        let result =
            TypescriptUnsafeProcessorTool::run_node_code(code, Some(package), "{}", "{}", &HashMap::new()).await;
        println!("Result: {:?}", result);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_without_package() {
        let code = r#"
        async function run(config: any, inputs: any) {
            console.log("DO NOT MATCH");
            return { message: "Hello without package.json!" };
        }
        "#;

        let result = TypescriptUnsafeProcessorTool::run_node_code(code, None, "{}", "{}", &HashMap::new()).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.contains(r#"{"message":"Hello without package.json!"}"#));
        assert!(!result.contains("DO NOT MATCH"));
    }
}
