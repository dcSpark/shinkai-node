use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JSTool {
    pub toolkit_name: String,
    pub name: String,
    pub description: String,
    pub input_args: Vec<ToolArgument>,
}

impl JSTool {
    pub fn run(&self, _input_json: JsonValue) -> Result<(), ToolError> {
        // Implement the functionality here
        Ok(())
    }

    /// Convert to JSON string
    pub fn to_json_string(&self) -> Result<String, ToolError> {
        let json_value = self.to_json()?;
        serde_json::to_string(&json_value).map_err(|e| ToolError::SerializationError(e.to_string()))
    }

    /// Convert from JSON string
    pub fn from_json_string(json_str: &str) -> Result<Self, ToolError> {
        let json_value: JsonValue = serde_json::from_str(json_str).map_err(|e| ToolError::ParseError(e.to_string()))?;
        Self::from_json(&json_value)
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<JsonValue, ToolError> {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for arg in &self.input_args {
            properties.insert(
                arg.name.clone(),
                serde_json::json!({
                    "type": arg.arg_type,
                    "description": arg.description,
                }),
            );
            if arg.is_required {
                required.push(arg.name.clone());
            }
        }

        let json_value = serde_json::json!({
            "name": self.name,
            "description": self.description,
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": required,
            },
        });

        Ok(json_value)
    }

    /// Convert from json
    pub fn from_json(json: &JsonValue) -> Result<Self, ToolError> {
        let name = json["name"].as_str().ok_or(ToolError::ParseError("name".to_string()))?;
        let description = json["description"]
            .as_str()
            .ok_or(ToolError::ParseError("description".to_string()))?;
        let parameters = json["parameters"]
            .as_object()
            .ok_or(ToolError::ParseError("parameters".to_string()))?;
        let properties = parameters["properties"]
            .as_object()
            .ok_or(ToolError::ParseError("properties".to_string()))?;
        let required = parameters["required"]
            .as_array()
            .ok_or(ToolError::ParseError("required".to_string()))?;

        let mut input_args = Vec::new();
        for (name, prop) in properties {
            let arg_type = prop["type"].as_str().ok_or(ToolError::ParseError("type".to_string()))?;
            let description = prop["description"]
                .as_str()
                .ok_or(ToolError::ParseError("description".to_string()))?;
            let is_required = required.iter().any(|r| r.as_str() == Some(name));

            input_args.push(ToolArgument {
                name: name.clone(),
                arg_type: arg_type.to_string(),
                description: description.to_string(),
                is_required,
            });
        }

        Ok(Self {
            toolkit_name: "".to_string(), // Assuming toolkit_name is not part of the JSON structure
            name: name.to_string(),
            description: description.to_string(),
            input_args,
        })
    }

    /// Convert from toolkit JSON
    pub fn from_toolkit_json(toolkit_name: &str, json: &JsonValue) -> Result<Self, ToolError> {
        let mut tool = Self::from_json(json)?;
        tool.toolkit_name = toolkit_name.to_string();
        Ok(tool)
    }
}
