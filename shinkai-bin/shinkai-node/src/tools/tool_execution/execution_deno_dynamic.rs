use serde_json::{Map, Value};
use shinkai_tools_primitives::tools::argument::ToolOutputArg;
use shinkai_tools_primitives::tools::deno_tools::DenoTool;
use shinkai_tools_primitives::tools::deno_tools::DenoToolResult;
use shinkai_tools_primitives::tools::error::ToolError;

pub fn execute_deno_tool(
    bearer: String,
    parameters: Map<String, Value>,
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

    // Run the tool and convert the RunResult to Value
    match tool.run_on_demand(bearer, header_code, parameters, extra_config) {
        Ok(run_result) => Ok(run_result.data),
        Err(e) => Err(e),
    }
}
