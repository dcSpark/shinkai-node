use std::collections::HashMap;

use serde_json::{Map, Value};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_tools_primitives::tools::{
    argument::ToolOutputArg, deno_tools::ToolResult, error::ToolError, python_tools::PythonTool,
    tool_config::ToolConfig,
};

use crate::utils::environment::fetch_node_environment;

pub fn execute_python_tool(
    bearer: String,
    node_name: ShinkaiName,
    parameters: Map<String, Value>,
    extra_config: Vec<ToolConfig>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    support_files: HashMap<String, String>,
    code: String,
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
        input_args: vec![],
        output_arg: ToolOutputArg { json: "".to_string() },
        activated: true,
        embedding: None,
        result: ToolResult::new("object".to_string(), Value::Null, vec![]),
        sql_tables: None,
        sql_queries: None,
        file_inbox: None,
        oauth: None,
    };

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
