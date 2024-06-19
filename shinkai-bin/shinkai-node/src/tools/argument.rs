use crate::tools::error::ToolError;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolArgument {
    pub name: String,
    pub arg_type: String,
    pub description: String,
    pub is_required: bool,
}

impl ToolArgument {
    /// Creates a new ToolArgument
    pub fn new(name: String, arg_type: String, description: String, is_required: bool) -> Self {
        Self {
            name,
            arg_type,
            description,
            is_required,
        }
    }

    /// Parses a ToolArgument from a toolkit json
    pub fn from_toolkit_json(json: &JsonValue) -> Result<Self, ToolError> {
        let name = json["name"].as_str().ok_or(ToolError::ParseError("name".to_string()))?;
        let arg_type = json["type"].as_str().ok_or(ToolError::ParseError("type".to_string()))?;
        let description = json["description"]
            .as_str()
            .ok_or(ToolError::ParseError("description".to_string()))?;
        let is_required = json["isRequired"]
            .as_bool()
            .ok_or(ToolError::ParseError("isRequired".to_string()))?;

        Ok(Self {
            name: name.to_string(),
            arg_type: arg_type.to_string(),
            description: description.to_string(),
            is_required,
        })
    }

    /// Converts a ToolArgument to a JSON structure
    pub fn to_toolkit_json(&self) -> JsonValue {
        serde_json::json!({
            "name": self.name,
            "type": self.arg_type,
            "description": self.description,
            "isRequired": self.is_required,
        })
    }
}
