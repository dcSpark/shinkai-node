use std::{collections::HashMap, path::PathBuf};

use super::execution_header_generator::generate_execution_environment;
use crate::utils::environment::fetch_node_environment;
use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{
    deno_tools::ToolResult,
    error::ToolError,
    parameters::Parameters,
    python_tools::PythonTool,
    tool_config::{OAuth, ToolConfig},
    tool_output_arg::ToolOutputArg,
};
use std::sync::Arc;

pub async fn execute_python_tool(
    _bearer: String,
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
) -> Result<Value, ToolError> {
    // Create a minimal DenoTool instance
    let tool = PythonTool {
        toolkit_name: "python".to_string(),
        name: "python_runtime".to_string(),
        author: "system".to_string(),
        py_code: code,
        tools: None,
        config: vec![],
        description: "Python runtime execution".to_string(),
        keywords: vec![],
        input_args: Parameters::new(),
        output_arg: ToolOutputArg { json: "".to_string() },
        activated: true,
        embedding: None,
        result: ToolResult::new("object".to_string(), Value::Null, vec![]),
        sql_tables: None,
        sql_queries: None,
        file_inbox: None,
        oauth: oauth.clone(),
        assets: None,
    };

    let env = generate_execution_environment(
        db.clone(),
        llm_provider.clone(),
        app_id.clone(),
        tool_id.clone(),
        "code-execution".to_string(),
        "".to_string(),
        &oauth.clone(),
    )
    .await?;

    let node_env = fetch_node_environment();
    let node_storage_path = node_env
        .node_storage_path
        .clone()
        .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;

    // Get Assets for Playground;
    // Read all files in the assets directory
    let assets_path = PathBuf::from(&node_storage_path)
        .join(".tools_storage")
        .join("playground")
        .join(app_id.clone());

    let mut assets_files = Vec::new();
    if assets_path.exists() {
        for entry in std::fs::read_dir(assets_path)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read assets directory: {}", e)))?
        {
            let entry =
                entry.map_err(|e| ToolError::ExecutionError(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            if path.is_file() {
                assets_files.push(path);
            }
        }
    }

    match tool.run_on_demand(
        env,
        node_env.api_listen_address.ip().to_string(),
        node_env.api_listen_address.port(),
        support_files,
        parameters,
        extra_config,
        node_storage_path,
        app_id.clone(),
        tool_id.clone(),
        node_name,
        false,
        assets_files,
        mounts,
    ) {
        Ok(run_result) => Ok(run_result.data),
        Err(e) => Err(e),
    }
}
