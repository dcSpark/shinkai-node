use std::collections::HashMap;

use serde_json::{Map, Value};
use shinkai_tools_primitives::tools::argument::ToolOutputArg;
use shinkai_tools_primitives::tools::deno_tools::DenoTool;
use shinkai_tools_primitives::tools::deno_tools::DenoToolResult;
use shinkai_tools_primitives::tools::error::ToolError;

use crate::utils::environment::fetch_node_environment;

pub fn execute_deno_tool(
    bearer: String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    extra_config: Option<String>,
    header_code: String,
    code: String,
) -> Result<Value, ToolError> {
    // Create a minimal DenoTool instance
    let tool = DenoTool {
        toolkit_name: "deno".to_string(),
        name: "deno_runtime".to_string(),
        author: "system".to_string(),
        js_code: code,
        config: vec![],
        description: "Deno runtime execution".to_string(),
        keywords: vec![],
        input_args: vec![],
        output_arg: ToolOutputArg { json: "".to_string() },
        activated: true,
        embedding: None,
        result: DenoToolResult::new("object".to_string(), Value::Null, vec![]),
    };

    let mut envs = HashMap::new();
    envs.insert("BEARER".to_string(), bearer);
    envs.insert("X_SHINKAI_TOOL_ID".to_string(), tool_id.clone());
    envs.insert("X_SHINKAI_APP_ID".to_string(), app_id.clone());

    let node_env = fetch_node_environment();
    let node_storage_path = node_env
        .node_storage_path
        .clone()
        .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;

    match tool.run_on_demand(
        envs,
        header_code,
        parameters,
        extra_config,
        node_storage_path,
        app_id.clone(),
        tool_id.clone(),
        false,
    ) {
        Ok(run_result) => Ok(run_result.data),
        Err(e) => Err(e),
    }
}
