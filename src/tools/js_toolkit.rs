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

impl InstalledJSToolkitMap {
    pub fn new() -> Self {
        Self {
            toolkits_info: HashMap::new(),
        }
    }

    /// DB Key For the Installed JS Toolkits Map
    pub fn db_key() -> String {
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
}

impl JSToolkitInfo {
    /// The DB Key where the corresponding whole JSToolkit is stored
    pub fn db_key(&self) -> String {
        JSToolkit::db_key_from_name(&self.name)
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
            activated: false,
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
}

impl JSToolkit {
    /// The DB Key where this JSToolkit is stored
    pub fn db_key(&self) -> String {
        Self::db_key_from_name(&self.name)
    }

    /// Given a toolkit name, generates the database key where the JSToolkit
    /// is stored in Topic::Toolkits
    pub fn db_key_from_name(js_toolkit_name: &str) -> String {
        let mut key = "js_toolkit".to_string();
        key.push_str(js_toolkit_name);
        key
    }

    pub fn from_toolkit_json(json: &str, js_code: &str) -> Result<Self, ToolError> {
        let parsed_json: JsonValue = serde_json::from_str(json)?;

        // Name parse
        let name = parsed_json["toolkit-name"]
            .as_str()
            .ok_or(ToolError::ParseError("toolkit-name".to_string()))?;

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
        let execution_setup_json = parsed_json["executionSetup"]
            .as_array()
            .ok_or(ToolError::ParseError("executionSetup".to_string()))?;
        let mut header_defs = Vec::new();
        for setup_json in execution_setup_json {
            let header_def = HeaderDefinition::from_toolkit_json(setup_json)?;
            header_defs.push(header_def);
        }

        Ok(Self {
            name: name.to_string(),
            js_code: js_code.to_string(),
            tools,
            header_definitions: header_defs,
            author: author.to_string(),
            version: version.to_string(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn default_toolkit_json() -> String {
        r#"{"toolkit-name":"Google Calendar Toolkit", "author":"Shinkai Team","version":"0.0.1","executionSetup":[{"name":"OAUTH","oauth":{"description":"","displayName":"Authentication","authUrl":"https://accounts.google.com/o/oauth2/auth","tokenUrl":"https://oauth2.googleapis.com/token","required":true,"pkce":true,"scope":["https://www.googleapis.com/auth/calendar.events","https://www.googleapis.com/auth/calendar.readonly"],"cloudOAuth":"activepieces"},"header":"x-shinkai-oauth"},{"name":"API_KEY","description":"Some Optional API Key","type":"STRING","isOptional":true,"header":"x-shinkai-api-key"},{"name":"API_SECRET","description":"Api Secret key","type":"STRING","header":"x-shinkai-api-secret"},{"name":"BASE_URL","description":"Base URL for api","type":"STRING","header":"x-shinkai-base-url"}],"tools":[{"name":"GoogleCalendarQuickEvent","description":"Activepieces Create Quick Event at Google Calendar","input":[{"name":"calendar_id","type":"STRING","description":"Primary calendar used if not specified","isOptional":true,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)?"},{"name":"text","type":"STRING","description":"The text describing the event to be created","isOptional":false,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)"},{"name":"send_updates","type":"ENUM","description":"Guests who should receive notifications about the creation of the new event.","isOptional":true,"wrapperType":"none","enum":["all","externalOnly","none"],"ebnf":"(\"all\" | \"externalOnly\" | \"none\")?"}],"output":[{"name":"response","type":"STRING","description":"Network Response","isOptional":false,"wrapperType":"none","ebnf":"([a-zA-Z0-9_]+)"}],"inputEBNF":"calendar_id ::= ([a-zA-Z0-9_]+)?\ntext ::= ([a-zA-Z0-9_]+)\nsend_updates ::= (\"all\" | \"externalOnly\" | \"none\")?\nresponse ::= ([a-zA-Z0-9_]+)"}]}"#.to_string()
    }

    #[test]
    fn test_js_toolkit_json_parsing() {
        let toolkit = JSToolkit::from_toolkit_json(&default_toolkit_json(), "").unwrap();

        assert_eq!(toolkit.name, "Google Calendar Toolkit");
        assert_eq!(
            toolkit.tools[0].ebnf_inputs(false).replace("\n", ""),
            r#"{"calendar_id": calendar_id, "text": text, "send_updates": send_updates, }calendar_id :== ([a-zA-Z0-9_]+)?text :== ([a-zA-Z0-9_]+)send_updates :== ("all" | "externalOnly" | "none")?"#
        );

        assert_eq!(toolkit.header_definitions.len(), 4);
        assert_eq!(toolkit.version, "0.0.1".to_string());
        assert_eq!(toolkit.author, "Shinkai Team".to_string());
    }
}
