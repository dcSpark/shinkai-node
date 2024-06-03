use crate::tools::error::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HeaderDefinition {
    OAuth(OAuth),
    GenericHeader(GenericHeader),
}

impl HeaderDefinition {
    /// User-facing name of the header. To be used by frontend with input box
    /// when user is required to input header values
    pub fn name(&self) -> String {
        match self {
            HeaderDefinition::OAuth(oauth) => oauth.name.clone(),
            HeaderDefinition::GenericHeader(header) => header.name.clone(),
        }
    }

    /// Description of the header, to be used in frontend
    pub fn description(&self) -> String {
        match self {
            HeaderDefinition::OAuth(oauth) => oauth.description.clone(),
            HeaderDefinition::GenericHeader(header) => header.description.clone(),
        }
    }

    /// The header key to be used when making the request
    pub fn header(&self) -> String {
        match self {
            HeaderDefinition::OAuth(oauth) => oauth.header.clone(),
            HeaderDefinition::GenericHeader(header) => header.header.clone(),
        }
    }

    /// Generates the shinkai_db_key that this header is stored at for the given toolkit_name
    pub fn shinkai_db_key(&self, toolkit_name: &str) -> String {
        format!("{}:::{}", self.header(), toolkit_name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenericHeader {
    name: String,
    description: String,
    header_datatype: String,
    required: bool,
    header: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuth {
    name: String,
    description: String,
    display_name: String,
    auth_url: String,
    token_url: String,
    required: bool,
    pkce: bool,
    scope: Vec<String>,
    cloud_oauth_name: String, // Ie. Google OAuth App name
    header: String,
}

impl HeaderDefinition {
    pub fn from_toolkit_json(json: &JsonValue) -> Result<Self, ToolError> {
        // Check if the JSON object contains the "oauth" key
        if let Some(oauth_details) = json.get("oauth") {
            let name = json
                .get("name")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a name".to_string()))?;

            let header = json
                .get("header")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a header".to_string()))?;

            let description = oauth_details
                .get("description")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();

            let display_name = oauth_details
                .get("displayName")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a displayName".to_string()))?;

            let auth_url = oauth_details
                .get("authUrl")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected an authUrl".to_string()))?;

            let token_url = oauth_details
                .get("tokenUrl")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a tokenUrl".to_string()))?;

            let required = oauth_details
                .get("required")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);

            let pkce = oauth_details.get("pkce").and_then(JsonValue::as_bool).unwrap_or(false);

            let scope = oauth_details
                .get("scope")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|s| s.as_str()).map(String::from).collect())
                .unwrap_or_default();

            let cloud_oauth_name = oauth_details
                .get("cloudOAuth")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();

            let oauth = OAuth {
                name: name.to_string(),
                description: description.to_string(),
                display_name: display_name.to_string(),
                auth_url: auth_url.to_string(),
                token_url: token_url.to_string(),
                required,
                pkce,
                scope,
                cloud_oauth_name: cloud_oauth_name.to_string(),
                header: header.to_string(),
            };

            Ok(HeaderDefinition::OAuth(oauth))
        } else {
            let name = json
                .get("name")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a name".to_string()))?;

            let description = json
                .get("description")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a description".to_string()))?;

            let header_datatype = json
                .get("type")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a type".to_string()))?;

            let is_optional = json.get("isOptional").and_then(JsonValue::as_bool).unwrap_or(false);

            let header = json
                .get("header")
                .and_then(JsonValue::as_str)
                .ok_or(ToolError::ParseError("Expected a header".to_string()))?;

            let generic_header = GenericHeader {
                name: name.to_string(),
                description: description.to_string(),
                header_datatype: header_datatype.to_string(),
                required: !is_optional,
                header: header.to_string(),
            };

            Ok(HeaderDefinition::GenericHeader(generic_header))
        }
    }
}
