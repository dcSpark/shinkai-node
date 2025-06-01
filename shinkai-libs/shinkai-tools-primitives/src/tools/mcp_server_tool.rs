use super::parameters::Parameters;
use super::tool_config::ToolConfig;
use super::tool_output_arg::ToolOutputArg;
use super::tool_playground::ToolPlaygroundMetadata;
use super::tool_types::{OperatingSystem, RunnerType, ToolResult};
use crate::tools::error::ToolError;
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;
use shinkai_mcp::mcp_methods::{run_tool_via_command, run_tool_via_sse};
use shinkai_message_primitives::schemas::mcp_server::{MCPServer, MCPServerType};
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_tools_runner::tools::run_result::RunResult;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MCPServerTool {
    pub version: String,
    pub name: String,
    pub author: String,
    pub mcp_server_ref: String,
    pub description: String,
    pub mcp_server_url: String,
    pub mcp_server_tool: String,

    #[serde(
        serialize_with = "ToolRouterKey::serialize_tool_router_key",
        deserialize_with = "ToolRouterKey::deserialize_tool_router_key"
    )]
    pub tool_router_key: Option<ToolRouterKey>,
    pub config: Vec<ToolConfig>,
    pub keywords: Vec<String>,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub activated: bool,
    pub embedding: Option<Vec<f32>>,
    pub result: ToolResult,
    pub tool_set: Option<String>,
    pub mcp_enabled: Option<bool>,
}

impl MCPServerTool {
    pub fn create_tool_router_key(node_name: String, server_id: String, tool_name: String) -> ToolRouterKey {
        ToolRouterKey::new(
            "local".to_string(),
            node_name.to_string(),
            format!("mcp_{}_{}", server_id, tool_name),
            None,
        )
    }

    pub fn get_metadata(&self) -> ToolPlaygroundMetadata {
        ToolPlaygroundMetadata {
            name: self.name.clone(),
            description: self.description.clone(),
            keywords: self.keywords.clone(),
            homepage: None,
            author: self.author.clone(),
            version: self.version.clone(),
            configurations: self.config.clone(),
            parameters: self.input_args.clone(),
            result: self.result.clone(),
            sql_tables: vec![],
            sql_queries: vec![],
            tools: None,
            oauth: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Linux, OperatingSystem::MacOS, OperatingSystem::Windows],
            tool_set: None,
        }
    }

    /// Convert to json
    pub fn to_json(&self) -> Result<String, ToolError> {
        serde_json::to_string(self).map_err(|_| ToolError::FailedJSONParsing)
    }

    /// Convert from json
    pub fn from_json(json: &str) -> Result<Self, ToolError> {
        let deserialized: Self = serde_json::from_str(json)?;
        Ok(deserialized)
    }

    pub async fn run(
        &self,
        mcp_server: MCPServer,
        parameters: serde_json::Map<String, serde_json::Value>,
        extra_config: Vec<ToolConfig>,
    ) -> Result<RunResult, ToolError> {
        let mut env: HashMap<String, String> = MCPServerTool::tool_config_to_env_vars(self.config.clone());

        // Merge extra_config into the config hashmap
        for c in extra_config {
            let ToolConfig::BasicConfig(basic_config) = c;
            if let Some(value) = basic_config.key_value {
                let mut parsed_value = value.to_string();
                if let Some(value) = value.as_str() {
                    parsed_value = value.to_string();
                }
                env.insert(basic_config.key_name.clone(), parsed_value);
            }
        }

        let value = MCPServerTool::run_tool(mcp_server, self.mcp_server_tool.clone(), env, parameters).await?;
        if value.is_error.unwrap_or(false) {
            let error = MCPServerTool::map_content_to_error_message(value.content).await;
            return Err(ToolError::ExecutionError(error));
        }
        let data = value.content.first().ok_or(ToolError::ExecutionError(
            "no content returned from MCP server".to_string(),
        ))?;
        Ok(RunResult {
            data: serde_json::to_value(data).map_err(|e| ToolError::FailedJSONParsing)?,
        })
    }

    pub async fn run_tool(
        mcp_server: MCPServer,
        tool: String,
        env: HashMap<String, String>,
        parameters: serde_json::Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, shinkai_mcp::error::McpError> {
        match mcp_server.r#type {
            MCPServerType::Command => {
                run_tool_via_command(mcp_server.command.unwrap_or_default(), tool, env, parameters).await
            }
            MCPServerType::Sse => run_tool_via_sse(mcp_server.url.unwrap_or_default(), tool, parameters).await,
        }
    }

    pub async fn map_content_to_error_message(content: Vec<Content>) -> String {
        content
            .iter()
            .map(|c| c.as_text().and_then(|t| Some(t.text.clone())).unwrap_or(String::new()))
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn map_content_to_value(content: Vec<Content>) -> Value {
        serde_json::to_value(content).unwrap_or(serde_json::Value::Null)
    }

    pub fn tool_config_to_env_vars(configs: Vec<ToolConfig>) -> HashMap<String, String> {
        let configs = configs
            .iter()
            .filter_map(|c| {
                if let ToolConfig::BasicConfig(c) = c {
                    if let Some(value) = &c.key_value {
                        let mut parsed_value = value.to_string();
                        if let Some(value) = value.as_str() {
                            parsed_value = value.to_string();
                        }
                        Some((c.key_name.clone(), parsed_value))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<(String, String)>>();
        HashMap::from_iter(configs)
    }

    pub fn check_required_config_fields(&self) -> bool {
        // Check if all required config fields are present
        true // For now, no required fields
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_run_tool() {
        let result = MCPServerTool::run_tool(
            MCPServer {
                id: Some(1),
                created_at: Some(String::from("2021-01-01T00:00:00Z")),
                updated_at: Some(String::from("2021-01-01T00:00:00Z")),
                name: "@modelcontextprotocol/server-everything".to_string(),
                r#type: MCPServerType::Command,
                url: None,
                command: Some("npx -y @modelcontextprotocol/server-everything".to_string()),
                is_enabled: true,
                env: None,
            },
            "add".to_string(),
            HashMap::new(),
            json!({
                "a": 1,
                "b": 2,
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .inspect_err(|e| {
            println!("error {:?}", e);
        });

        assert!(result.is_ok());
        let unwrapped = result.unwrap();
        assert_eq!(unwrapped.content.len(), 1);
        assert!(unwrapped.content[0].as_text().unwrap().text.contains("3"));
    }
}
