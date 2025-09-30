use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::SigningKey;
use regex::Regex;
use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::{
    shinkai_name::ShinkaiName,
    shinkai_tools::DynamicToolType,
    tool_router_key::ToolRouterKey,
};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiToolHeader;
use shinkai_tools_primitives::tools::tool_config::ToolConfig;
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_execution::execution_coordinator::execute_code;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;

pub struct CodeExecutionProcessorTool {
    pub tool: ShinkaiToolHeader,
    pub _tool_embedding: Option<Vec<f32>>,
}

impl CodeExecutionProcessorTool {
    pub fn new() -> Self {
        let mut params = Parameters::new();
        params.add_property(
            "language".to_string(),
            "string".to_string(),
            "Execution language. Supported: typescript or python.".to_string(),
            true,
            None,
        );
        params.add_property(
            "code".to_string(),
            "string".to_string(),
            "Source code to execute.".to_string(),
            true,
            None,
        );

        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai Code Execution Processor".to_string(),
                description: "Execute arbitrary TypeScript or Python code using Shinkai's dynamic runtimes.".to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_code_execution_processor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Execute Python or TypeScript code".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0.0".to_string(),
                enabled: true,
                mcp_enabled: Some(false),
                input_args: params,
                output_arg: ToolOutputArg {
                    json: r#"{"type":"object","properties":{"stdout":{"type":"string"},"stderr":{"type":"string"},"result":{"type":"object"},"__created_files__":{"type":"array","items":{"type":"string"}}}}"#.to_string(),
                },
                config: None,
                usage_type: None,
                tool_offering: None,
            },
            _tool_embedding: None,
        }
    }
}

fn parse_dynamic_tool_type(language: &str) -> Result<DynamicToolType, ToolError> {
    match language.to_lowercase().as_str() {
        "typescript" | "ts" => Ok(DynamicToolType::DenoDynamic),
        "python" | "py" => Ok(DynamicToolType::PythonDynamic),
        other => Err(ToolError::ExecutionError(format!(
            "Unsupported language '{}'. Use 'typescript' or 'python'.",
            other
        ))),
    }
}

fn split_lines_with_terminator(input: &str) -> Vec<&str> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut start = 0;
    for (idx, ch) in input.char_indices() {
        if ch == '\n' {
            segments.push(&input[start..=idx]);
            start = idx + 1;
        }
    }
    if start < input.len() {
        segments.push(&input[start..]);
    }
    segments
}

fn extract_python_dependencies(code: &str) -> (Vec<String>, String) {
    if code.trim().is_empty() {
        return (Vec::new(), code.to_string());
    }

    let segments = split_lines_with_terminator(code);
    if segments.is_empty() {
        return (Vec::new(), code.to_string());
    }

    let mut idx = 0;
    while idx < segments.len() && segments[idx].trim().is_empty() {
        idx += 1;
    }

    if idx >= segments.len() {
        return (Vec::new(), code.to_string());
    }

    let first_trimmed = segments[idx].trim();
    if !first_trimmed.starts_with('#') {
        return (Vec::new(), code.to_string());
    }

    let first_comment = first_trimmed.trim_start_matches('#').trim();
    if first_comment != "/// script" {
        return (Vec::new(), code.to_string());
    }

    let header_start_idx = idx;
    let mut header_end_idx = idx;
    let mut dependency_block = String::new();
    let mut collecting_dependencies = false;
    let mut header_closed = false;

    for j in header_start_idx..segments.len() {
        let line = segments[j];
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            break;
        }

        let comment_text = trimmed.trim_start_matches('#').trim();
        if j == header_start_idx && comment_text != "/// script" {
            return (Vec::new(), code.to_string());
        }

        header_end_idx = j + 1;

        if comment_text == "///" {
            header_closed = true;
            break;
        }

        if comment_text.starts_with("dependencies") {
            if let Some(pos) = comment_text.find('[') {
                collecting_dependencies = true;
                dependency_block.push_str(&comment_text[pos..]);
                dependency_block.push('\n');
                if comment_text[pos..].contains(']') {
                    collecting_dependencies = false;
                }
            }
        } else if collecting_dependencies {
            dependency_block.push_str(comment_text);
            dependency_block.push('\n');
        }

        if collecting_dependencies && comment_text.contains(']') {
            collecting_dependencies = false;
        }
    }

    if !header_closed {
        return (Vec::new(), code.to_string());
    }

    let mut dependencies = Vec::new();
    if !dependency_block.is_empty() {
        if let Ok(regex) = Regex::new(r#"["']([^"']+)["']"#) {
            for capture in regex.captures_iter(&dependency_block) {
                let dep = capture[1].trim();
                if !dep.is_empty() && !dependencies.iter().any(|existing| existing == dep) {
                    dependencies.push(dep.to_string());
                }
            }
        }
    }

    let mut remaining = String::new();
    for segment in &segments[..header_start_idx] {
        remaining.push_str(segment);
    }
    for segment in &segments[header_end_idx..] {
        remaining.push_str(segment);
    }

    (dependencies, remaining)
}

fn format_dependency_comment(dependencies: &[String]) -> String {
    if dependencies.is_empty() {
        return String::new();
    }

    let mut comment = String::from("# /// script\n# dependencies = [\n");
    for dependency in dependencies {
        comment.push_str("#   \"");
        comment.push_str(dependency);
        comment.push_str("\",\n");
    }
    comment.push_str("# ]\n# ///\n\n");
    comment
}

fn wrap_python_code(code: &str, dependencies: &[String]) -> String {
    let encoded = STANDARD.encode(code);
    let dependency_comment = format_dependency_comment(dependencies);
    format!(
        r#"{dependency_comment}
import base64
import contextlib
import traceback
import importlib
import subprocess
import sys
from io import StringIO

USER_CODE = base64.b64decode("{encoded}").decode("utf-8")

def run(config, inputs):
    exec_globals = {{"config": config, "inputs": inputs, "__name__": "__main__"}}
    exec_globals["__builtins__"] = __builtins__
    stdout_buffer = StringIO()
    stderr_buffer = StringIO()
    try:
        with contextlib.redirect_stdout(stdout_buffer), contextlib.redirect_stderr(stderr_buffer):
            exec(USER_CODE, exec_globals)
    except Exception:
        return {{
            "data": {{
                "stdout": stdout_buffer.getvalue(),
                "stderr": stderr_buffer.getvalue(),
                "error": traceback.format_exc(),
            }}
        }}

    response = {{}}
    stdout_text = stdout_buffer.getvalue()
    stderr_text = stderr_buffer.getvalue()
    if "result" in exec_globals:
        response["result"] = exec_globals["result"]
    if stdout_text:
        response["stdout"] = stdout_text
    if stderr_text:
        response["stderr"] = stderr_text
    if not response:
        response["status"] = "success"
    return response
"#
    )
}

fn indent_code(code: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    code
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn wrap_deno_code(code: &str) -> String {
    let indented = indent_code(code, 8);
    format!(
        r#"export async function run(config, inputs) {{
    const response = {{}};
    const output = [];
    const capture = (args) => {{
        output.push(args.map((arg) => {{
            if (typeof arg === "string") {{
                return arg;
            }}
            try {{
                return JSON.stringify(arg);
            }} catch (_) {{
                return String(arg);
            }}
        }}).join(" "));
    }};
    const originalLog = console.log;
    const originalError = console.error;
    console.log = (...args) => {{
        capture(args);
        originalLog(...args);
    }};
    console.error = (...args) => {{
        capture(args);
        originalError(...args);
    }};
    try {{
        const userCode = async (config, inputs) => {{
{indented}
        }};
        const result = await userCode(config, inputs);
        if (typeof result !== "undefined") {{
            response.result = result;
        }}
    }} catch (error) {{
        if (error instanceof Error) {{
            response.error = `${{error.name}}: ${{error.message}}\n${{error.stack ?? ""}}`;
        }} else {{
            response.error = String(error);
        }}
    }} finally {{
        console.log = originalLog;
        console.error = originalError;
    }}
    if (output.length) {{
        response.stdout = output.join("\n");
    }}
    if (!("status" in response) && !("error" in response) && !("stdout" in response) && !("result" in response)) {{
        response.status = "success";
    }}
    return response;
}}
"#
    )
}

fn prepare_code(tool_type: &DynamicToolType, code: &str, dependencies: &[String]) -> String {
    match tool_type {
        DynamicToolType::PythonDynamic => wrap_python_code(code, dependencies),
        DynamicToolType::DenoDynamic => wrap_deno_code(code),
        _ => code.to_string(),
    }
}

#[async_trait]
impl ToolExecutor for CodeExecutionProcessorTool {
    async fn execute(
        bearer: String,
        tool_id: String,
        app_id: String,
        db_clone: Arc<SqliteManager>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        llm_provider: String,
    ) -> Result<Value, ToolError> {
        let language = parameters
            .get("language")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::ExecutionError("'language' parameter is required".to_string()))?;
        let tool_type = parse_dynamic_tool_type(language)?;

        let raw_code = parameters
            .get("code")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::ExecutionError("'code' parameter is required".to_string()))?;

        let (dependencies, executable_code) = if let DynamicToolType::PythonDynamic = tool_type {
            extract_python_dependencies(raw_code)
        } else {
            (Vec::new(), raw_code.to_string())
        };

        let prepared_code = prepare_code(&tool_type, &executable_code, &dependencies);

        let execution_parameters: Map<String, Value> = Map::new();
        let tools: Vec<ToolRouterKey> = Vec::new();
        let extra_config: Vec<ToolConfig> = Vec::new();

        execute_code(
            tool_type,
            prepared_code,
            tools,
            execution_parameters,
            extra_config,
            None,
            db_clone,
            tool_id,
            app_id,
            None,
            llm_provider,
            bearer,
            node_name_clone,
            None,
            None,
            None,
            identity_manager_clone,
            job_manager_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
        )
        .await
    }
}
