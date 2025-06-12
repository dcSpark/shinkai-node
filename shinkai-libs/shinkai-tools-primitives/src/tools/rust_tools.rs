use std::fmt;

use shinkai_message_primitives::shinkai_utils::utils;

use crate::tools::error::ToolError;

use super::parameters::Parameters;
use super::shinkai_tool::ShinkaiToolHeader;
use super::tool_output_arg::ToolOutputArg;
use super::tool_playground::ToolPlaygroundMetadata;
use super::tool_types::ToolResult;

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
pub struct RustTool {
    pub name: String,
    pub description: String,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub tool_embedding: Option<Vec<f32>>,
    pub tool_router_key: String,
    pub mcp_enabled: Option<bool>,
}

impl RustTool {
    pub fn new(
        name: String,
        description: String,
        input_args: Parameters,
        output_arg: ToolOutputArg,
        tool_embedding: Option<Vec<f32>>,
        tool_router_key: String,
    ) -> Self {
        Self {
            name: utils::clean_string(&name),
            description,
            input_args,
            output_arg,
            tool_embedding,
            tool_router_key,
            mcp_enabled: Some(false),
        }
    }

    pub fn author(&self) -> String {
        "@@official.shinkai".to_string()
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

    pub fn from_shinkai_tool_header(header: &ShinkaiToolHeader) -> Result<Self, ToolError> {
        Ok(RustTool {
            name: header.name.clone(),
            description: header.description.clone(),
            input_args: header.input_args.clone(),
            output_arg: header.output_arg.clone(),
            tool_embedding: None, // Assuming no embedding is provided in the header
            tool_router_key: header.tool_router_key.clone(),
            mcp_enabled: header.mcp_enabled,
        })
    }

    pub fn get_metadata(&self) -> ToolPlaygroundMetadata {
        let (output_type, output_properties, output_required) = 
            match serde_json::from_str::<serde_json::Value>(&self.output_arg.json) {
                Ok(json_schema) => {
                    let r#type = json_schema.get("type").and_then(|v| v.as_str()).unwrap_or("object").to_string();
                    let properties = json_schema.get("properties").cloned().unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
                    let required_array = json_schema.get("required").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    let required_vec = required_array.into_iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>();
                    (r#type, properties, required_vec)
                }
                Err(_) => (
                    "object".to_string(), 
                    serde_json::Value::Object(serde_json::Map::new()), 
                    vec![]
                ),
            };

        let result = ToolResult::new(output_type, output_properties, output_required);

        ToolPlaygroundMetadata {
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            homepage: None,
            description: self.description.clone(),
            author: self.author(),
            keywords: vec![],
            configurations: vec![],
            parameters: self.input_args.clone(),
            result,
            sql_tables: vec![],
            sql_queries: vec![],
            tools: None,
            oauth: None,
            runner: super::tool_types::RunnerType::Any,
            operating_system: vec![],
            tool_set: None,
        }
    }
}
