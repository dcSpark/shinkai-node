use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::{Parameters, Property};
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;

use tokio::sync::Mutex;

use async_trait::async_trait;
use std::process::{Command, Stdio};
use std::{env, fs};

// LLM Tool
pub struct StagehandProcessorTool {
    pub tool: ShinkaiToolHeader,
    pub tool_embedding: Option<Vec<f32>>,
}

impl StagehandProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai Stagehand Processor".to_string(),
                description: "Tool for executing Node.js code in a sandboxed environment".to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_stagehand_processor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Tool for executing Stagehand".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0.0".to_string(),
                enabled: true,
                input_args: {
                    let mut params = Parameters::new();

                    // Create the properties for the command object
                    let mut command_props = std::collections::HashMap::new();
                    command_props.insert(
                        "id".to_string(),
                        Property::new("string".to_string(), "Unique identifier for the command".to_string()),
                    );

                    // Create enum-like property for action
                    let mut action_prop = Property::new(
                        "string".to_string(),
                        "Type of action to perform: 'goto' | 'wait' | 'evaluate' | 'act' | 'goto-stage' ".to_string(),
                    );
                    action_prop.property_type = "enum".to_string();
                    command_props.insert("action".to_string(), action_prop);

                    command_props.insert(
                        "payload".to_string(),
                        Property::new(
                            "string".to_string(),
                            "Action Payload: goto=>url, wait=>ms, evaluate=>text-prompt, act=>text-prompt, goto-stage=>stage-id"
                                .to_string(),
                        ),
                    );

                    // Optional jsonSchema property
                    command_props.insert(
                        "jsonSchema".to_string(),
                        Property::new(
                            "object".to_string(),
                            "Optional JSON schema for actions 'evaluate' and 'act'".to_string(),
                        ),
                    );

                    // Create the command object property
                    let command_object = Property::with_nested_properties(
                        "object".to_string(),
                        "A command to execute".to_string(),
                        command_props,
                    );

                    // Create the commands array property
                    let commands_array =
                        Property::with_array_items("Array of commands to execute".to_string(), command_object);

                    // Add the commands array to the parameters
                    params.properties.insert("commands".to_string(), commands_array);
                    params.required.push("commands".to_string());

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

    async fn run_node_code(code: &str, package: &str) -> Result<String, ToolError> {
        // Get npm and node binary locations from env or use defaults
        let npm_binary = env::var("NPM_BINARY_LOCATION").unwrap_or_else(|_| "npm".to_string());
        let node_binary = env::var("NODE_BINARY_LOCATION").unwrap_or_else(|_| "node".to_string());

        // Create temporary directory with random name
        let root_temp_dir = env::temp_dir();
        let random_id = format!("shinkai_stagehand_{}", uuid::Uuid::new_v4());
        let temp_path = root_temp_dir.join(random_id);

        fs::create_dir_all(temp_path.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp dir: {}", e)))?;
        // Write files
        fs::write(temp_path.clone().join("package.json"), package)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write package.json: {}", e)))?;
        fs::write(temp_path.clone().join("index.ts"), code)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write index.ts: {}", e)))?;

        // Run npm install
        let npm_output = Command::new(&npm_binary)
            .arg("install")
            .current_dir(temp_path.clone())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
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

        if !npm_output.status.success() {
            return Err(ToolError::ExecutionError(format!(
                "npm install failed: {}",
                String::from_utf8_lossy(&npm_output.stderr)
            )));
        }

        // Run node index.ts
        let node_output = Command::new(&node_binary)
            .arg("--experimental-strip-types")
            .arg("index.ts")
            .current_dir(temp_path.clone())
            .stdout(Stdio::inherit())
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
        // Some error messaages:
        // ❌ Error in game loop: proxy.evaluate: Target page, context or browser has been closed
        // proxy.evaluate: Target page, context or browser has been closed
        let stdout = String::from_utf8_lossy(&node_output.stdout).to_string();

        if !node_output.status.success() {
            return Err(ToolError::ExecutionError(format!(
                "Node execution failed: {}\nOutput: {}",
                String::from_utf8_lossy(&node_output.stderr),
                stdout
            )));
        }

        Ok(stdout)
    }
}

fn get_ts_code(json_string: String) -> String {
    let code = format!(
        r#"
import {{ jsonSchemaToZod }} from "json-schema-to-zod";
import pkg from '@browserbasehq/stagehand';
const {{ Stagehand, ConstructorParams }} = pkg;
import {{ z }} from "zod";
z.object({{}});

async function stagehandRun(config: CONFIG, inputs: INPUTS) {{
    const stagehandConfig: ConstructorParams = {{
        env: "LOCAL",
        modelName: "gpt-4o",
        modelClientOptions: {{
            apiKey: "YOUR_OPENAI_API_KEY",
        }},
        enableCaching: false,
        debugDom: true /* Enable DOM debugging features */,
        headless: false /* Run browser in headless mode */,
        domSettleTimeoutMs: 10_000 /* Timeout for DOM to settle in milliseconds */,
        verbose: 1,
    }}

    console.log("🎮 Starting 2048 bot...");
    const stagehand = new Stagehand(stagehandConfig);
    try {{
        console.log("🌟 Initializing Stagehand...");
        await stagehand.init();
        console.log("🌐 Navigating to 2048...");
        for (const input of inputs.commands) {{
            switch (input.action) {{
                case "goto":
                    await stagehand.page.goto(input.payload);
                    break;
                case "wait":
                    await new Promise((resolve) => setTimeout(resolve, parseInt(input.payload)));
                    break;
                case "act":
                    await stagehand.page.act(input.payload);
                    break;
                case "goto-stage":
                    await stagehand.gotoStage(input.payload);
                    break;
            }}
        }}
    }} catch (error) {{
        console.error("❌ Error", error);
        throw error; // Re-throw non-game-over errors
    }}
}}


const x = jsonSchemaToZod({{
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "properties": {{
        "score": {{
            "type": "number"
        }},
        "highestTile": {{
            "type": "number"
        }},
        "grid": {{
            "type": "array",
            "items": {{
                "type": "array",
                "items": {{
                    "type": "number"
                }}
            }}
        }}
    }},
    "required": ["score", "highestTile", "grid"],
    "additionalProperties": false
}});
// console.log({{ x, z: !!z.object({{}}) }});
const scoreSchema = eval(x);

const moveSchema = eval(jsonSchemaToZod({{
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "properties": {{
        "move": {{
            "type": "string",
            "enum": ["up", "down", "left", "right"]
        }},
        "confidence": {{
            "type": "number"
        }},
        "reasoning": {{
            "type": "string"
        }}
    }},
    "required": ["move", "confidence", "reasoning"],
    "additionalProperties": false
}}));

type CONFIG = {{}};
type INPUTS = {{
    commands: {{
        id: string,
        action: 'goto' | 'wait' | 'evaluate' | 'act' | 'goto-stage',
        payload: string,
        jsonSchema?: object
    }}[]
}};

type OUTPUTS = {{ message: string }};
const inputs = {json_string}
;

async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUTS> {{
    await stagehandRun(config, inputs);
    return {{ message: "OK" }};
}}

run({{}}, inputs).then((result) => {{
    console.log(">>>", result);
}}).catch((error) => {{
    console.error(">>>", error);
}});

"#
    );
    println!("STAGEHAND CODE");
    println!("================");
    println!("{}", code);
    println!("================");
    return code.to_string();
}

fn get_ts_package() -> String {
    let package = r#"
{
  "name": "standalone",
  "version": "1.0.0",
  "main": "index.ts",
  "scripts": {
    "test": "echo \"Error: no test specified\" && exit 1"
  },
  "author": "",
  "license": "ISC",
  "description": "",
  "dependencies": {
    "@browserbasehq/stagehand": "^1.10.0",
    "json-schema-to-zod": "^2.6.0",
    "zod": "^3.24.1"
  }
}
"#;
    return package.to_string();
}

#[async_trait]
impl ToolExecutor for StagehandProcessorTool {
    async fn execute(
        _bearer: String,
        _tool_id: String,
        _app_id: String,
        _db_clone: Arc<SqliteManager>,
        _node_name_clone: ShinkaiName,
        _identity_manager_clone: Arc<Mutex<IdentityManager>>,
        _job_manager_clone: Arc<Mutex<JobManager>>,
        _encryption_secret_key_clone: EncryptionStaticKey,
        _encryption_public_key_clone: EncryptionPublicKey,
        _signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        _llm_provider: String,
    ) -> Result<Value, ToolError> {
        // Extract the commands array from parameters
        let json_string = serde_json::to_string(&parameters)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to serialize parameters: {}", e)))?;

        let code = get_ts_code(json_string);
        let package = get_ts_package();

        // Run the Node.js code and get the output
        let stdout = Self::run_node_code(&code, &package).await?;

        Ok(json!({
            "stdout": stdout
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[tokio::test]
    // async fn run_2048() {
    //     let code = get_ts_code();
    //     let package = get_ts_package();
    //     let stdout = StagehandProcessorTool::run_node_code(&code, &package).await;
    //     assert!(stdout.is_ok(), "Node.js execution failed: {:?}", stdout.err());
    // }

    #[tokio::test]
    async fn test_run_node_code() {
        // Test a simple Node.js program
        let code = r#"
            const message = 'Hello from Node.js!';
            console.log(message);
        "#;

        let package = r#"{
            "name": "test-node-app",
            "version": "1.0.0",
            "description": "Test Node.js application",
            "main": "index.ts",
            "scripts": {
                "test": "echo \"Error: no test specified\" && exit 1"
            }
        }"#;

        let result = StagehandProcessorTool::run_node_code(code, package).await;
        assert!(result.is_ok(), "Node.js execution failed: {:?}", result.err());

        let output = result.unwrap();
        assert!(output.contains("Hello from Node.js!"), "Expected output not found");
    }

    #[tokio::test]
    async fn test_run_node_code_with_dependencies() {
        // Test Node.js program with an external dependency
        let code = r#"
            const chalk = require('chalk');
            console.log(chalk.blue('Hello in blue!'));
        "#;

        let package = r#"{
            "name": "test-node-app",
            "version": "1.0.0",
            "description": "Test Node.js application",
            "main": "index.ts",
            "dependencies": {
                "chalk": "^4.1.2"
            }
        }"#;

        let result = StagehandProcessorTool::run_node_code(code, package).await;
        assert!(result.is_ok(), "Node.js execution failed: {:?}", result.err());

        let output = result.unwrap();
        assert!(output.contains("Hello in blue!"), "Expected output not found");
    }

    #[tokio::test]
    async fn test_run_node_code_with_error() {
        // Test Node.js program with a syntax error
        let code = r#"
            console.log('This has a syntax error'
        "#; // Missing closing parenthesis

        let package = r#"{
            "name": "test-node-app",
            "version": "1.0.0"
        }"#;

        let result = StagehandProcessorTool::run_node_code(code, package).await;
        assert!(result.is_err(), "Expected an error for invalid syntax");
    }
}
