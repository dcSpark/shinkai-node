use std::fmt;

use crate::tools::error::ToolError;

use super::tool_output_arg::ToolOutputArg;
use super::parameters::Parameters;
use super::shinkai_tool::ShinkaiToolHeader;

#[derive(Debug)]
pub enum RustToolError {
    InvalidFunctionArguments(String),
    FailedJSONParsing,
}

impl fmt::Display for RustToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RustToolError::InvalidFunctionArguments(msg) => write!(f, "Invalid function arguments: {}", msg),
            RustToolError::FailedJSONParsing => write!(f, "Failed to parse JSON"),
        }
    }
}

impl std::error::Error for RustToolError {}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RustTool {
    pub name: String,
    pub description: String,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub tool_embedding: Option<Vec<f32>>,
    pub tool_router_key: String,
}

impl RustTool {
    pub fn new(
        name: String,
        description: String,
        input_args: Parameters,
        output_arg: ToolOutputArg,
        tool_embedding: Option<Vec<f32>>,
        tool_router_key: String,
    ) -> Self {
        Self {
            name: VRPath::clean_string(&name),
            description,
            input_args,
            output_arg,
            tool_embedding,
            tool_router_key,
        }
    }

    /// Default name of the rust toolkit
    pub fn toolkit_name(&self) -> String {
        "rust_toolkit".to_string()
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }

    pub fn from_shinkai_tool_header(header: &ShinkaiToolHeader) -> Result<Self, ToolError> {
        // Parse the tool_router_key to ensure it contains "rust_toolkit"
        let parts: Vec<&str> = header.tool_router_key.split(":::").collect();
        if parts.len() != 3 || parts[1] != "rust_toolkit" {
            return Err(ToolError::InvalidFunctionArguments(
                "Invalid tool_router_key format or missing 'rust_toolkit'".to_string(),
            ));
        }

        Ok(RustTool {
            name: header.name.clone(),
            description: header.description.clone(),
            input_args: header.input_args.clone(),
            output_arg: header.output_arg.clone(),
            tool_embedding: None, // Assuming no embedding is provided in the header
            tool_router_key: header.tool_router_key.clone(),
        })
    }
}
