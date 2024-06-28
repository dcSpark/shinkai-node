use crate::tools::error::ToolError;
use crate::tools::js_tools::JSTool;
use serde::{Deserialize, Serialize};

/// A JSToolkit is a collection of JSTools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JSToolkit {
    pub name: String,
    pub tools: Vec<JSTool>,
    pub author: String,
    pub version: String,
}

impl JSToolkit {
    /// Creates a new JSToolkit with the provided name and js_code, and default values for other fields.
    pub fn new_semi_dummy_with_defaults(name: &str, js_code: &str) -> Self {
        Self {
            name: name.to_string(),
            tools: vec![JSTool {
                toolkit_name: name.to_string(),
                name: name.to_string(),
                author: "Dummy author".to_string(), // Dummy author
                config: vec![], // Empty headers
                js_code: js_code.to_string(),
                description: "Dummy description".to_string(), // Dummy description
                input_args: vec![],                           // Empty arguments
                activated: false,
                config_set: true,
                embedding: None,
            }],
            author: "Dummy author".to_string(), // Dummy author
            version: "1.0.0".to_string(),       // Dummy version
        }
    }

    /// Generate the key that this toolkit will be stored under in the tool router
    pub fn gen_router_key(name: &str, author: &str) -> String {
        // We replace any `/` in order to not have the names break VRPaths
        format!("{}:::{}", author, name).replace('/', "|")
    }

    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }
}
