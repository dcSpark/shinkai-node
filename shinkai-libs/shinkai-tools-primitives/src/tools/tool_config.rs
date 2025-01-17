use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolConfig {
    BasicConfig(BasicConfig),
}

impl ToolConfig {
    /// User-facing name of the header. To be used by frontend with input box
    /// when user is required to input header values
    pub fn name(&self) -> String {
        match self {
            ToolConfig::BasicConfig(config) => config.key_name.clone(),
        }
    }

    /// Description of the header, to be used in frontend
    pub fn description(&self) -> String {
        match self {
            ToolConfig::BasicConfig(config) => config.description.clone(),
        }
    }

    /// The header key to be used when making the request
    pub fn header(&self) -> String {
        match self {
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
            ToolConfig::BasicConfig(config) => ToolConfig::BasicConfig(BasicConfig {
                key_name: config.key_name.clone(),
                description: config.description.clone(),
                required: config.required,
                type_name: config.type_name.clone(),
                key_value: None,
            }),
        }
    }

    /// Creates a vector of ToolConfig::BasicConfig instances from a serde_json::Value
    pub fn basic_config_from_value(value: &Value) -> Vec<ToolConfig> {
        let mut configs = Vec::new();

        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                let (key_value, type_name) = if let Some(val_obj) = val.as_object() {
                    (None, val_obj.get("type").and_then(|v| v.as_str()).map(String::from))
                } else {
                    // Convert any value type to string representation
                    let key_value = match val {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        Value::Bool(b) => Some(b.to_string()),
                        _ => None,
                    };
                    // Infer type_name based on value type
                    let type_name = match val {
                        Value::String(_) => Some("string".to_string()),
                        Value::Number(_) => Some("number".to_string()),
                        Value::Bool(_) => Some("boolean".to_string()),
                        _ => None,
                    };
                    (key_value, type_name)
                };

                let basic_config = BasicConfig {
                    key_name: key.clone(),
                    description: format!("Description for {}", key),
                    required: false,
                    type_name,
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
                let type_name = obj.get("type").and_then(|v| v.as_str()).map(String::from);

                let basic_config = BasicConfig {
                    key_name: key_name.to_string(),
                    description: description.to_string(),
                    required,
                    type_name,
                    key_value,
                };
                return Some(ToolConfig::BasicConfig(basic_config));
            }

            // Add similar checks for other ToolConfig variants like OAuth or GenericHeader if needed
        }

        None
    }

    /// Creates a vector of ToolConfig::OAuth instances from a serde_json::Value
    pub fn oauth_from_value(value: &Value) -> Vec<OAuth> {
        let mut oauths = Vec::new();

        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                if let Some(oauth_obj) = val.as_object() {
                    let mut oauth_value = serde_json::Map::new();
                    oauth_value.insert(key.clone(), serde_json::Value::Object(oauth_obj.clone()));
                    if let Some(oauth) = OAuth::from_value(&serde_json::Value::Object(oauth_value)) {
                        oauths.push(oauth);
                    }
                }
            }
        }

        oauths
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
    pub scope: Option<String>,
    #[serde(rename = "authorizationUrl")]
    pub authorization_url: String,
    #[serde(rename = "tokenUrl")]
    pub token_url: Option<String>,
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(rename = "clientSecret")]
    pub client_secret: String,
    #[serde(rename = "redirectUrl")]
    pub redirect_url: String,
    pub version: String,
    #[serde(rename = "responseType")]
    pub response_type: String,
    pub scopes: Vec<String>,
    #[serde[rename = "pkceType"]]
    pub pkce_type: Option<String>,
    pub refresh_token: Option<String>,
}

impl OAuth {
    /// Attempts to deserialize a serde_json::Value into an OAuth instance
    pub fn from_value(value: &Value) -> Option<OAuth> {
        if let Some(obj) = value.as_object() {
            // We need a name and at least one other field to create a valid OAuth object
            let name = obj.keys().next()?.to_string();
            let oauth_obj = obj.get(&name)?.as_object()?;

            Some(OAuth {
                name,
                scope: oauth_obj.get("scope").and_then(|v| v.as_str()).map(String::from),
                authorization_url: oauth_obj
                    .get("authorizationUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://secrets.shinkai.com/redirect")
                    .to_string(),
                token_url: oauth_obj.get("tokenUrl").and_then(|v| v.as_str()).map(String::from),
                client_id: oauth_obj
                    .get("clientId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                client_secret: oauth_obj
                    .get("clientSecret")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                redirect_url: oauth_obj
                    .get("redirectUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://secrets.shinkai.com/redirect")
                    .to_string(),
                version: oauth_obj
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                response_type: oauth_obj
                    .get("responseType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("code")
                    .to_string(),
                scopes: oauth_obj
                    .get("scopes")
                    .and_then(|v| v.as_array())
                    .map_or(Vec::new(), |arr| {
                        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    }),
                pkce_type: oauth_obj.get("pkceType").and_then(|v| v.as_str()).map(String::from),
                refresh_token: oauth_obj.get("refreshToken").and_then(|v| {
                    let value = v.as_str().unwrap_or_default().to_string();
                    let expected = "true".to_string();
                    if value == expected {
                        Some("true".to_string())
                    } else {
                        Some("false".to_string())
                    }
                }),
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasicConfig {
    pub key_name: String,
    pub description: String,
    pub required: bool,
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub key_value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_from_value_parsing() {
        let json_str = r#"{
            "key_name": "apiKey",
            "description": "API Key for weather service",
            "required": true,
            "type": "string",
            "key_value": "63d35ff6068c3103ccd1227546935111"
        }"#;

        let value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON");

        if let Some(tool_config) = ToolConfig::from_value(&value) {
            match tool_config {
                ToolConfig::BasicConfig(config) => {
                    assert_eq!(config.key_name, "apiKey");
                    assert_eq!(config.description, "API Key for weather service");
                    assert!(config.required);
                    assert_eq!(config.type_name, Some("string".to_string()));
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
                    assert_eq!(config.type_name, None);
                    assert_eq!(config.key_value, Some("63d35ff6068c3103ccd1227546935111".to_string()));
                }
                _ => panic!("Parsed ToolConfig is not a BasicConfig"),
            }
        } else {
            panic!("Failed to parse ToolConfig from value");
        }
    }

    #[test]
    fn test_oauth_from_value() {
        let json_str = r#"{
            "github": {
                "redirectUrl": "https://custom.redirect.com",
                "version": "2.0",
                "responseType": "code",
                "authorizationUrl": "https://github.com/login/oauth/authorize",
                "clientId": "test_client_id",
                "clientSecret": "test_client_secret",
                "scopes": ["repo", "user"],
                "pkceType": "plain",
                "refreshToken": "true"
            }
        }"#;

        let value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON");
        let oauth = OAuth::from_value(&value).expect("Failed to parse OAuth from value");

        assert_eq!(oauth.name, "github");
        assert_eq!(oauth.redirect_url, "https://custom.redirect.com");
        assert_eq!(oauth.version, "2.0");
        assert_eq!(oauth.response_type, "code".to_string());
        assert_eq!(oauth.authorization_url, "https://github.com/login/oauth/authorize");
        assert_eq!(oauth.client_id, "test_client_id");
        assert_eq!(oauth.client_secret, "test_client_secret");
        assert_eq!(oauth.scopes, vec!["repo", "user"]);
        assert_eq!(oauth.pkce_type, Some("plain".to_string()));
        assert_eq!(oauth.refresh_token, Some("true".to_string()));
    }

    #[test]
    fn test_oauth_from_value_with_defaults() {
        let json_str = r#"{
            "minimal": {
                "authorizationUrl": "https://example.com/auth"
            }
        }"#;

        let value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON");
        let oauth = OAuth::from_value(&value).expect("Failed to parse OAuth from value");

        assert_eq!(oauth.name, "minimal");
        assert_eq!(oauth.redirect_url, "https://secrets.shinkai.com/redirect");
        assert_eq!(oauth.version, "");
        assert_eq!(oauth.response_type, "code".to_string());
        assert_eq!(oauth.authorization_url, "https://example.com/auth");
        assert_eq!(oauth.client_id, "");
        assert_eq!(oauth.client_secret, "");
        assert!(oauth.scopes.is_empty());
        assert_eq!(oauth.pkce_type, None);
        assert_eq!(oauth.refresh_token, Some("false".to_string()));
    }

    #[test]
    fn test_basic_config_from_value_different_types() {
        let test_config = json!({
            "string_key": "test_value",
            "number_key": 42,
            "bool_key": true,
            "object_key": {
                "type": "custom_type"
            }
        });

        let configs = ToolConfig::basic_config_from_value(&test_config);

        assert_eq!(configs.len(), 4);

        // Helper function to find config by key
        let find_config = |key: &str| {
            configs.iter().find(|c| {
                let ToolConfig::BasicConfig(bc) = c;
                bc.key_name == key
            })
        };

        // Test string value
        if let Some(ToolConfig::BasicConfig(string_config)) = find_config("string_key") {
            assert_eq!(string_config.key_value, Some("test_value".to_string()));
            assert_eq!(string_config.type_name, Some("string".to_string()));
        } else {
            panic!("string_key config not found");
        }

        // Test number value
        if let Some(ToolConfig::BasicConfig(number_config)) = find_config("number_key") {
            assert_eq!(number_config.key_value, Some("42".to_string()));
            assert_eq!(number_config.type_name, Some("number".to_string()));
        } else {
            panic!("number_key config not found");
        }

        // Test boolean value
        if let Some(ToolConfig::BasicConfig(bool_config)) = find_config("bool_key") {
            assert_eq!(bool_config.key_value, Some("true".to_string()));
            assert_eq!(bool_config.type_name, Some("boolean".to_string()));
        } else {
            panic!("bool_key config not found");
        }

        // Test object value
        if let Some(ToolConfig::BasicConfig(object_config)) = find_config("object_key") {
            assert_eq!(object_config.key_value, None);
            assert_eq!(object_config.type_name, Some("custom_type".to_string()));
        } else {
            panic!("object_key config not found");
        }
    }
}
