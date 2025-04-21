use std::collections::HashMap;
use std::sync::Arc;
use std::process::Command;
use std::fs;
use std::env;

use serde_json::{Map, Value};
use wasmtime::{Engine, Instance, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::tool_config::{OAuth, ToolConfig};
use shinkai_tools_primitives::tools::tool_types::{OperatingSystem, RunnerType};
use shinkai_tools_primitives::tools::wasm_tools::WasmTool;

async fn compile_deno_to_wasm(code: &str) -> Result<Vec<u8>, ToolError> {
    // Create a temporary file with the Deno code
    let temp_dir = std::env::temp_dir();
    let input_file = temp_dir.join("input.ts");
    let output_file = temp_dir.join("output.wasm");
    
    std::fs::write(&input_file, code)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to write temp file: {}", e)))?;

    // Compile Deno code to WASM using deno compile
    let output = Command::new("deno")
        .args(&["compile", "--target", "wasm", "--output", output_file.to_str().unwrap(), input_file.to_str().unwrap()])
        .output()
        .map_err(|e| ToolError::ExecutionError(format!("Failed to compile Deno to WASM: {}", e)))?;

    if !output.status.success() {
        return Err(ToolError::ExecutionError(format!(
            "Failed to compile Deno to WASM: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    // Read the compiled WASM
    let wasm_bytes = std::fs::read(&output_file)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to read WASM file: {}", e)))?;

    // Clean up temp files
    let _ = std::fs::remove_file(&input_file);
    let _ = std::fs::remove_file(&output_file);

    Ok(wasm_bytes)
}

async fn compile_python_to_wasm(code: &str) -> Result<Vec<u8>, ToolError> {
    // Create a temporary directory for the Python project
    let temp_dir = std::env::temp_dir().join("python_wasm");
    fs::create_dir_all(&temp_dir)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp directory: {}", e)))?;

    // Write the Python code to a file
    let input_file = temp_dir.join("main.py");
    fs::write(&input_file, code)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to write Python file: {}", e)))?;

    // Create a requirements.txt file with pyodide
    let requirements = temp_dir.join("requirements.txt");
    fs::write(&requirements, "pyodide==0.25.0")
        .map_err(|e| ToolError::ExecutionError(format!("Failed to write requirements: {}", e)))?;

    // Create a setup.py file
    let setup_py = temp_dir.join("setup.py");
    fs::write(&setup_py, r#"
from setuptools import setup

setup(
    name="python_wasm",
    version="0.1",
    py_modules=["main"],
    install_requires=["pyodide==0.25.0"],
)
"#).map_err(|e| ToolError::ExecutionError(format!("Failed to write setup.py: {}", e)))?;

    // Install pyodide and compile to WASM
    let output = Command::new("python")
        .args(&["-m", "pip", "install", "pyodide"])
        .output()
        .map_err(|e| ToolError::ExecutionError(format!("Failed to install pyodide: {}", e)))?;

    if !output.status.success() {
        return Err(ToolError::ExecutionError(format!(
            "Failed to install pyodide: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    // Compile Python to WASM using pyodide
    let output = Command::new("python")
        .args(&["-m", "pyodide", "build", "--output", "output.wasm", "main.py"])
        .current_dir(&temp_dir)
        .output()
        .map_err(|e| ToolError::ExecutionError(format!("Failed to compile Python to WASM: {}", e)))?;

    if !output.status.success() {
        return Err(ToolError::ExecutionError(format!(
            "Failed to compile Python to WASM: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    // Read the compiled WASM
    let wasm_file = temp_dir.join("output.wasm");
    let wasm_bytes = fs::read(&wasm_file)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to read WASM file: {}", e)))?;

    // Clean up temp files
    let _ = fs::remove_dir_all(&temp_dir);

    Ok(wasm_bytes)
}

async fn execute_in_tee(wasm_bytes: &[u8], tee_type: &str) -> Result<Value, ToolError> {
    match tee_type {
        "apple" => {
            // For Apple Silicon, use enarx runtime
            let temp_dir = env::temp_dir().join("tee_execution");
            fs::create_dir_all(&temp_dir)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp directory: {}", e)))?;

            let wasm_file = temp_dir.join("input.wasm");
            fs::write(&wasm_file, wasm_bytes)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to write WASM file: {}", e)))?;

            // Execute using enarx with Apple backend
            let output = Command::new("enarx")
                .args(&["run", "--backend=apple", wasm_file.to_str().unwrap()])
                .output()
                .map_err(|e| ToolError::ExecutionError(format!("Failed to execute in TEE: {}", e)))?;

            if !output.status.success() {
                return Err(ToolError::ExecutionError(format!(
                    "TEE execution failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }

            let result = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(&result)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to parse TEE output: {}", e)))
        }
        "intel" => {
            // For Intel SGX, use enarx with SGX backend
            let temp_dir = env::temp_dir().join("tee_execution");
            fs::create_dir_all(&temp_dir)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp directory: {}", e)))?;

            let wasm_file = temp_dir.join("input.wasm");
            fs::write(&wasm_file, wasm_bytes)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to write WASM file: {}", e)))?;

            // Execute using enarx with SGX backend
            let output = Command::new("enarx")
                .args(&["run", "--backend=sgx", wasm_file.to_str().unwrap()])
                .output()
                .map_err(|e| ToolError::ExecutionError(format!("Failed to execute in TEE: {}", e)))?;

            if !output.status.success() {
                return Err(ToolError::ExecutionError(format!(
                    "TEE execution failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }

            let result = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(&result)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to parse TEE output: {}", e)))
        }
        "amd" => {
            // For AMD SEV, use enarx with SEV backend
            let temp_dir = env::temp_dir().join("tee_execution");
            fs::create_dir_all(&temp_dir)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to create temp directory: {}", e)))?;

            let wasm_file = temp_dir.join("input.wasm");
            fs::write(&wasm_file, wasm_bytes)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to write WASM file: {}", e)))?;

            // Execute using enarx with SEV backend
            let output = Command::new("enarx")
                .args(&["run", "--backend=sev", wasm_file.to_str().unwrap()])
                .output()
                .map_err(|e| ToolError::ExecutionError(format!("Failed to execute in TEE: {}", e)))?;

            if !output.status.success() {
                return Err(ToolError::ExecutionError(format!(
                    "TEE execution failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }

            let result = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(&result)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to parse TEE output: {}", e)))
        }
        _ => Err(ToolError::ExecutionError("Unsupported TEE type".to_string())),
    }
}

pub async fn execute_wasm_tool(
    bearer: String,
    db: Arc<SqliteManager>,
    node_name: ShinkaiName,
    parameters: Map<String, Value>,
    extra_config: Vec<ToolConfig>,
    oauth: Option<Vec<OAuth>>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    support_files: HashMap<String, String>,
    code: String,
    mounts: Option<Vec<String>>,
    runner: Option<RunnerType>,
    operating_system: Option<Vec<OperatingSystem>>,
) -> Result<Value, ToolError> {
    // Check if the code is Python or Deno code
    let wasm_bytes = if code.trim().starts_with("// deno") || code.contains("deno") {
        compile_deno_to_wasm(&code).await?
    } else if code.trim().starts_with("# python") || code.contains("def ") || code.contains("import ") {
        compile_python_to_wasm(&code).await?
    } else {
        code.as_bytes().to_vec()
    };

    // Detect TEE type based on architecture
    let tee_type = if cfg!(target_arch = "aarch64") {
        "apple"
    } else if cfg!(target_arch = "x86_64") {
        // Check for Intel SGX or AMD SEV
        if Command::new("is-sgx-available").output().is_ok() {
            "intel"
        } else {
            "amd"
        }
    } else {
        return Err(ToolError::ExecutionError("Unsupported architecture".to_string()));
    };

    // Execute in TEE
    execute_in_tee(&wasm_bytes, tee_type).await
}

pub async fn check_wasm_tool(
    tool_id: String,
    app_id: String,
    support_files: HashMap<String, String>,
    code: String,
) -> Result<Vec<String>, ToolError> {
    // Check if the code is Python or Deno code
    let wasm_bytes = if code.trim().starts_with("// deno") || code.contains("deno") {
        compile_deno_to_wasm(&code).await?
    } else if code.trim().starts_with("# python") || code.contains("def ") || code.contains("import ") {
        compile_python_to_wasm(&code).await?
    } else {
        code.as_bytes().to_vec()
    };

    // Create a minimal WasmTool instance
    let tool = WasmTool {
        name: "wasm_runtime".to_string(),
        homepage: None,
        author: "@@system.shinkai".to_string(),
        version: "1.0".to_string(),
        mcp_enabled: Some(false),
        wasm_code: String::from_utf8_lossy(&wasm_bytes).to_string(),
        tools: vec![],
        config: vec![],
        oauth: None,
        description: "WASM runtime execution".to_string(),
        keywords: vec![],
        input_args: shinkai_tools_primitives::tools::parameters::Parameters::new(),
        output_arg: shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg {
            json: "".to_string(),
        },
        activated: true,
        embedding: None,
        result: shinkai_tools_primitives::tools::tool_types::ToolResult::new(
            "object".to_string(),
            Value::Null,
            vec![],
        ),
        sql_tables: None,
        sql_queries: None,
        file_inbox: None,
        assets: None,
        runner: RunnerType::OnlyHost,
        operating_system: vec![
            OperatingSystem::Linux,
            OperatingSystem::MacOS,
            OperatingSystem::Windows,
        ],
        tool_set: None,
        tee_config: None,
    };

    // Validate WASM code
    let engine = Engine::default();
    match Module::from_binary(&engine, &wasm_bytes) {
        Ok(_) => Ok(vec![]),
        Err(e) => Err(ToolError::ExecutionError(format!(
            "Invalid WASM code: {}",
            e
        ))),
    }
} 