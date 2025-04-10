use super::parameters::{Parameters, Property};
use super::tool_config::{OAuth, ToolConfig};
use super::tool_output_arg::ToolOutputArg;
use super::tool_types::{OperatingSystem, RunnerType};
use super::error::ToolError;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentTool {
    pub version: String,
    pub name: String,
    pub homepage: Option<String>,
    pub author: String,
    pub mcp_enabled: Option<bool>,
    pub config: Vec<ToolConfig>,
    pub description: String,
    pub keywords: Vec<String>,
    pub input_args: Parameters,
    pub output_arg: ToolOutputArg,
    pub activated: bool,
    pub embedding: Option<Vec<f32>>,
    pub tools: Vec<ToolRouterKey>,
    pub runner: RunnerType,
    pub operating_system: Vec<OperatingSystem>,
    pub tool_set: Option<String>,
    pub oauth: Option<Vec<OAuth>>,
}

impl AgentTool {
    pub fn new(
        version: String,
        name: String,
        author: String,
        description: String,
        keywords: Vec<String>,
        input_args: Parameters,
        output_arg: ToolOutputArg,
        tools: Vec<ToolRouterKey>,
        runner: RunnerType,
        operating_system: Vec<OperatingSystem>,
        tool_set: Option<String>,
        homepage: Option<String>,
        oauth: Option<Vec<OAuth>>,
    ) -> Self {
        AgentTool {
            version,
            name,
            author,
            description,
            keywords,
            input_args,
            output_arg,
            activated: true,
            embedding: None,
            tools,
            runner,
            operating_system,
            tool_set,
            homepage,
            config: vec![],
            mcp_enabled: None,
            oauth,
        }
    }

    pub fn default_input_args() -> Parameters {
        let mut params = Parameters::new();
        
        params.add_property(
            "prompt".to_string(),
            "string".to_string(),
            "The prompt to send to the agent".to_string(),
            true,
        );
        
        let image_property = Property::new(
            "string".to_string(),
            "URL or base64 encoded image".to_string(),
        );
        let array_property = Property::with_array_items(
            "Images to include with the prompt".to_string(),
            image_property,
        );
        params.properties.insert("images".to_string(), array_property);
        
        params.add_property(
            "session_id".to_string(),
            "string".to_string(),
            "Session ID to reuse an existing chat session".to_string(),
            false,
        );
        
        params
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    pub fn from_json(json: &serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }

    pub async fn run(
        &self,
        envs: HashMap<String, String>,
        api_ip: String,
        api_port: u16,
        support_files: Vec<String>,
        function_args: serde_json::Value,
        function_config: Vec<ToolConfig>,
        node_storage_path: String,
        app_id: String,
        tool_id: String,
        node_name: shinkai_message_primitives::schemas::shinkai_name::ShinkaiName,
        is_test: bool,
        tool_name: Option<String>,
        additional_files: Option<Vec<String>>,
    ) -> Result<serde_json::Value, ToolError> {
        let prompt = function_args["prompt"]
            .as_str()
            .ok_or_else(|| ToolError::ExecutionError("Missing prompt parameter".to_string()))?
            .to_string();
        
        let images = function_args.get("images").and_then(|v| {
            if v.is_array() {
                Some(
                    v.as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|img| img.as_str().map(|s| s.to_string()))
                        .collect::<Vec<String>>(),
                )
            } else {
                None
            }
        });
        
        let session_id = function_args
            .get("session_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()));
        
        
        let response = if let Some(session_id) = session_id {
            serde_json::json!({
                "result": "Agent executed with existing session",
                "session_id": session_id,
                "prompt": prompt,
                "images": images,
            })
        } else {
            let new_session_id = format!("session_{}", uuid::Uuid::new_v4());
            serde_json::json!({
                "result": "New agent session created",
                "session_id": new_session_id,
                "prompt": prompt,
                "images": images,
            })
        };
        
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_agent_tool_serialization() {
        let input_args = AgentTool::default_input_args();
        let output_arg = ToolOutputArg::new("string".to_string(), "Agent response".to_string());
        
        let agent_tool = AgentTool::new(
            "1.0.0".to_string(),
            "TestAgent".to_string(),
            "Test Author".to_string(),
            "Test Description".to_string(),
            vec!["agent".to_string(), "test".to_string()],
            input_args,
            output_arg,
            vec![],
            RunnerType::Any,
            vec![OperatingSystem::Linux],
            None,
            None,
            None,
        );
        
        let json = agent_tool.to_json();
        let deserialized = AgentTool::from_json(&json).unwrap();
        
        assert_eq!(agent_tool, deserialized);
    }
    
    #[test]
    fn test_default_input_args() {
        let params = AgentTool::default_input_args();
        
        assert!(params.properties.contains_key("prompt"));
        assert!(params.required.contains(&"prompt".to_string()));
        
        assert!(params.properties.contains_key("images"));
        assert!(!params.required.contains(&"images".to_string()));
        
        assert!(params.properties.contains_key("session_id"));
        assert!(!params.required.contains(&"session_id".to_string()));
    }
}
