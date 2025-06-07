use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::indexable_version::IndexableVersion;
use super::shinkai_name::ShinkaiName;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "String")]
pub struct ToolRouterKey {
    pub source: String,
    pub author: String,
    pub name: String,
    pub version: Option<String>,
}

impl TryFrom<String> for ToolRouterKey {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let result = ToolRouterKey::from_string(&s);
        result.map_err(|e| e.to_string())
    }
}

impl ToolRouterKey {
    pub fn new(source: String, author: String, name: String, version: Option<String>) -> Self {
        Self {
            source,
            author,
            name,
            version,
        }
    }

    pub fn deserialize_tool_router_keys<'de, D>(deserializer: D) -> Result<Vec<Self>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string_vec: Vec<String> = match Vec::deserialize(deserializer) {
            Ok(v) => v,
            Err(e) => {
                println!("Failed to deserialize string vector: {}", e);
                return Err(e);
            }
        };

        let result = string_vec
            .into_iter()
            .map(|s| {
                Self::from_string(&s).map_err(|e| {
                    println!("Failed to parse tool router key: {}", e);
                    serde::de::Error::custom(e)
                })
            })
            .collect();

        result
    }

    pub fn serialize_tool_router_keys<S>(tools: &Vec<ToolRouterKey>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let strings: Vec<String> = tools
            .iter()
            .map(|k| {
                let s = k.to_string_with_version();
                s
            })
            .collect();
        strings.serialize(serializer)
    }

    pub fn serialize_tool_router_key<S>(tool: &Option<ToolRouterKey>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(key) = &tool {
            key.to_string_with_version().serialize(serializer)
        } else {
            None::<String>.serialize(serializer)
        }
    }

    pub fn deserialize_tool_router_key<'de, D>(deserializer: D) -> Result<Option<ToolRouterKey>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = match String::deserialize(deserializer) {
            Ok(v) => v,
            Err(e) => {
                println!("Failed to deserialize string: {}", e);
                return Err(e);
            }
        };

        let tool = ToolRouterKey::from_string(&s).map_err(|e| {
            println!("Failed to parse tool router key: {}", e);
            serde::de::Error::custom(e)
        })?;
        Ok(Some(tool))
    }

    pub fn sanitize(input: &str) -> String {
        input
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .collect()
    }

    pub fn to_string_without_version(&self) -> String {
        let sanitized_source = Self::sanitize(&self.source);
        let sanitized_author = Self::sanitize(&self.author);
        let sanitized_name = Self::sanitize(&self.name);

        let key = format!("{}:::{}:::{}", sanitized_source, sanitized_author, sanitized_name);
        key.replace('/', "|").to_lowercase()
    }

    pub fn to_string_with_version(&self) -> String {
        if self.version.is_none() {
            return self.to_string_without_version();
        }

        let sanitized_source = Self::sanitize(&self.source);
        let sanitized_author = Self::sanitize(&self.author);
        let sanitized_name = Self::sanitize(&self.name);

        let version_str = self.version.clone().unwrap();

        let key = format!(
            "{}:::{}:::{}:::{}",
            sanitized_source, sanitized_author, sanitized_name, version_str
        );

        key.replace('/', "|").to_lowercase()
    }

    pub fn from_string(key: &str) -> Result<Self, String> {
        let parts: Vec<&str> = key.split(":::").collect();
        match parts.len() {
            3 => Ok(Self::new(
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
                None,
            )),
            4 => Ok(Self::new(
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
                Some(parts[3].to_string()),
            )),
            _ => Err("Invalid tool router key format".to_string()),
        }
    }

    pub fn convert_to_path(&self) -> String {
        self.to_string_without_version()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .to_lowercase()
    }

    pub fn version(&self) -> Option<IndexableVersion> {
        self.version
            .as_ref()
            .and_then(|v| IndexableVersion::from_string(v).ok())
    }

    /// Converts a normal tool router key to a network router key
    /// Example: "local:::guillevalin:::echo_function" with node_name "@@guillevalin.sep-shinkai"
    /// becomes "__guillevalin_sep_shinkai:::guillevalin:::echo_function"
    pub fn to_network_router_key(key_str: &str, node_name: &str) -> Result<String, String> {
        let key = Self::from_string(key_str)?;

        // Create a ShinkaiName to properly validate the node name
        let shinkai_name =
            ShinkaiName::new(node_name.to_string()).map_err(|e| format!("Invalid node name '{}': {}", node_name, e))?;

        // Sanitize the node name to create the network source
        let network_source = Self::sanitize(&shinkai_name.node_name);
        let network_key = Self::new(network_source, key.author, key.name, key.version);
        Ok(network_key.to_string_with_version())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_router_key_to_string_without_version() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "@@official.shinkai".to_string(),
            "concat_strings".to_string(),
            None,
        );
        assert_eq!(
            key.to_string_without_version(),
            "local:::__official_shinkai:::concat_strings"
        );
    }

    #[test]
    fn test_tool_router_key_to_string_with_version() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "@@official.shinkai".to_string(),
            "concat_strings".to_string(),
            Some("1.0".to_string()),
        );
        assert_eq!(
            key.to_string_with_version(),
            "local:::__official_shinkai:::concat_strings:::1.0"
        );
    }

    #[test]
    fn test_tool_router_key_from_string_without_version() {
        let key_str = "local:::__official_shinkai:::concat_strings";
        let key = ToolRouterKey::from_string(key_str).unwrap();
        assert_eq!(
            key,
            ToolRouterKey::new(
                "local".to_string(),
                "__official_shinkai".to_string(),
                "concat_strings".to_string(),
                None
            )
        );
    }

    #[test]
    fn test_tool_router_key_from_string_with_version() {
        let key_str = "local:::__official_shinkai:::concat_strings:::1.0";
        let key = ToolRouterKey::from_string(key_str).unwrap();
        assert_eq!(
            key,
            ToolRouterKey::new(
                "local".to_string(),
                "__official_shinkai".to_string(),
                "concat_strings".to_string(),
                Some("1.0".to_string())
            )
        );
    }

    #[test]
    fn test_tool_router_key_from_string_invalid_format() {
        let key_str = "invalid_key_format";
        assert!(ToolRouterKey::from_string(key_str).is_err());
    }

    #[test]
    fn test_tool_router_key_generation() {
        // Create a ToolRouterKey instance
        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "@@system.shinkai".to_string(),
            "shinkai: download pages".to_string(),
            None,
        );

        // Generate the router key string
        let router_key_string = tool_router_key.to_string_without_version();

        // Expected key format
        let expected_key = "local:::__system_shinkai:::shinkai__download_pages";

        // Assert that the generated key matches the expected pattern
        assert_eq!(router_key_string, expected_key);
    }

    #[test]
    fn test_tool_router_key_no_spaces_in_to_string() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "@@system.shinkai".to_string(),
            "versioned_tool".to_string(),
            Some("2.0".to_string()),
        );
        let key_string = key.to_string_without_version();
        eprintln!("key_string: {:?}", key_string);
        assert!(!key_string.contains(' '), "Key string should not contain spaces");
        assert_eq!(key_string, "local:::__system_shinkai:::versioned_tool");
    }

    #[test]
    fn test_tool_router_key_to_string_with_version_returns_without_version_when_none() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "@@official_shinkai".to_string(),
            "concat_strings".to_string(),
            None,
        );
        assert_eq!(
            key.to_string_with_version(),
            "local:::__official_shinkai:::concat_strings"
        );
    }

    #[test]
    fn test_to_network_router_key() {
        let original_key = "local:::guillevalin:::echo_function";
        let node_name = "@@guillevalin.sep-shinkai";
        let network_key = ToolRouterKey::to_network_router_key(original_key, node_name).unwrap();
        assert_eq!(network_key, "__guillevalin_sep_shinkai:::guillevalin:::echo_function");
    }

    #[test]
    fn test_to_network_router_key_with_version() {
        let original_key = "local:::guillevalin:::echo_function:::1.0";
        let node_name = "@@guillevalin.sep-shinkai";
        let network_key = ToolRouterKey::to_network_router_key(original_key, node_name).unwrap();
        assert_eq!(
            network_key,
            "__guillevalin_sep_shinkai:::guillevalin:::echo_function:::1.0"
        );
    }

    #[test]
    fn test_to_network_router_key_different_domains() {
        let original_key = "local:::alice:::test_tool";

        // Test with .shinkai domain
        let node_name1 = "@@alice.sep-shinkai";
        let network_key1 = ToolRouterKey::to_network_router_key(original_key, node_name1).unwrap();
        assert_eq!(network_key1, "__alice_sep_shinkai:::alice:::test_tool");

        // Test with .arb-sep-shinkai domain
        let node_name2 = "@@alice.sep-shinkai";
        let network_key2 = ToolRouterKey::to_network_router_key(original_key, node_name2).unwrap();
        assert_eq!(network_key2, "__alice_sep_shinkai:::alice:::test_tool");
    }
}
