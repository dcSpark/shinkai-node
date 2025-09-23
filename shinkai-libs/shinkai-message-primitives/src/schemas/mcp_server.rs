use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use utoipa::ToSchema;

pub type MCPServerEnv = std::collections::HashMap<String, String>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct MCPServer {
    pub id: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub name: String,
    pub r#type: MCPServerType,
    pub url: Option<String>,
    pub env: Option<MCPServerEnv>,
    pub command: Option<String>,
    pub is_enabled: bool,
}

impl MCPServer {
    pub fn sanitize_env(&mut self) {
        let mut sanitized_env = MCPServerEnv::new();
        for (key, _) in self.env.clone().unwrap_or_default() {
            sanitized_env.insert(key.clone(), "".to_string());
        }
        self.env = Some(sanitized_env);
    }
    pub fn get_command_hash(&self) -> String {
        // Create a string to hash based on the command and related fields
        let command_string = match self.r#type {
            MCPServerType::Command => self.command.clone().unwrap_or_default().trim().to_string(),
            MCPServerType::Sse => self.url.clone().unwrap_or_default().trim().to_string(),
            MCPServerType::Http => self.url.clone().unwrap_or_default().trim().to_string(),
        };

        // Create a hasher and hash the command string
        let mut hasher = DefaultHasher::new();
        command_string.hash(&mut hasher);

        // Get the full 64-bit hash
        let hash_64 = hasher.finish();

        // Use modulo to ensure it fits in exactly 12 base36 digits (36^12 = 4,738,381,338,321,616,896)
        let hash_mod = hash_64 % 4_738_381_338_321_616_896;

        // Convert to base36 string and pad to exactly 12 characters
        Self::u64_to_base36_fixed_length(hash_mod, 12)
    }

    fn u64_to_base36_fixed_length(mut num: u64, length: usize) -> String {
        const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
        let mut result = Vec::new();

        // Convert to base36
        while num > 0 {
            result.push(CHARS[(num % 36) as usize]);
            num /= 36;
        }

        // Pad with leading zeros if necessary
        while result.len() < length {
            result.push(b'0');
        }

        result.reverse();
        String::from_utf8(result).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum MCPServerType {
    Sse,
    Command,
    Http,
}

impl MCPServerType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_uppercase().as_str() {
            "SSE" => Ok(MCPServerType::Sse),
            "COMMAND" => Ok(MCPServerType::Command),
            "HTTP" => Ok(MCPServerType::Http),
            _ => Err(format!("Invalid MCP server type: {}", s)),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            MCPServerType::Sse => "SSE".to_string(),
            MCPServerType::Command => "COMMAND".to_string(),
            MCPServerType::Http => "HTTP".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_server(command: Option<String>) -> MCPServer {
        MCPServer {
            id: None,
            created_at: None,
            updated_at: None,
            name: "Test Server".to_string(),
            r#type: MCPServerType::Command,
            url: Some("http://example.com".to_string()),
            env: None,
            command,
            is_enabled: true,
        }
    }

    #[test]
    fn test_same_command_same_hash() {
        let server1 = create_test_server(Some(
            "npx @modelcontextprotocol/server-everything@2025.9.12".to_string(),
        ));
        let server2 = create_test_server(Some(
            "npx @modelcontextprotocol/server-everything@2025.9.12".to_string(),
        ));

        let hash1 = server1.get_command_hash();
        let hash2 = server2.get_command_hash();

        assert_eq!(hash1, hash2, "Same commands should produce the same hash");
        assert_eq!(hash1.len(), 12, "Hash should be exactly 12 characters long");
    }

    #[test]
    fn test_trimmed_commands_same_hash() {
        let server1 = create_test_server(Some("npx test-command".to_string()));
        let server2 = create_test_server(Some("  npx test-command  ".to_string()));
        let server3 = create_test_server(Some("\tnpx test-command\n".to_string()));

        let hash1 = server1.get_command_hash();
        let hash2 = server2.get_command_hash();
        let hash3 = server3.get_command_hash();

        assert_eq!(
            hash1, hash2,
            "Commands with leading/trailing spaces should have same hash"
        );
        assert_eq!(hash1, hash3, "Commands with tabs/newlines should have same hash");
        assert_eq!(hash1.len(), 12, "Hash should be exactly 12 characters long");
    }

    #[test]
    fn test_different_commands_different_hash() {
        let server1 = create_test_server(Some("npx command1".to_string()));
        let server2 = create_test_server(Some("npx command2".to_string()));

        let hash1 = server1.get_command_hash();
        let hash2 = server2.get_command_hash();

        assert_ne!(hash1, hash2, "Different commands should produce different hashes");
        assert_eq!(hash1.len(), 12, "Hash should be exactly 12 characters long");
        assert_eq!(hash2.len(), 12, "Hash should be exactly 12 characters long");
    }

    #[test]
    fn test_none_command_hash() {
        let server1 = create_test_server(None);
        let server2 = create_test_server(None);

        let hash1 = server1.get_command_hash();
        let hash2 = server2.get_command_hash();

        assert_eq!(hash1, hash2, "Servers with no command should have same hash");
        assert_eq!(hash1.len(), 12, "Hash should be exactly 12 characters long");
    }

    #[test]
    fn test_hash_consistency_across_calls() {
        let server = create_test_server(Some("npx consistent-test".to_string()));

        let hash1 = server.get_command_hash();
        let hash2 = server.get_command_hash();
        let hash3 = server.get_command_hash();

        assert_eq!(hash1, hash2, "Multiple calls should return same hash");
        assert_eq!(hash2, hash3, "Multiple calls should return same hash");
        assert_eq!(hash1.len(), 12, "Hash should be exactly 12 characters long");
    }

    #[test]
    fn test_hash_format() {
        let server = create_test_server(Some("test command".to_string()));
        let hash = server.get_command_hash();

        assert_eq!(hash.len(), 12, "Hash should be exactly 12 characters");

        // Verify all characters are valid base36 (0-9, a-z)
        for ch in hash.chars() {
            assert!(
                ch.is_ascii_digit() || (ch.is_ascii_lowercase() && ch <= 'z'),
                "Hash should only contain base36 characters (0-9, a-z), found: {}",
                ch
            );
        }
    }
}
