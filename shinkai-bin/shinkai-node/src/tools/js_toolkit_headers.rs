use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolConfig {
    OAuth(OAuth),
    GenericHeader(GenericHeader),
    BasicConfig(BasicConfig),
}

impl ToolConfig {
    /// User-facing name of the header. To be used by frontend with input box
    /// when user is required to input header values
    pub fn name(&self) -> String {
        match self {
            ToolConfig::OAuth(oauth) => oauth.name.clone(),
            ToolConfig::GenericHeader(header) => header.name.clone(),
            ToolConfig::BasicConfig(config) => config.key_name.clone(),
        }
    }

    /// Description of the header, to be used in frontend
    pub fn description(&self) -> String {
        match self {
            ToolConfig::OAuth(oauth) => oauth.description.clone(),
            ToolConfig::GenericHeader(header) => header.description.clone(),
            ToolConfig::BasicConfig(config) => config.description.clone(),
        }
    }

    /// The header key to be used when making the request
    pub fn header(&self) -> String {
        match self {
            ToolConfig::OAuth(oauth) => oauth.header.clone(),
            ToolConfig::GenericHeader(header) => header.header.clone(),
            ToolConfig::BasicConfig(config) => config.key_value.clone().unwrap_or_default(),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasicConfig {
    pub key_name: String,
    pub description: String,
    pub required: bool,
    pub key_value: Option<String>,
}