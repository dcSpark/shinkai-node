use crate::tools::argument::ToolArgument;
use crate::tools::error::ToolError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value as JsonValue;
use shinkai_vector_resources::embeddings::Embedding;

use super::argument::ToolOutputArg;
use super::tool_config::ToolConfig;
use super::tool_playground::{SqlQuery, SqlTable};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PythonTool {
    pub toolkit_name: String,
    pub name: String,
    pub author: String,
    pub py_code: String,
    pub tools: Option<Vec<String>>,
    pub config: Vec<ToolConfig>,
    pub description: String,
    pub keywords: Vec<String>,
    pub input_args: Vec<ToolArgument>,
    pub output_arg: ToolOutputArg,
    pub activated: bool,
    pub embedding: Option<Embedding>,
    pub result: PythonToolResult,
    pub sql_tables: Option<Vec<SqlTable>>,
    pub sql_queries: Option<Vec<SqlQuery>>,
    pub file_inbox: Option<String>,
}

impl PythonTool {
    /// Default name of the rust toolkit
    pub fn toolkit_name(&self) -> String {
        "python-toolkit".to_string()
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

#[derive(Debug, Clone, PartialEq)]
pub struct PythonToolResult {
    pub r#type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}

impl Serialize for PythonToolResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper = Helper {
            result_type: self.r#type.clone(),
            properties: self.properties.clone(),
            required: self.required.clone(),
        };

        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PythonToolResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = Helper::deserialize(deserializer)?;

        Ok(PythonToolResult {
            r#type: helper.result_type,
            properties: helper.properties,
            required: helper.required,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Helper {
    #[serde(rename = "type", alias = "result_type")]
    result_type: String,
    properties: JsonValue,
    required: Vec<String>,
}

impl PythonToolResult {
    pub fn new(result_type: String, properties: serde_json::Value, required: Vec<String>) -> Self {
        PythonToolResult {
            r#type: result_type,
            properties,
            required,
        }
    }
}
