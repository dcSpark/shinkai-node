use super::parameters::Parameters;
use super::tool_config::ToolConfig;
use super::tool_output_arg::ToolOutputArg;
use super::tool_types::ToolResult;
use crate::tools::error::ToolError;
use shinkai_tools_runner::tools::run_result::RunResult;
use std::process::Stdio;
use std::{collections::HashMap, process::Command};
use std::{env, fs};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MCPServerTool {
    pub version: String,
    pub name: String,
    pub author: String,
    pub mcp_server_ref: String,
    pub description: String,
    pub mcp_server_url: String,
    pub mcp_server_tool: String,

    pub config: Vec<ToolConfig>,
    pub keywords: Vec<String>,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub activated: bool,
    pub embedding: Option<Vec<f32>>,
    pub result: ToolResult,
    pub tool_set: Option<String>,
}

impl MCPServerTool {
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
        parameters: serde_json::Map<String, serde_json::Value>,
        config: Vec<ToolConfig>,
    ) -> Result<RunResult, ToolError> {
        let r = self
            .run_node_code(
                &self.js().clone(),
                Some(&self.package_json().clone()),
                parameters,
                config,
                &HashMap::new(),
            )
            .await;
        match r {
            Ok(response_json) => {
                // Parse the response

                let result = serde_json::from_str(&response_json).unwrap_or_default();
                Ok(RunResult { data: result })
            }
            Err(e) => Err(e),
        }
    }

    async fn run_node_code(
        self: &Self,
        code: &str,
        package: Option<&str>,
        parameters: serde_json::Map<String, serde_json::Value>,
        config: Vec<ToolConfig>,
        envs: &HashMap<String, String>,
    ) -> Result<String, ToolError> {
        // Get npm and node binary locations from env or use defaults
        let npm_binary = env::var("NPM_BINARY_LOCATION").unwrap_or_else(|_| "npm".to_string());
        let node_binary = env::var("NODE_BINARY_LOCATION").unwrap_or_else(|_| "node".to_string());

        // TODO pass the parameters and config into the code
        let init_code = format!(
            r#"
        const inputs = {{}};
        const config = {{}};
        run(config, inputs).then((result) => {{
            console.log("<TYPE_SCRIPT_UNSAFE_PROCESSOR_OUTPUT>");
            console.log(JSON.stringify(result));
            console.log("</TYPE_SCRIPT_UNSAFE_PROCESSOR_OUTPUT>");
            setTimeout(() => {{
                process.exit(0);
            }}, 1000);
        }}).catch((error) => {{
            console.log("<TYPE_SCRIPT_UNSAFE_PROCESSOR_ERROR>");
            console.log(JSON.stringify({{ error }}));
            console.log("</TYPE_SCRIPT_UNSAFE_PROCESSOR_ERROR>");
        }});
        "#,
        );

        // Create temporary directory with random name
        let root_temp_dir = env::temp_dir();
        let random_id = format!(
            "shinkai_stagehand_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let temp_path = root_temp_dir.join(random_id);

        fs::create_dir_all(temp_path.clone())
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp dir: {}", e)))?;
        // Write files}
        println!("Folder created: {}", temp_path.display());

        let final_code = format!("{}\n{}", code, init_code);
        // println!("Running code: {}", final_code);
        fs::write(temp_path.clone().join("index.js"), final_code)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write index.js: {}", e)))?;

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

        // Run node index.js
        let node_output = Command::new(&node_binary)
            .envs(envs)
            .arg("index.js")
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

    fn js(self: &Self) -> String {
        let js = r#"
            import { Client } from "@modelcontextprotocol/sdk/client/index.js";
            import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

            async function run() {
                console.log("Starting client...");
                const transport = new StdioClientTransport({
                    command: "node",
                    args: ["/Users/edwardalvarado/mcp-server-tmdb/dist/index.js"]
                });

                console.log("Creating client...");
                const client = new Client(
                {
                    name: "example-client",
                    version: "1.0.0"
                },
                {
                    capabilities: {
                    prompts: {},
                    resources: {},
                    tools: {}
                    }
                }
                );

                console.log("Connecting client...");
                await client.connect(transport);

                // console.log("Listing prompts...");
                // // List prompts
                // const prompts = await client.listPrompts();
                // console.log(prompts);
                // Get a prompt
                // const prompt = await client.getPrompt("example-prompt", {
                //   arg1: "value"
                // });

                console.log("Listing resources...");
                // List resources
                const resources = await client.listTools();
                console.log(JSON.stringify(resources, null, 2));

                // Read a resource
                // const resource = await client.readResource("file:///example.txt");

                // // Call a tool
                const result = await client.callTool({
                name: "search_movies",
                arguments: {
                    query: "dune"
                }
                });

                console.log(JSON.stringify(result, null, 2));
                return result;
            }

        "#;
        return js.to_string();
    }

    fn package_json(self: &Self) -> String {
        let packagejson = r#"
        {
            "name": "client",
            "version": "1.0.0",
            "main": "index.js",
            "type": "module",
            "scripts": {
                "test": "echo \"Error: no test specified\" && exit 1"
            },
            "author": "",
            "license": "ISC",
            "description": "",
            "dependencies": {
                "@modelcontextprotocol/sdk": "^1.5.0"
            }
        }        
        "#;
        return packagejson.to_string();
    }

    pub fn check_required_config_fields(&self) -> bool {
        // Check if all required config fields are present
        true // For now, no required fields
    }
}
