use std::collections::HashMap;

use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_tools_primitives::tools::deno_tools::{DenoTool, ToolResult};
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::tool_config::{OAuth, ToolConfig};
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;

use super::execution_coordinator::handle_oauth;
use crate::utils::environment::fetch_node_environment;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub async fn execute_deno_tool(
    bearer: String,
    db: Arc<RwLock<SqliteManager>>,
    node_name: ShinkaiName,
    parameters: Map<String, Value>,
    extra_config: Vec<ToolConfig>,
    oauth: Option<Vec<OAuth>>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    support_files: HashMap<String, String>,
    code: String,
) -> Result<Value, ToolError> {
    // Create a minimal DenoTool instance
    let tool = DenoTool {
        toolkit_name: "deno".to_string(),
        name: "deno_runtime".to_string(),
        author: "system".to_string(),
        js_code: code,
        tools: None,
        config: vec![],
        oauth: oauth.clone(),
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
    };

    let mut envs = HashMap::new();
    envs.insert("BEARER".to_string(), bearer);
    envs.insert("X_SHINKAI_TOOL_ID".to_string(), tool_id.clone());
    envs.insert("X_SHINKAI_APP_ID".to_string(), app_id.clone());
    envs.insert("X_SHINKAI_INSTANCE_ID".to_string(), "".to_string()); // TODO Pass data from the API
    envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), llm_provider.clone());

    let oauth = handle_oauth(
        &oauth.clone(),
        &db,
        app_id.clone(),
        tool_id.clone(),
        "code-execution".to_string(),
    )
    .await?;
    envs.insert("SHINKAI_OAUTH".to_string(), oauth.to_string());

    let node_env = fetch_node_environment();
    let node_storage_path = node_env
        .node_storage_path
        .clone()
        .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;

    match tool.run_on_demand(
        envs,
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
    ) {
        Ok(run_result) => Ok(run_result.data),
        Err(e) => Err(e),
    }
}

pub fn check_deno_tool(
    tool_id: String,
    app_id: String,
    support_files: HashMap<String, String>,
    code: String,
) -> Result<Vec<String>, ToolError> {
    // Create a minimal DenoTool instance
    let tool = DenoTool {
        toolkit_name: "deno".to_string(),
        name: "deno_runtime".to_string(),
        author: "system".to_string(),
        js_code: code,
        tools: None,
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
}
