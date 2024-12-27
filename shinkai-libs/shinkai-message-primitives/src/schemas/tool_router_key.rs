#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRouterKey {
    pub source: String,
    pub toolkit_name: String,
    pub name: String,
    pub version: Option<String>,
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

    pub fn to_string(&self) -> String {
        let sanitized_name = self.name.replace(':', "_").replace(' ', "_");
        let mut key = format!("{}:::{}:::{}", self.source, self.toolkit_name, sanitized_name);
        if let Some(version) = &self.version {
            key.push_str(&format!(":::{}", version));
        }
        key.replace('/', "|").to_lowercase()
    }

    pub fn from_string(key: &str) -> Result<Self, String> {
        let parts: Vec<&str> = key.split(":::").collect();
        match parts.len() {
            3 => Ok(Self::new(parts[0].to_string(), parts[1].to_string(), parts[2].to_string(), None)),
            4 => Ok(Self::new(parts[0].to_string(), parts[1].to_string(), parts[2].to_string(), Some(parts[3].to_string()))),
            _ => Err("Invalid tool router key format".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_router_key_to_string_without_version() {
        let key = ToolRouterKey::new("local".to_string(), "rust_toolkit".to_string(), "concat_strings".to_string(), None);
        assert_eq!(key.to_string(), "local:::rust_toolkit:::concat_strings");
    }

    #[test]
    fn test_tool_router_key_to_string_with_version() {
        let key = ToolRouterKey::new("local".to_string(), "rust_toolkit".to_string(), "concat_strings".to_string(), Some("1.0".to_string()));
        assert_eq!(key.to_string(), "local:::rust_toolkit:::concat_strings:::1.0");
    }

    #[test]
    fn test_tool_router_key_from_string_without_version() {
        let key_str = "local:::rust_toolkit:::concat_strings";
        let key = ToolRouterKey::from_string(key_str).unwrap();
        assert_eq!(key, ToolRouterKey::new("local".to_string(), "rust_toolkit".to_string(), "concat_strings".to_string(), None));
    }

    #[test]
    fn test_tool_router_key_from_string_with_version() {
        let key_str = "local:::rust_toolkit:::concat_strings:::1.0";
        let key = ToolRouterKey::from_string(key_str).unwrap();
        assert_eq!(key, ToolRouterKey::new("local".to_string(), "rust_toolkit".to_string(), "concat_strings".to_string(), Some("1.0".to_string())));
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
        let router_key_string = tool_router_key.to_string();

        // Expected key format
        let expected_key = "local:::deno_toolkit:::shinkai__download_pages";

        // Assert that the generated key matches the expected pattern
        assert_eq!(router_key_string, expected_key);
    }
}
