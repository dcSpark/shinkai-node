use rmcp::model::Tool;
use shinkai_tools_primitives::tools::{
    mcp_server_tool::MCPServerTool, parameters::{Parameters, Property}, shinkai_tool::ShinkaiTool, tool_config::ToolConfig, tool_output_arg::ToolOutputArg, tool_types::ToolResult
};

/// Converts an rmcp Tool to a ShinkaiTool::MCPServer
pub fn convert_to_shinkai_tool(
    tool: &Tool,
    server_name: &str,
    server_id: &str,
    node_name: &str,
    tools_config: Vec<ToolConfig>,
) -> ShinkaiTool {
    // Extract properties map from the tool's input schema
    let props_map: std::collections::HashMap<String, Property> = tool
        .input_schema
        .get("properties")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Extract required fields
    let req_vec: Vec<String> = tool
        .input_schema
        .get("required")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Create the MCPServerTool
    let mcp_tool = MCPServerTool {
        name: format!("{}_{}", server_name, tool.name),
        author: node_name.to_string(),
        description: tool.description.to_string(),
        config: tools_config,
        activated: true,
        input_args: Parameters {
            schema_type: tool
                .input_schema
                .get("type")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| "object".to_string()),
            properties: props_map,
            required: req_vec,
        },
        keywords: vec![],
        version: "1.0.0".to_string(),
        embedding: None,
        mcp_enabled: Some(false),
        mcp_server_ref: server_id.to_string(),
        mcp_server_tool: tool.name.to_string(),
        mcp_server_url: "".to_string(),
        output_arg: ToolOutputArg::empty(),
        result: ToolResult {
            r#type: "object".to_string(),
            properties: serde_json::json!({}),
            required: vec![],
        },
        tool_set: Some(format!("__mcp_{}", server_name)),
    };

    // Return the ShinkaiTool
    ShinkaiTool::MCPServer(mcp_tool, true)
}

#[cfg(test)]
pub mod tests_mcp_manager {
    use super::*;
    use rmcp::model::Tool;
    use serde_json::json;
    use shinkai_tools_primitives::tools::tool_config::BasicConfig;
    use std::sync::Arc;

    /// Creates mock Tool objects for testing purposes
    pub fn mock_tools() -> Vec<Tool> {
        vec![
            // A simple read-only tool
            Tool::new(
                "get_info",
                "Retrieves information without modifying anything",
                Arc::new(
                    serde_json::from_value(json!({
                        "type": "object",
                        "properties": {
                            "id": {"type": "string", "description": "ID to look up"}
                        },
                        "required": ["id"]
                    }))
                    .unwrap(),
                ),
            ),
            // A destructive tool that modifies state
            Tool::new(
                "update_data",
                "Updates data in the system",
                Arc::new(
                    serde_json::from_value(json!({
                        "type": "object",
                        "properties": {
                            "id": {"type": "string", "description": "ID of record to update"},
                            "value": {"type": "string", "description": "New value"}
                        },
                        "required": ["id", "value"]
                    }))
                    .unwrap(),
                ),
            ),
            // An idempotent tool
            Tool::new(
                "create_if_not_exists",
                "Creates a resource if it doesn't already exist",
                Arc::new(
                    serde_json::from_value(json!({
                        "type": "object",
                        "properties": {
                            "name": {"type": "string", "description": "Name of the resource"},
                            "config": {"type": "object", "description": "Configuration options"}
                        },
                        "required": ["name"]
                    }))
                    .unwrap(),
                ),
            ),
        ]
    }

    #[test]
    fn test_convert_to_shinkai_tool() {
        let mock_tools_vec = mock_tools();
        let tool = mock_tools_vec.first().unwrap();
        let server_name = "test_server";
        let server_id = "test_server_123";
        let node_name = "test_node";
        let tools_config = vec![ToolConfig::BasicConfig(BasicConfig {
            key_name: "api_key".to_string(),
            description: "API Key for testing".to_string(),
            required: true,
            type_name: Some("string".to_string()),
            key_value: Some(serde_json::Value::String("test_key".to_string())),
        })];

        let shinkai_tool = convert_to_shinkai_tool(tool, server_name, server_id, node_name, tools_config);

        if let ShinkaiTool::MCPServer(mcp_tool, enabled) = shinkai_tool {
            assert_eq!(mcp_tool.name, "test_server_get_info");
            assert_eq!(mcp_tool.author, "test_node");
            assert_eq!(mcp_tool.description, "Retrieves information without modifying anything");
            assert_eq!(mcp_tool.mcp_server_ref, "test_server_123");
            assert_eq!(mcp_tool.mcp_server_tool, "get_info");
            assert_eq!(mcp_tool.tool_set, Some("__mcp_test_server".to_string()));
            assert_eq!(enabled, true);

            // Check that properties were correctly extracted
            assert_eq!(mcp_tool.input_args.required.len(), 1);
            assert_eq!(mcp_tool.input_args.required[0], "id");
            assert!(mcp_tool.input_args.properties.contains_key("id"));

            // Verify config was properly set
            assert_eq!(mcp_tool.config.len(), 1);
            if let ToolConfig::BasicConfig(basic_config) = &mcp_tool.config[0] {
                assert_eq!(basic_config.key_name, "api_key");
            } else {
                panic!("Expected BasicConfig");
            }
        } else {
            panic!("Expected ShinkaiTool::MCPServer variant");
        }
    }
}
