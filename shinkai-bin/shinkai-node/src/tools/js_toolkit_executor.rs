use crate::tools::error::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use shinkai_tools_runner::tools::tool::Tool;

/// The resulting data from execution a JS tool
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub tool: String,
    pub result: String,
}

pub struct JSToolkitExecutor;

impl JSToolkitExecutor {
    pub async fn new_local() -> Result<Self, ToolError> {
        Ok(JSToolkitExecutor)
    }

    /// Submits a request to the JS Toolkit Executor
    pub async fn submit_request(
        &self,
        tool_name: String,
        code: String,
        tool_arguments: JsonValue,
        fn_args: JsonValue,
    ) -> Result<ToolExecutionResult, ToolError> {
        let mut tool = Tool::new();
        tool.load_from_code(&code, &tool_arguments.to_string()).await.map_err(ToolError::from)?;
        let result = tool.run(&fn_args.to_string()).await.map_err(ToolError::from)?;
        Ok(ToolExecutionResult {
            tool: tool_name,
            result,
        })
    }
}
