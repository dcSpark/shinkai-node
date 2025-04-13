use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    schemas::{crontab::CronTask, job_config::JobConfig, shinkai_name::ShinkaiName, tool_router_key::ToolRouterKey}, shinkai_utils::job_scope::MinimalJobScope
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct Agent {
    pub name: String,
    pub agent_id: String,
    pub full_identity_name: ShinkaiName,
    pub llm_provider_id: String,
    pub ui_description: String,
    pub knowledge: Vec<String>,
    pub storage_path: String,
    #[serde(
        serialize_with = "ToolRouterKey::serialize_tool_router_keys",
        deserialize_with = "ToolRouterKey::deserialize_tool_router_keys"
    )]
    pub tools: Vec<ToolRouterKey>,
    pub debug_mode: bool,
    pub config: Option<JobConfig>,
    #[serde(default)]
    pub scope: MinimalJobScope,
    pub cron_tasks: Option<Vec<CronTask>>,
    pub embedding: Option<Vec<f32>>,
    pub mcp_enabled: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_agent_deserialize_with_valid_tools() {
        let json_data = json!({
            "name": "test_agent",
            "agent_id": "test123",
            "full_identity_name": "test.agent",
            "llm_provider_id": "test_provider",
            "ui_description": "Test Agent",
            "knowledge": ["test knowledge"],
            "storage_path": "/test/path",
            "tools": [
                "local:::__some_name:::tool1",
                "local:::__some_name:::tool2:::1.0"
            ],
            "debug_mode": false,
            "config": null
        });

        let agent: Agent = serde_json::from_value(json_data).unwrap();
        assert_eq!(agent.tools.len(), 2);
        assert_eq!(agent.tools[0].source, "local");
        assert_eq!(agent.tools[0].author, "__some_name");
        assert_eq!(agent.tools[0].name, "tool1");
        assert_eq!(agent.tools[0].version, None);
        assert_eq!(agent.tools[1].version, Some("1.0".to_string()));
    }

    #[test]
    fn test_agent_deserialize_with_invalid_tool() {
        let json_data = json!({
            "name": "test_agent",
            "agent_id": "test123",
            "full_identity_name": "test.agent",
            "llm_provider_id": "test_provider",
            "ui_description": "Test Agent",
            "knowledge": ["test knowledge"],
            "storage_path": "/test/path",
            "tools": [
                "invalid_tool_format"
            ],
            "debug_mode": false,
            "config": null
        });

        let result = serde_json::from_value::<Agent>(json_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_deserialize_with_empty_tools() {
        let json_data = json!({
            "name": "test_agent",
            "agent_id": "test123",
            "full_identity_name": "test.agent",
            "llm_provider_id": "test_provider",
            "ui_description": "Test Agent",
            "knowledge": ["test knowledge"],
            "storage_path": "/test/path",
            "tools": [],
            "debug_mode": false,
            "config": null
        });

        let agent: Agent = serde_json::from_value(json_data).unwrap();
        assert!(agent.tools.is_empty());
    }
}
