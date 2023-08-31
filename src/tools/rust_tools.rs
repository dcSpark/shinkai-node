use crate::resources::embeddings::Embedding;
use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use lazy_static::lazy_static;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

lazy_static! {
    static ref RUST_TOOLS: Vec<RustTool> = vec![
        RustTool::new(
            "Message Sender".to_string(),
            "This is a tool for sending messages".to_string(),
            vec![],
            vec![],
            Embedding::new("", vec![]),
        ),
        RustTool::new(
            "Vector Search".to_string(),
            "This is a tool for searching vectors".to_string(),
            vec![],
            vec![],
            Embedding::new("", vec![]),
        ),
    ];
    pub static ref RUST_TOOLKIT: RustToolkit = {
        let mut map = HashMap::new();
        for tool in RUST_TOOLS.iter() {
            map.insert(tool.name.clone(), tool.clone());
        }
        RustToolkit { rust_tool_map: map }
    };
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RustToolkit {
    pub rust_tool_map: HashMap<String, RustTool>,
}

impl RustToolkit {
    pub fn get_tool(&self, name: &str) -> Result<&RustTool, ToolError> {
        self.rust_tool_map
            .get(name)
            .ok_or(ToolError::ToolNotFound(name.to_string()))
    }

    pub fn toolkit_name() -> String {
        "rust-toolkit".to_string()
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RustTool {
    pub name: String,
    pub description: String,
    pub input_args: Vec<ToolArgument>,
    pub output_args: Vec<ToolArgument>,
    pub tool_embedding: Embedding,
}

impl RustTool {
    pub fn new(
        name: String,
        description: String,
        input_args: Vec<ToolArgument>,
        output_args: Vec<ToolArgument>,
        tool_embedding: Embedding,
    ) -> Self {
        Self {
            name,
            description,
            input_args,
            output_args,
            tool_embedding,
        }
    }

    /// Default name of the rust toolkit
    pub fn toolkit_name(&self) -> String {
        RustToolkit::toolkit_name()
    }

    // Default name of the rust toolkit
    pub fn ebnf_inputs(&self, add_arg_descriptions: bool) -> String {
        RustToolkit::toolkit_name()
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
}

/// TODO: Implement Rust Tool Execution logic on the executor .
/// This is needed because we can't serialize functions into JSON, and so
/// the ToolRouter won't work otherwise.
pub struct RustToolExecutor {}
// pub run_tool: Box<dyn Fn(JsonValue) -> Result<JsonValue, ToolError> + Send + Sync>,
