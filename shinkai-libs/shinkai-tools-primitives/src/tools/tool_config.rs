use serde::{Deserialize, Serialize};
use serde_json::Value;

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

    /// Returns a sanitized copy of the ToolConfig by removing key-values from BasicConfig
    pub fn sanitize(&self) -> ToolConfig {
        match self {
            ToolConfig::OAuth(oauth) => ToolConfig::OAuth(oauth.clone()),
            ToolConfig::GenericHeader(header) => ToolConfig::GenericHeader(header.clone()),
            ToolConfig::BasicConfig(config) => ToolConfig::BasicConfig(BasicConfig {
                key_name: config.key_name.clone(),
                description: config.description.clone(),
                required: config.required,
                key_value: None,
            }),
        }
    }

    /// Creates a vector of ToolConfig::BasicConfig instances from a serde_json::Value
    pub fn basic_config_from_value(value: &Value) -> Vec<ToolConfig> {
        let mut configs = Vec::new();

        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                let key_value = val.as_str().map(String::from);

                let basic_config = BasicConfig {
                    key_name: key.clone(),
                    description: format!("Description for {}", key),
                    required: false, // Set default or determine from context
                    key_value,
                };
                configs.push(ToolConfig::BasicConfig(basic_config));
            }
        }

        configs
    }

    /// Attempts to deserialize a serde_json::Value into a ToolConfig
    pub fn from_value(value: &Value) -> Option<ToolConfig> {
        if let Some(obj) = value.as_object() {
            // Check for BasicConfig structure
            if let Some(key_name) = obj.get("key_name").and_then(|v| v.as_str()) {
                let description = obj.get("description").and_then(|v| v.as_str()).unwrap_or_default();
                let required = obj.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
                let key_value = obj.get("key_value").and_then(|v| v.as_str()).map(String::from);

                let basic_config = BasicConfig {
                    key_name: key_name.to_string(),
                    description: description.to_string(),
                    required,
                    key_value,
                };
                return Some(ToolConfig::BasicConfig(basic_config));
            }

            // Add similar checks for other ToolConfig variants like OAuth or GenericHeader if needed
        }

        None
    }

    /// Creates a vector of ToolConfig::OAuth instances from a serde_json::Value
    pub fn oauth_from_value(value: &Value) -> Vec<ToolConfig> {
        let mut configs = Vec::new();

        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                if let Some(oauth_obj) = val.as_object() {
                    let oauth = OAuth {
                        name: key.clone(),
                        description: oauth_obj.get("description").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        display_name: oauth_obj.get("display_name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        auth_url: oauth_obj.get("auth_url").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        token_url: oauth_obj.get("token_url").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        required: oauth_obj.get("required").and_then(|v| v.as_bool()).unwrap_or(false),
                        pkce: oauth_obj.get("pkce").and_then(|v| v.as_bool()).unwrap_or(false),
                        scope: oauth_obj.get("scope").and_then(|v| v.as_array()).map_or(Vec::new(), |arr| {
                            arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                        }),
                        cloud_oauth_name: oauth_obj.get("cloud_oauth_name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        header: oauth_obj.get("header").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    };
                    configs.push(ToolConfig::OAuth(oauth));
                }
            }
        }

        configs
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenericHeader {
    pub name: String,
    pub description: String,
    pub header_datatype: String,
    pub required: bool,
    pub header: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuth {
    pub name: String,
    pub description: String,
    pub display_name: String,
    pub auth_url: String,
    pub token_url: String,
    pub required: bool,
    pub pkce: bool,
    pub scope: Vec<String>,
    pub cloud_oauth_name: String, // Ie. Google OAuth App name
    pub header: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasicConfig {
    pub key_name: String,
    pub description: String,
    pub required: bool,
    pub key_value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_value_parsing() {
        let json_str = r#"{
            "key_name": "apiKey",
            "description": "API Key for weather service",
            "required": true,
            "key_value": "63d35ff6068c3103ccd1227546935111"
        }"#;

        let value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON");

        if let Some(tool_config) = ToolConfig::from_value(&value) {
            match tool_config {
                ToolConfig::BasicConfig(config) => {
                    assert_eq!(config.key_name, "apiKey");
                    assert_eq!(config.description, "API Key for weather service");
                    assert!(config.required);
                    assert_eq!(config.key_value, Some("63d35ff6068c3103ccd1227546935111".to_string()));
                }
                _ => panic!("Parsed ToolConfig is not a BasicConfig"),
            }
        } else {
            panic!("Failed to parse ToolConfig from value");
        }
    }

    #[test]
    fn test_from_value_parsing_with_missing_fields() {
        let json_str = r#"{
            "key_name": "apiKey",
            "key_value": "63d35ff6068c3103ccd1227546935111"
        }"#;

        let value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON");

        if let Some(tool_config) = ToolConfig::from_value(&value) {
            match tool_config {
                ToolConfig::BasicConfig(config) => {
                    assert_eq!(config.key_name, "apiKey");
                    assert_eq!(config.description, "");
                    assert!(!config.required);
                    assert_eq!(config.key_value, Some("63d35ff6068c3103ccd1227546935111".to_string()));
                }
                _ => panic!("Parsed ToolConfig is not a BasicConfig"),
            }
        } else {
            panic!("Failed to parse ToolConfig from value");
        }
    }
}
