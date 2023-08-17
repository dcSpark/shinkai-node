use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use lazy_static::lazy_static;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

// Instantiating a global hardcoded RUST_TOOLKIT
lazy_static! {
    static ref RUST_TOOLS: Vec<Arc<dyn RustTool>> =
        vec![Arc::new(MessageSenderTool {}), Arc::new(VectorSearchTool {}),];
    pub static ref RUST_TOOLKIT: RustToolkit = {
        let mut map = HashMap::new();
        for tool in RUST_TOOLS.iter() {
            map.insert(tool.name(), Arc::clone(tool));
        }
        RustToolkit { rust_tool_map: map }
    };
}

pub struct RustToolkit {
    pub rust_tool_map: HashMap<String, Arc<dyn RustTool>>,
}

impl RustToolkit {
    pub fn get_tool(&self, name: &str) -> Option<&Arc<dyn RustTool>> {
        self.rust_tool_map.get(name)
    }
}

pub trait RustTool: Sync + Send {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn run(&self, input_json: JsonValue) -> Result<(), ToolError>;
    fn input_args(&self) -> Vec<ToolArgument>;
    fn output_args(&self) -> Vec<ToolArgument>;
}
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MessageSenderTool {}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorSearchTool {}

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
