use serde_json::{Map, Value};
use shinkai_tools_primitives::tools::argument::ToolArgument;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::js_tools::{JSTool, JSToolResult};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;

pub fn execute_deno_tool(
    tool_router_key: String,
    parameters: Map<String, Value>,
    extra_config: Option<String>,
) -> Result<Value, ToolError> {
    // Extract the JavaScript code from parameters
    let js_code = parameters
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::ExecutionError("Missing 'code' parameter".to_string()))?
        .to_string();

    let code = format!("{}", js_code);

    // Create a minimal JSTool instance
    let tool = JSTool {
        toolkit_name: "deno".to_string(),
        name: "deno_runtime".to_string(),
        author: "system".to_string(),
        js_code: code,
        config: vec![],
        description: "Deno runtime execution".to_string(),
        keywords: vec![],
        input_args: vec![],
        activated: true,
        embedding: None,
        result: JSToolResult::new("object".to_string(), Value::Null, vec![]),
    };

    // Create a new parameters map without the code parameter
    let mut execution_parameters = parameters.clone();
    execution_parameters.remove("code");

    // Run the tool and convert the RunResult to Value
    match tool.run(execution_parameters, extra_config) {
        Ok(run_result) => Ok(run_result.data),
        Err(e) => Err(e),
    }
}
