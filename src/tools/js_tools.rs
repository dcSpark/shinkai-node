use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSTool {
    pub toolkit_name: String,
    pub name: String,
    pub description: String,
    pub input_args: Vec<ToolArgument>,
    pub output_args: Vec<ToolArgument>,
}

impl JSTool {
    pub fn run(&self, _input_json: JsonValue) -> Result<(), ToolError> {
        // Implement the functionality here
        Ok(())
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
        })
    }
}
