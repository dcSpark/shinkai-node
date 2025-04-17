use crate::tools::error::ToolError;
use crate::tools::parameters::Parameters;
use crate::tools::tool_output_arg::ToolOutputArg;
use std::fmt;

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
pub struct AgentToolWrapper {
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub embedding: Option<Vec<f32>>,
    pub mcp_enabled: Option<bool>,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
}

impl AgentToolWrapper {
    pub fn new(
        agent_id: String,
        name: String,
        description: String,
        author: String,
        embedding: Option<Vec<f32>>,
    ) -> Self {
        Self {
            agent_id,
            name,
            description,
            author,
            embedding,
            mcp_enabled: Some(false),
            input_args: default_input_args(),
            output_arg: default_output_arg(),
        }
    }

    /// Returns the input arguments of the tool
    pub fn input_args(&self) -> Parameters {
        self.input_args.clone()
    }

    /// Returns the output argument of the tool
    pub fn output_arg(&self) -> ToolOutputArg {
        self.output_arg.clone()
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

fn default_input_args() -> Parameters {
    let mut params = Parameters::new();

    // Add content parameter (required but can be empty)
    params.add_property(
        "prompt".to_string(),
        "string".to_string(),
        "Message to the agent".to_string(),
        true,
    );

    // TODO: add later
    // // Add images parameter (optional string array)
    // let string_property = crate::tools::parameters::Property::new("string".to_string(), "Image URL".to_string());
    // params.properties.insert(
    //     "images".to_string(),
    //     crate::tools::parameters::Property::with_array_items("Array of image URLs".to_string(), string_property),
    // );

    // Add session_id parameter (not required)
    params.add_property(
        "session_id".to_string(),
        "string".to_string(),
        "Session identifier".to_string(),
        false,
    );

    params
}

fn default_output_arg() -> ToolOutputArg {
    ToolOutputArg {
        json: r#"{"type":"string","description":"Agent response"}"#.to_string(),
    }
}
