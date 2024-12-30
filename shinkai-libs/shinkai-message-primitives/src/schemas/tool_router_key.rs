use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::indexable_version::IndexableVersion;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "String")]
pub struct ToolRouterKey {
    pub source: String,
    pub toolkit_name: String,
    pub name: String,
    pub version: Option<String>,
}

impl TryFrom<String> for ToolRouterKey {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        ToolRouterKey::from_string(&s).map_err(|e| e.to_string())
    }
}

impl ToolRouterKey {
    pub fn new(source: String, toolkit_name: String, name: String, version: Option<String>) -> Self {
        Self {
            source,
            toolkit_name,
            name,
            version,
        }
    }

    pub fn deserialize_tool_router_keys<'de, D>(deserializer: D) -> Result<Option<Vec<Self>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let string_vec: Option<Vec<String>> = Option::deserialize(deserializer)?;
    
        match string_vec {
            Some(vec) => {
                let router_keys = vec
                    .into_iter()
                    .map(|s| Self::from_string(&s))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(serde::de::Error::custom)?;
                Ok(Some(router_keys))
            }
            None => Ok(None),
        }
    }

    pub fn serialize_tool_router_keys<S>(
        keys: &Option<Vec<ToolRouterKey>>, 
        serializer: S
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match keys {
            Some(keys) => {
                let strings: Vec<String> = keys
                    .iter()
                    .map(|k| {
                        // If version is Some, use to_string_with_version()
                        if k.version.is_some() {
                            k.to_string_with_version()
                        } else {
                            k.to_string_without_version()
                        }
                    })
                    .collect();
                strings.serialize(serializer)
            }
            None => serializer.serialize_none(),
        }
    }

    fn sanitize(input: &str) -> String {
        input.chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .collect()
    }

    pub fn to_string_without_version(&self) -> String {
        let sanitized_source = Self::sanitize(&self.source);
        let sanitized_toolkit_name = Self::sanitize(&self.toolkit_name);
        let sanitized_name = Self::sanitize(&self.name);
        
        let key = format!("{}:::{}:::{}", sanitized_source, sanitized_toolkit_name, sanitized_name);
        key.replace('/', "|").to_lowercase()
    }

    pub fn to_string_with_version(&self) -> String {
        let sanitized_source = Self::sanitize(&self.source);
        let sanitized_toolkit_name = Self::sanitize(&self.toolkit_name);
        let sanitized_name = Self::sanitize(&self.name);
        
        let version_str = self.version.clone().unwrap_or_else(|| "none".to_string());

        let key = format!(
            "{}:::{}:::{}:::{}",
            sanitized_source, sanitized_toolkit_name, sanitized_name, version_str
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_router_key_to_string_without_version() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "rust_toolkit".to_string(),
            "concat_strings".to_string(),
            None,
        );
        assert_eq!(key.to_string_without_version(), "local:::rust_toolkit:::concat_strings");
    }

    #[test]
    fn test_tool_router_key_to_string_with_version() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "rust_toolkit".to_string(),
            "concat_strings".to_string(),
            Some("1.0".to_string()),
        );
        assert_eq!(
            key.to_string_with_version(),
            "local:::rust_toolkit:::concat_strings:::1.0"
        );
    }

    #[test]
    fn test_tool_router_key_to_string_with_version_none() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "rust_toolkit".to_string(),
            "concat_strings".to_string(),
            None,
        );
        assert_eq!(
            key.to_string_with_version(),
            "local:::rust_toolkit:::concat_strings:::none"
        );
    }

    #[test]
    fn test_tool_router_key_from_string_without_version() {
        let key_str = "local:::rust_toolkit:::concat_strings";
        let key = ToolRouterKey::from_string(key_str).unwrap();
        assert_eq!(
            key,
            ToolRouterKey::new(
                "local".to_string(),
                "rust_toolkit".to_string(),
                "concat_strings".to_string(),
                None
            )
        );
    }

    #[test]
    fn test_tool_router_key_from_string_with_version() {
        let key_str = "local:::rust_toolkit:::concat_strings:::1.0";
        let key = ToolRouterKey::from_string(key_str).unwrap();
        assert_eq!(
            key,
            ToolRouterKey::new(
                "local".to_string(),
                "rust_toolkit".to_string(),
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
            "deno_toolkit".to_string(),
            "shinkai: download pages".to_string(),
            None,
        );

        // Generate the router key string
        let router_key_string = tool_router_key.to_string_without_version();

        // Expected key format
        let expected_key = "local:::deno_toolkit:::shinkai__download_pages";

        // Assert that the generated key matches the expected pattern
        assert_eq!(router_key_string, expected_key);
    }

    #[test]
    fn test_tool_router_key_no_spaces_in_to_string() {
        let key = ToolRouterKey::new(
            "local".to_string(),
            "deno toolkit".to_string(),
            "versioned_tool".to_string(),
            Some("2.0".to_string()),
        );
        let key_string = key.to_string_without_version();
        eprintln!("key_string: {:?}", key_string);
        assert!(!key_string.contains(' '), "Key string should not contain spaces");
        assert_eq!(key_string, "local:::deno_toolkit:::versioned_tool");
    }
}
