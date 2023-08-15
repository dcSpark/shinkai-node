use crate::tools::error::ToolError;
use serde_json::json;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolArgument {
    pub name: String,
    pub arg_type: String,
    pub description: String,
    pub is_optional: bool,
    pub ebnf: String,
}

impl ToolArgument {
    /// Parses a ToolArgument from a toolkit json
    pub fn from_toolkit_json(json: &JsonValue) -> Result<Self, ToolError> {
        let name = json["name"].as_str().ok_or(ToolError::ParseError("name".to_string()))?;
        let arg_type = json["type"].as_str().ok_or(ToolError::ParseError("type".to_string()))?;
        let description = json["description"]
            .as_str()
            .ok_or(ToolError::ParseError("description".to_string()))?;
        let is_optional = json["isOptional"]
            .as_bool()
            .ok_or(ToolError::ParseError("isOptional".to_string()))?;
        let ebnf = json["ebnf"].as_str().ok_or(ToolError::ParseError("ebnf".to_string()))?;

        Ok(Self {
            name: name.to_string(),
            arg_type: arg_type.to_string(),
            description: description.to_string(),
            is_optional,
            ebnf: ebnf.to_string(),
        })
    }

    /// Returns the ebnf definition with the name of the argument prepended
    /// properly in EBNF notation
    pub fn labled_ebnf(&self) -> String {
        format!("{} :== {}", self.name, self.ebnf)
    }
}
