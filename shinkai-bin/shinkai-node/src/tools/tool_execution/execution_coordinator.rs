use crate::llm_provider::job_manager::JobManager;
use crate::tools::tool_definitions::definition_generation::generate_tool_definitions;
use crate::tools::tool_execution::execution_custom::execute_custom_tool;
use crate::tools::tool_execution::execution_deno_dynamic::{check_deno_tool, execute_deno_tool};
use crate::tools::tool_execution::execution_python_dynamic::execute_python_tool;
use crate::utils::environment::fetch_node_environment;
use serde_json::json;
use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::shinkai_tools::DynamicToolType;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;

use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use tokio::sync::{Mutex, RwLock};

use crate::managers::IdentityManager;
use ed25519_dalek::SigningKey;

use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn execute_tool(
    bearer: String,
    node_name: ShinkaiName,
    db: Arc<RwLock<SqliteManager>>,
    vector_fs: Arc<VectorFS>,
    tool_router_key: String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    extra_config: Vec<ToolConfig>,
    oauth: Vec<ToolConfig>,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
) -> Result<Value, ToolError> {
    eprintln!("[execute_tool] with tool_router_key: {}", tool_router_key);

    // Determine the tool type based on the tool_router_key
    if tool_router_key.contains("rust_toolkit") {
        // Execute as a Rust tool
        execute_custom_tool(
            &tool_router_key,
            parameters,
            tool_id,
            app_id,
            extra_config,
            oauth,
            bearer,
            db,
            vector_fs,
            llm_provider,
            node_name,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
        )
        .await
    } else {
        // Assume it's a Deno tool if not Rust
        let tool = db
            .read()
            .await
            .get_tool_by_key(&tool_router_key)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to get tool: {}", e)))?;

        match tool {
            ShinkaiTool::Deno(deno_tool, _) => {
                let mut envs = HashMap::new();
                envs.insert("BEARER".to_string(), bearer);
                envs.insert("X_SHINKAI_TOOL_ID".to_string(), tool_id.clone());
                envs.insert("X_SHINKAI_APP_ID".to_string(), app_id.clone());
                envs.insert("X_SHINKAI_INSTANCE_ID".to_string(), "".to_string()); // TODO Pass data from the API
                envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), llm_provider.clone());

                let node_env = fetch_node_environment();
                let node_storage_path = node_env
                    .node_storage_path
                    .clone()
                    .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
                let support_files = generate_tool_definitions(
                    deno_tool.tools.clone().unwrap_or_default(),
                    CodeLanguage::Typescript,
                    db,
                    false,
                )
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;

                deno_tool
                    .run(
                        envs,
                        node_env.api_listen_address.ip().to_string(),
                        node_env.api_listen_address.port(),
                        support_files,
                        parameters,
                        extra_config,
                        oauth,
                        node_storage_path,
                        app_id.clone(),
                        tool_id.clone(),
                        node_name,
                        true,
                    )
                    .map(|result| json!(result.data))
                    .map_err(|e| ToolError::ExecutionError(e.to_string()))
            }
            _ => Err(ToolError::ExecutionError(format!("Unsupported tool type: {:?}", tool))),
        }
    }
}

pub async fn execute_code(
    tool_type: DynamicToolType,
    code: String,
    tools: Vec<String>,
    parameters: Map<String, Value>,
    extra_config: Vec<ToolConfig>,
    oauth: Vec<ToolConfig>,
    sqlite_manager: Arc<RwLock<SqliteManager>>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    bearer: String,
    node_name: ShinkaiName,
) -> Result<Value, ToolError> {
    eprintln!("[execute_code] tool_type: {}", tool_type);

    // Route based on the prefix
    match tool_type {
        DynamicToolType::DenoDynamic => {
            let support_files = generate_tool_definitions(tools, CodeLanguage::Typescript, sqlite_manager, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            execute_deno_tool(
                bearer.clone(),
                node_name,
                parameters,
                extra_config,
                oauth,
                tool_id,
                app_id,
                llm_provider,
                support_files,
                code,
            )
        }
        DynamicToolType::PythonDynamic => {
            let support_files = generate_tool_definitions(tools, CodeLanguage::Python, sqlite_manager, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            execute_python_tool(
                bearer.clone(),
                node_name,
                parameters,
                extra_config,
                tool_id,
                app_id,
                llm_provider,
                support_files,
                code,
            )
        }
    }
}

pub async fn check_code(
    tool_type: DynamicToolType,
    unfiltered_code: String,
    tool_id: String,
    app_id: String,
    tools: Vec<String>,
    sqlite_manager: Arc<RwLock<SqliteManager>>,
) -> Result<Vec<String>, ToolError> {
    eprintln!("[check_code] tool_type: {}", tool_type);

    // Use the new function to extract fenced code blocks
    let code_blocks = extract_fenced_code_blocks(&unfiltered_code);
    let code_extracted = if !code_blocks.is_empty() {
        code_blocks.join("\n\n")
    } else {
        unfiltered_code
    };

    eprintln!("[check_code] code_extracted: {}", code_extracted);

    match tool_type {
        DynamicToolType::DenoDynamic => {
            let support_files = generate_tool_definitions(tools, CodeLanguage::Typescript, sqlite_manager, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            // Since `check_deno_tool` is synchronous, run it in a blocking task
            tokio::task::spawn_blocking(move || check_deno_tool(tool_id, app_id, support_files, code_extracted))
                .await
                .map_err(|e| ToolError::ExecutionError(format!("Task Join Error: {}", e)))?
        }
        DynamicToolType::PythonDynamic => Err(ToolError::ExecutionError("NYI Python".to_string())),
    }
}

fn extract_fenced_code_blocks(unfiltered_code: &str) -> Vec<String> {
    // Updated pattern to handle both formats in the regex
    let re = Regex::new(r"```(?:\w+(?:\\n|\n))?([\s\S]*?)```").unwrap();
    let matches: Vec<String> = re
        .captures_iter(unfiltered_code)
        .map(|cap| cap[1].to_string())
        .collect();

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fenced_code_blocks() {
        let input = r#"
          Based on the provided documentation and code, I will implement a tool that downloads a website into markdown. This involves using the `deno` library to make an HTTP request to the website and then parsing the HTML response to extract relevant information.\n\nFirst, let's import the necessary libraries and define our function signature:\n```typescript\ntype CONFIG = {};\ntype INPUTS = {\n  url: string;\n};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const { url } = inputs;\n\n  // ...\n}\n```\nNext, we can use the `deno` library to make an HTTP request to the website:\n```typescript\nimport { fetch } from 'deno';\n\nconst response = await fetch(url);\nconst html = await response.text();\n```\nThen, we can parse the HTML response using a markdown parser. For this example, let's use the `marked` library, which is available on npm:\n```typescript\nimport { marked } from 'npm:marked';\n\nconst markdown = marked(html);\n```\nFinally, we can return the markdown as our output:\n```typescript\nreturn {\n  markdown,\n};\n}\n```\nPutting it all together, our `run` function would look like this:\n```typescript\nimport { fetch } from 'deno';\nimport { marked } from 'npm:marked';\n\ntype CONFIG = {};\ntype INPUTS = {\n  url: string;\n};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const { url } = inputs;\n\n  const response = await fetch(url);\n  const html = await response.text();\n  const markdown = marked(html);\n\n  return {\n    markdown,\n  };\n}\n```\nThis tool can be used to download a website into markdown by calling the `run` function with the URL of the website as an argument.\n\nHere is the complete code:\n\n```typescript\nimport { fetch } from 'deno';\nimport { marked } from 'npm:marked';\n\ntype CONFIG = {};\ntype INPUTS = {\n  url: string;\n};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const { url } = inputs;\n\n  const response = await fetch(url);\n  const html = await response.text();\n  const markdown = marked(html);\n\n  return {\n    markdown,\n  };\n}\n```\n\nPlease note that this code is a simple example and might not cover all edge cases. Depending on the complexity of the website, you might need to adjust the parsing logic accordingly.
        "#;

        let result = extract_fenced_code_blocks(input);
        let expected = vec![
            "type CONFIG = {};\\ntype INPUTS = {\\n  url: string;\\n};\\ntype OUTPUT = {};\\n\\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\\n  const { url } = inputs;\\n\\n  // ...\\n}\\n".to_string(),
            "import { fetch } from 'deno';\\n\\nconst response = await fetch(url);\\nconst html = await response.text();\\n".to_string(),
            "import { marked } from 'npm:marked';\\n\\nconst markdown = marked(html);\\n".to_string(),
            "return {\\n  markdown,\\n};\\n}\\n".to_string(),
            "import { fetch } from 'deno';\\nimport { marked } from 'npm:marked';\\n\\ntype CONFIG = {};\\ntype INPUTS = {\\n  url: string;\\n};\\ntype OUTPUT = {};\\n\\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\\n  const { url } = inputs;\\n\\n  const response = await fetch(url);\\n  const html = await response.text();\\n  const markdown = marked(html);\\n\\n  return {\\n    markdown,\\n  };\\n}\\n".to_string(),
            "import { fetch } from 'deno';\\nimport { marked } from 'npm:marked';\\n\\ntype CONFIG = {};\\ntype INPUTS = {\\n  url: string;\\n};\\ntype OUTPUT = {};\\n\\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\\n  const { url } = inputs;\\n\\n  const response = await fetch(url);\\n  const html = await response.text();\\n  const markdown = marked(html);\\n\\n  return {\\n    markdown,\\n  };\\n}\\n".to_string(),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_extract_fenced_code_blocks_with_typescript() {
        let input = r#"Based on the provided documentation, we will implement a tool that downloads the webpage at `https://jhftss.github.io/` and converts it to plain text.

```typescript
import { getHomePath } from './shinkai-local-support.ts';

type CONFIG = {};
type INPUTS = {};
type OUTPUT = {};

export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {
  const url = 'https://jhftss.github.io/';
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const text = await response.text();
    const fileContent = text.replace(/<[^>]*>|[\n\r]/g, '');
    const filePath = `${getHomePath()}/downloaded_text.txt`;
    Deno.writeTextFileSync(filePath, fileContent);
  } catch (error) {
    console.error(error.message);
    return { error: 'Failed to download and convert webpage' };
  }
  return {};
}

```"#;

        let expected = vec![r#"import { getHomePath } from './shinkai-local-support.ts';

type CONFIG = {};
type INPUTS = {};
type OUTPUT = {};

export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {
  const url = 'https://jhftss.github.io/';
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const text = await response.text();
    const fileContent = text.replace(/<[^>]*>|[\n\r]/g, '');
    const filePath = `${getHomePath()}/downloaded_text.txt`;
    Deno.writeTextFileSync(filePath, fileContent);
  } catch (error) {
    console.error(error.message);
    return { error: 'Failed to download and convert webpage' };
  }
  return {};
}

"#
        .to_string()];

        let result = extract_fenced_code_blocks(input);
        assert_eq!(result, expected);
    }
}
