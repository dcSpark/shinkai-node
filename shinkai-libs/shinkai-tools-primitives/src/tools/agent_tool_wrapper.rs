use crate::tools::error::ToolError;
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
        }
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
