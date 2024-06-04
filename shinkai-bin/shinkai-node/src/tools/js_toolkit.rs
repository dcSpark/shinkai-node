use crate::tools::error::ToolError;
use crate::tools::js_toolkit_headers::HeaderDefinition;
use crate::tools::js_tools::JSTool;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// A hashmap that holds the toolkit infos for all installed `JSToolKit`s
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledJSToolkitMap {
    toolkits_info: HashMap<String, JSToolkitInfo>,
}

impl Default for InstalledJSToolkitMap {
    fn default() -> Self {
        Self::new()
    }
}

impl InstalledJSToolkitMap {
    pub fn new() -> Self {
        Self {
            toolkits_info: HashMap::new(),
        }
    }

    /// Updates the headers_set field for a given toolkit
    pub fn update_headers_set(&mut self, toolkit_name: &str, headers_set: bool) -> Result<(), ToolError> {
        let toolkit_info = self
            .toolkits_info
            .get_mut(toolkit_name)
            .ok_or(ToolError::ToolkitNotFound)?;

        toolkit_info.headers_set = headers_set;

        Ok(())
    }

    /// Sets a given toolkit to active state
    pub fn activate_toolkit(&mut self, toolkit_name: &str) -> Result<(), ToolError> {
        let toolkit_info = self
            .toolkits_info
            .get_mut(toolkit_name)
            .ok_or(ToolError::ToolkitNotFound)?;

        toolkit_info.activated = true;

        Ok(())
    }

    /// Sets a given toolkit to deactivated state
    pub fn deactivate_toolkit(&mut self, toolkit_name: &str) -> Result<(), ToolError> {
        let toolkit_info = self
            .toolkits_info
            .get_mut(toolkit_name)
            .ok_or(ToolError::ToolkitNotFound)?;

        toolkit_info.activated = false;

        Ok(())
    }

    /// DB Key For the Installed JS Toolkits Map
    pub fn shinkai_db_key() -> String {
        "installed_js_toolkit_map".to_string()
    }

    pub fn add_toolkit_info(&mut self, js_toolkit_info: &JSToolkitInfo) {
        self.toolkits_info
            .insert(js_toolkit_info.name.clone(), js_toolkit_info.clone());
    }

    pub fn get_toolkit_info(&self, name: &str) -> Result<&JSToolkitInfo, ToolError> {
        self.toolkits_info.get(name).ok_or(ToolError::ToolkitNotFound)
    }

    pub fn remove_toolkit_info(&mut self, name: &str) -> Result<(), ToolError> {
        self.toolkits_info.remove(name).ok_or(ToolError::ToolkitNotFound)?;
        Ok(())
    }

    pub fn get_all_toolkit_infos(&self) -> Vec<&JSToolkitInfo> {
        self.toolkits_info.values().collect()
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

/// A basic struct that holds information about an installed JSToolkit
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JSToolkitInfo {
    pub name: String,
    pub author: String,
    pub version: String,
    pub activated: bool,
    pub headers_set: bool,
}

impl JSToolkitInfo {
    /// The DB Key where the corresponding whole JSToolkit is stored
    pub fn shinkai_db_key(&self) -> String {
        JSToolkit::shinkai_db_key_from_name(&self.name)
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

impl From<&JSToolkit> for JSToolkitInfo {
    fn from(toolkit: &JSToolkit) -> Self {
        Self {
            name: toolkit.name.clone(),
            author: toolkit.author.clone(),
            version: toolkit.version.clone(),
            activated: toolkit.activated,
            headers_set: toolkit.headers_set,
        }
    }
}

/// A JS Toolkit with the packed JS code and tool/header definitions.
/// Of note, to use a tool within a JSToolkit the actual header values need
/// to be fetched from the DB, as they are stored separately (due to header
/// initialization being after the toolkit itself gets installed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JSToolkit {
    pub name: String,
    pub js_code: String,
    pub tools: Vec<JSTool>,
    pub header_definitions: Vec<HeaderDefinition>,
    pub author: String,
    pub version: String,
    activated: bool,
    headers_set: bool,
}

impl JSToolkit {
    /// The DB Key where this JSToolkit is stored
    pub fn shinkai_db_key(&self) -> String {
        Self::shinkai_db_key_from_name(&self.name)
    }

    // Returns activated bool
    pub fn activated(&self) -> bool {
        self.activated
    }

    // Returns headers_set bool
    pub fn headers_set(&self) -> bool {
        self.headers_set
    }

    /// Given a toolkit name, generates the database key where the JSToolkit
    /// is stored in Topic::Toolkits
    pub fn shinkai_db_key_from_name(js_toolkit_name: &str) -> String {
        let mut key = "js_toolkit".to_string();
        key.push_str(js_toolkit_name);
        key
    }

    /// Given a toolkit definition json, create a JSToolkit
    pub fn from_toolkit_json(parsed_json: &JsonValue, js_code: &str) -> Result<Self, ToolError> {
        // Name parse
        let name = parsed_json["toolkitName"]
            .as_str()
            .ok_or(ToolError::ParseError("toolkitName".to_string()))?;

        // Author parse
        let author = parsed_json["author"]
            .as_str()
            .ok_or(ToolError::ParseError("author".to_string()))?;

        // Version parse
        let version = parsed_json["version"]
            .as_str()
            .ok_or(ToolError::ParseError("version".to_string()))?;

        // Tools parse
        let tools_json = parsed_json["tools"]
            .as_array()
            .ok_or(ToolError::ParseError("tools".to_string()))?;
        let mut tools = Vec::new();
        for tool_json in tools_json {
            let tool = JSTool::from_toolkit_json(name, tool_json)?;
            tools.push(tool);
        }

        // Header defs parsing
        let execution_setup_json = &parsed_json["toolkitHeaders"];
        let mut header_defs = Vec::new();
        if let Some(array) = execution_setup_json.as_array() {
            for setup_json in array {
                let header_def = HeaderDefinition::from_toolkit_json(setup_json)?;
                header_defs.push(header_def);
            }
        } else if execution_setup_json.is_object() && execution_setup_json.as_object().unwrap().is_empty() {
            // If it's an empty object, do nothing as header_defs is already an empty vector
        } else {
            return Err(ToolError::ParseError("toolkitHeaders".to_string()));
        }

        Ok(Self {
            name: name.to_string(),
            js_code: js_code.to_string(),
            tools,
            header_definitions: header_defs,
            author: author.to_string(),
            version: version.to_string(),
            activated: false,
            headers_set: false,
        })
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
