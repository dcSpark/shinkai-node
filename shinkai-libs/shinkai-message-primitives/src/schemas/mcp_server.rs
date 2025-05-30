use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum MCPServerType {
    Sse,
    Command,
}

impl MCPServerType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_uppercase().as_str() {
            "SSE" => Ok(MCPServerType::Sse),
            "COMMAND" => Ok(MCPServerType::Command),
            _ => Err(format!("Invalid MCP server type: {}", s)),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            MCPServerType::Sse => "SSE".to_string(),
            MCPServerType::Command => "COMMAND".to_string(),
        }
    }
}
