use std::collections::{HashMap, HashSet};

use crate::utils::github_mcp::{
    extract_command_from_json_config_in_readme, extract_env_vars_from_smithery_yaml, extract_mcp_env_vars_from_readme, fetch_github_file, parse_github_url, GitHubRepo
};
use reqwest::Client;
use rmcp::model::Tool;
use serde_json::Value;
use shinkai_http_api::api_v2::api_v2_handlers_mcp_servers::AddMCPServerRequest;
use shinkai_message_primitives::schemas::mcp_server::MCPServerType;
use shinkai_tools_primitives::tools::{
    mcp_server_tool::MCPServerTool, parameters::{Parameters, Property}, shinkai_tool::ShinkaiTool, tool_config::ToolConfig, tool_output_arg::ToolOutputArg, tool_types::ToolResult
};
use toml::Table;

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
    let tool_name = tool
        .name
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
    let tool_router_key =
        MCPServerTool::create_tool_router_key(node_name.to_string(), server_id.to_string(), tool_name.to_string());
    let mcp_tool = MCPServerTool {
        name: format!("{} - {}", server_name, tool_name),
        author: node_name.to_string(),
        description: tool.description.to_string(),
        config: tools_config,
        activated: true,
        tool_router_key: Some(tool_router_key),
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
            properties: serde_json::json!({
                "content": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {"type": "string", "description": "Content type", "enum": ["text", "image", "audio"]},
                            "text": {"type": "string", "description": "Text content"},
                            "data": {"type": "string", "description": "Image content"},
                            "mimeType": {"type": "string", "description": "Mime type of the content"},
                        },
                        "required": ["type"]
                    }
                },
                "isError": {"type": "boolean", "description": "Whether the tool call was successful"}
            }),
            required: vec!["content".to_string(), "isError".to_string()],
        },
        tool_set: Some(format!("__mcp{}_{}", server_id.to_string(), server_name)),
    };

    // Return the ShinkaiTool
    ShinkaiTool::MCPServer(mcp_tool, true)
}

fn sanitize_mcp_server_name(name: String) -> String {
    name.replace("@", "")
        .trim()
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '_', "_")
}

async fn process_python_mcp_project(
    pyproject_toml_content: String,
    _repo_info: &GitHubRepo,
    env_vars: HashSet<String>,
    readme_content: Option<String>,
) -> Result<AddMCPServerRequest, String> {
    // Parse pyproject.toml
    let pyproject_toml: Table = pyproject_toml_content
        .parse::<Table>()
        .map_err(|e| format!("Failed to parse pyproject.toml: {}", e))?;

    // Extract package name
    let project = pyproject_toml
        .get("project")
        .ok_or_else(|| "Missing 'project' section in pyproject.toml".to_string())?
        .as_table()
        .ok_or_else(|| "Invalid 'project' section in pyproject.toml".to_string())?;

    let package_name = project
        .get("name")
        .ok_or_else(|| "Missing 'name' field in pyproject.toml".to_string())?
        .as_str()
        .ok_or_else(|| "Invalid 'name' field in pyproject.toml".to_string())?
        .to_string();

    // Check for project.scripts section to determine entry point
    let entry_point = if let Some(scripts) = project.get("scripts").and_then(|v| v.as_table()) {
        // Use the first script as entry point
        if !scripts.is_empty() {
            let script_name = scripts.keys().next().unwrap();
            Some(script_name.to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Create server name from package name
    let server_name = format!("{}", package_name);

    // Create environment variables map from extracted env vars
    let mut env_map = HashMap::new();
    for var_name in env_vars {
        env_map.insert(var_name.clone(), "".to_string());
    }

    // First try to extract command from README JSON config
    let command: String = if let Some(ref readme) = readme_content {
        if let Some(extracted_command) = extract_command_from_json_config_in_readme(readme) {
            extracted_command
        } else {
            // Fallback to original logic
            if let Some(script) = entry_point {
                format!("uvx run {}", script)
            } else {
                // Fallback to python -m if no script found
                format!("python -m {}", package_name.replace("-", "_"))
            }
        }
    } else {
        // No README available, use original logic
        if let Some(script) = entry_point {
            format!("uvx run {}", script)
        } else {
            // Fallback to python -m if no script found
            format!("python -m {}", package_name.replace("-", "_"))
        }
    };

    let request = AddMCPServerRequest {
        name: sanitize_mcp_server_name(server_name),
        r#type: MCPServerType::Command,
        url: None,
        command: Some(command),
        env: Some(env_map),
        is_enabled: true,
    };

    return Ok(request);
}

async fn process_nodejs_mcp_project(
    package_json_content: String,
    _repo_info: &GitHubRepo,
    env_vars: HashSet<String>,
    readme_content: Option<String>,
) -> Result<AddMCPServerRequest, String> {
    // Parse package.json
    let package_json: Value =
        serde_json::from_str(&package_json_content).map_err(|e| format!("Failed to parse package.json: {}", e))?;

    // Extract package name
    let package_name = package_json
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'name' field in package.json".to_string())?
        .to_string();

    // Create server name from package name
    let server_name = format!("{}", package_name);

    // Create environment variables map from extracted env vars
    let mut env_map = HashMap::new();
    for var_name in env_vars {
        env_map.insert(var_name.clone(), "".to_string());
    }

    // First try to extract command from README JSON config
    let command: String = if let Some(ref readme) = readme_content {
        if let Some(extracted_command) = extract_command_from_json_config_in_readme(readme) {
            extracted_command
        } else {
            // Fallback to original logic
            format!("npx -y {}", package_name)
        }
    } else {
        // No README available, use original logic
        format!("npx -y {}", package_name)
    };

    // Create registration request
    let request = AddMCPServerRequest {
        name: sanitize_mcp_server_name(server_name),
        r#type: MCPServerType::Command,
        url: None,
        command: Some(command),
        env: Some(env_map),
        is_enabled: true,
    };

    Ok(request)
}

pub async fn import_mcp_server_from_github_url(github_url: String) -> Result<AddMCPServerRequest, String> {
    let repo_info = parse_github_url(&github_url)?;

    let client = Client::builder()
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Fetch README.md content (used for both env vars fallback and command extraction)
    let readme_result = fetch_github_file(&client, &repo_info.owner, &repo_info.repo, "README.md").await;
    let readme_content = readme_result.ok();

    // Try to fetch smithery.yaml first for environment variables
    let mut env_vars = HashSet::new();
    let smithery_result = fetch_github_file(&client, &repo_info.owner, &repo_info.repo, "smithery.yaml").await;

    if let Ok(smithery_content) = smithery_result {
        env_vars = extract_env_vars_from_smithery_yaml(&smithery_content);
    } else if let Some(ref readme) = readme_content {
        // Fallback to README.md regex extraction for environment variables
        env_vars = extract_mcp_env_vars_from_readme(readme);
    } else {
        log::info!("Neither smithery.yaml nor README.md found for environment variable extraction");
    }

    // Try to fetch package.json first (Node.js project)
    let package_json_result = fetch_github_file(&client, &repo_info.owner, &repo_info.repo, "package.json").await;

    if let Ok(package_json_content) = package_json_result {
        return process_nodejs_mcp_project(package_json_content, &repo_info, env_vars, readme_content).await;
    }

    // If package.json not found, try pyproject.toml (Python project)
    let pyproject_toml_result = fetch_github_file(&client, &repo_info.owner, &repo_info.repo, "pyproject.toml").await;

    if let Ok(pyproject_toml_content) = pyproject_toml_result {
        return process_python_mcp_project(pyproject_toml_content, &repo_info, env_vars, readme_content).await;
    }

    // If neither found, return error
    Err(format!(
        "Could not find package.json or pyproject.toml in repository {}/{}",
        repo_info.owner, repo_info.repo
    ))
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
            assert_eq!(mcp_tool.name, "test_server - get_info");
            assert_eq!(mcp_tool.author, "test_node");
            assert_eq!(mcp_tool.description, "Retrieves information without modifying anything");
            assert_eq!(mcp_tool.mcp_server_ref, "test_server_123");
            assert_eq!(mcp_tool.mcp_server_tool, "get_info");
            assert_eq!(mcp_tool.tool_set, Some("__mcptest_server_123_test_server".to_string()));
            assert_eq!(enabled, true);

            // Check that properties were correctly extracted
            assert_eq!(mcp_tool.input_args.required.len(), 1);
            assert_eq!(mcp_tool.input_args.required[0], "id");
            assert!(mcp_tool.input_args.properties.contains_key("id"));

            // Verify config was properly set
            assert_eq!(mcp_tool.config.len(), 1);
            let ToolConfig::BasicConfig(basic_config) = &mcp_tool.config[0];
            assert_eq!(basic_config.key_name, "api_key");
        } else {
            panic!("Expected ShinkaiTool::MCPServer variant");
        }
    }

    #[tokio::test]
    async fn test_import_mcp_server_from_github_url_nodejs() {
        let github_url = "https://github.com/dcSpark/mcp-server-helius".to_string();
        let result = import_mcp_server_from_github_url(github_url).await;

        assert!(result.is_ok(), "Import failed: {:?}", result.err());
        let request = result.unwrap();

        assert_eq!(request.name, "mcp_dockmaster_mcp_server_helius");
        assert_eq!(request.r#type, MCPServerType::Command);
        assert_eq!(request.url, None);
        assert_eq!(
            request.command,
            Some("npx -y @mcp-dockmaster/mcp-server-helius".to_string())
        );
        assert!(request.env.is_some());
        let env_map = request.env.unwrap();
        assert_eq!(env_map.get("HELIUS_API_KEY"), Some(&"".to_string()));
        assert_eq!(request.is_enabled, true);
    }
}
