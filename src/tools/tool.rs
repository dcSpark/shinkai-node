use crate::tools::argument::ToolArgument;
use crate::tools::auth::ToolAuth;
use crate::tools::error::ToolError;
use serde_json::Value as JsonValue;

pub enum Tool {
    JSTool(JSTool),
    RustTool(Box<dyn RustTool>),
}

pub struct JSTool {
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

}

pub struct MessageSenderTool {}

pub struct VectorSearchTool {}

pub trait RustTool {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn run(&self, input_json: JsonValue) -> Result<(), ToolError>;
    fn input_args(&self) -> Vec<ToolArgument>;
    fn output_args(&self) -> Vec<ToolArgument>;
    fn auth(&self) -> Option<ToolAuth>;
}

impl RustTool for MessageSenderTool {
    fn name(&self) -> String {
        "MessageSenderTool".to_string()
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

    fn auth(&self) -> Option<ToolAuth> {
        // Implement the functionality here
        None
    }
}

impl RustTool for VectorSearchTool {
    fn name(&self) -> String {
        "VectorSearchTool".to_string()
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

    fn auth(&self) -> Option<ToolAuth> {
        // Implement the functionality here
        None
    }
}
