use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct MCPServer {
    pub id: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub name: String,
    pub r#type: MCPServerType,
    pub url: Option<String>,
    pub command: Option<String>,
    pub is_enabled: bool,
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
}
