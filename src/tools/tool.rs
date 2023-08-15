use crate::tools::argument::ToolArgument;
use crate::tools::auth::ToolAuth;
use crate::tools::error::ToolError;
use serde_json::Value as JsonValue;

pub enum Tool {
    JSTool(JSTool),
    RustTool(Box<dyn RustTool>),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSTool {
    pub toolkit_name: String,
    pub name: String,
    pub description: String,
    pub input_args: Vec<ToolArgument>,
    pub output_args: Vec<ToolArgument>,
    pub auth: Option<ToolAuth>,
}

impl JSTool {
    fn run(&self, _input_json: JsonValue) -> Result<(), ToolError> {
        // Implement the functionality here
        Ok(())
    }

    /// Parses a JSTool from a toolkit json
    pub fn from_toolkit_json(toolkit_name: &str, json: &JsonValue) -> Result<Self, ToolError> {
        let name = json["name"].as_str().ok_or(ToolError::ParseError("name".to_string()))?;
        let description = json["description"]
            .as_str()
            .ok_or(ToolError::ParseError("description".to_string()))?;

        let input_args_json = json["input"]
            .as_array()
            .ok_or(ToolError::ParseError("input".to_string()))?;
        let mut input_args = Vec::new();
        for arg in input_args_json {
            let tool_arg = ToolArgument::from_toolkit_json(arg)?;
            input_args.push(tool_arg);
        }

        let output_args_json = json["output"]
            .as_array()
            .ok_or(ToolError::ParseError("output".to_string()))?;
        let mut output_args = Vec::new();
        for arg in output_args_json {
            let tool_arg = ToolArgument::from_toolkit_json(arg)?;
            output_args.push(tool_arg);
        }

        Ok(Self {
            toolkit_name: toolkit_name.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            input_args,
            output_args,
            auth: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MessageSenderTool {}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorSearchTool {}

pub trait RustTool {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn run(&self, input_json: JsonValue) -> Result<(), ToolError>;
    fn input_args(&self) -> Vec<ToolArgument>;
    fn output_args(&self) -> Vec<ToolArgument>;
}

impl RustTool for MessageSenderTool {
    fn name(&self) -> String {
        "Message Sender".to_string()
    }

    fn description(&self) -> String {
        "This is a tool for sending messages".to_string()
    }

    fn run(&self, _input_json: JsonValue) -> Result<(), ToolError> {
        // Implement the functionality here
        Ok(())
    }

    fn input_args(&self) -> Vec<ToolArgument> {
        // Implement the functionality here
        vec![]
    }

    fn output_args(&self) -> Vec<ToolArgument> {
        // Implement the functionality here
        vec![]
    }
}

impl RustTool for VectorSearchTool {
    fn name(&self) -> String {
        "Vector Search".to_string()
    }

    fn description(&self) -> String {
        "This is a tool for searching vectors".to_string()
    }

    fn run(&self, _input_json: JsonValue) -> Result<(), ToolError> {
        // Implement the functionality here
        Ok(())
    }

    fn input_args(&self) -> Vec<ToolArgument> {
        // Implement the functionality here
        vec![]
    }

    fn output_args(&self) -> Vec<ToolArgument> {
        // Implement the functionality here
        vec![]
    }
}
