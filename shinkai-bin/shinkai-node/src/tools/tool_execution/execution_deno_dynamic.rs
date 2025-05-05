use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_tools_primitives::tools::deno_tools::DenoTool;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::tool_config::{OAuth, ToolConfig};
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use shinkai_tools_primitives::tools::tool_types::{OperatingSystem, RunnerType, ToolResult};

use super::execution_header_generator::{check_tool, generate_execution_environment};
use crate::utils::environment::fetch_node_environment;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;

pub async fn execute_deno_tool(
    _bearer: String,
    db: Arc<SqliteManager>,
    node_name: ShinkaiName,
    parameters: Map<String, Value>,
    extra_config: Vec<ToolConfig>,
    oauth: Option<Vec<OAuth>>,
    tool_id: String,
    app_id: String,
    agent_id: Option<String>,
    llm_provider: String,
    support_files: HashMap<String, String>,
    code: String,
    mounts: Option<Vec<String>>,
    runner: Option<RunnerType>,
    operating_system: Option<Vec<OperatingSystem>>,
) -> Result<Value, ToolError> {
    let tool_router_key = ToolRouterKey::new(
        "local".to_string(),
        "@@system.shinkai".to_string(),
        "deno_runtime".to_string(),
        None,
    );

    // Create a minimal DenoTool instance
    let tool = DenoTool {
        name: "deno_runtime".to_string(),
        tool_router_key: Some(tool_router_key.clone()),
        homepage: None,
        author: "@@system.shinkai".to_string(),
        version: "1.0.0".to_string(),
        js_code: code,
        tools: vec![],
        config: extra_config.clone(),
        oauth: oauth.clone(),
        description: "Deno runtime execution".to_string(),
        keywords: vec![],
        mcp_enabled: Some(false),
        input_args: Parameters::new(),
        output_arg: ToolOutputArg { json: "".to_string() },
        activated: true,
        embedding: None,
        result: ToolResult::new("object".to_string(), Value::Null, vec![]),
        sql_tables: None,
        sql_queries: None,
        file_inbox: None,
        assets: None,
        runner: runner.unwrap_or_default(),
        operating_system: operating_system.unwrap_or(vec![
            OperatingSystem::Linux,
            OperatingSystem::MacOS,
            OperatingSystem::Windows,
        ]),
        tool_set: None,
    };

    let env = generate_execution_environment(
        db.clone(),
        llm_provider.clone(),
        app_id.clone(),
        tool_id.clone(),
        agent_id.clone(),
        // TODO: Update this value for runtime tool execution
        "code-execution".to_string(),
        "".to_string(),
        &oauth,
    )
    .await?;

    check_tool(
        "code-execution".to_string(),
        tool.config.clone(),
        parameters.clone(),
        tool.input_args.clone(),
        &oauth,
    )?;

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

    match tool
        .run_on_demand(
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
        )
        .await
    {
        Ok(run_result) => Ok(run_result.data),
        Err(e) => Err(e),
    }
}

pub async fn check_deno_tool(
    tool_id: String,
    app_id: String,
    support_files: HashMap<String, String>,
    code: String,
) -> Result<Vec<String>, ToolError> {
    let tool_router_key = ToolRouterKey::new(
        "local".to_string(),
        "@@system.shinkai".to_string(),
        "deno_runtime".to_string(),
        None,
    );

    // Create a minimal DenoTool instance
    let tool = DenoTool {
        name: "deno_runtime".to_string(),
        tool_router_key: Some(tool_router_key.clone()),
        homepage: None,
        author: "@@system.shinkai".to_string(),
        version: "1.0".to_string(),
        mcp_enabled: Some(false),
        js_code: code,
        tools: vec![],
        config: vec![],
        oauth: None,
        description: "Deno runtime execution".to_string(),
        keywords: vec![],
        input_args: Parameters::new(),
        output_arg: ToolOutputArg { json: "".to_string() },
        activated: true,
        embedding: None,
        result: ToolResult::new("object".to_string(), Value::Null, vec![]),
        sql_tables: None,
        sql_queries: None,
        file_inbox: None,
        assets: None,
        runner: RunnerType::Any,
        operating_system: vec![OperatingSystem::Linux, OperatingSystem::MacOS, OperatingSystem::Windows],
        tool_set: None,
    };

    let node_env = fetch_node_environment();
    let node_storage_path = node_env
        .node_storage_path
        .clone()
        .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;

    tool.check(
        node_env.api_listen_address.ip().to_string(),
        node_env.api_listen_address.port(),
        support_files,
        node_storage_path,
        app_id.clone(),
        tool_id.clone(),
    )
    .await
}
