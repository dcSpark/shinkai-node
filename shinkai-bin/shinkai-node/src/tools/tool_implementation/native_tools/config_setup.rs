use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg};
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::tool_config::ToolConfig;

use ed25519_dalek::SigningKey;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_implementation::native_tools::llm_prompt_processor::LlmPromptProcessorTool;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;

use async_trait::async_trait;
use tokio::sync::Mutex;

pub struct ConfigSetupTool {
    pub tool: ShinkaiToolHeader,
    pub tool_embedding: Option<Vec<f32>>,
}

impl ConfigSetupTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai Tool Config Updater".to_string(),
                description: r#"Tool for updating the configuration of a tool. 
This tool allows you to update config fields of a tool by providing the tool_router_key and config key-value pairs.

Example usage:
{
    "tool_router_key": "local:::deno_toolkit:::my_tool",
    "config": {
            "api_key": "some-api-key",
            "api_secret": "some-api-secret"
    }
}"#.to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_tool_config_updater".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Update ShinkaiTool configurations".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                mcp_enabled: Some(false),
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("tool_router_key".to_string(), "string".to_string(), "The tool_router_key of the tool to update".to_string(), true, None);
                    params.add_property("config".to_string(), "object".to_string(), "Configuration key-value pairs to update".to_string(), true, None);
                    params
                },
                output_arg: ToolOutputArg {
                    json: r#"{"type": "object", "properties": {"success": {"type": "boolean"}, "message": {"type": "string"}}}"#.to_string(),
                },
                config: None,
                usage_type: None,
                tool_offering: None,
            },
            tool_embedding: None,
        }
    }
}

async fn config_update(
    tool_router_key: String,
    config: &Map<String, Value>,
    db_clone: Arc<SqliteManager>,
) -> Result<Value, ToolError> {
    // Get the existing tool
    let mut tool = db_clone
        .get_tool_by_key(&tool_router_key)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get tool by key: {}", e)))?;

    let mut update_count = 0;
    // Convert config object to Vec<ToolConfig>
    let mut new_configs = tool.get_config();
    for (key, value) in config {
        new_configs
            .iter_mut()
            .find(|config| {
                let ToolConfig::BasicConfig(basic_config) = config;
                basic_config.key_name == key.to_string()
            })
            .map(|config| {
                let ToolConfig::BasicConfig(basic_config) = config;
                update_count += 1;
                if value.is_null() {
                    basic_config.key_value = None;
                } else {
                    basic_config.key_value = Some(value.clone());
                }
            });
    }

    if update_count == 0 {
        return Err(ToolError::ExecutionError("No config fields were updated".to_string()));
    }
    // Update the tool's config based on its type
    match &mut tool {
        shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool::Deno(deno_tool, _) => {
            deno_tool.config = new_configs;
        }
        shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool::Python(python_tool, _) => {
            python_tool.config = new_configs;
        }
        _ => {
            return Err(ToolError::ExecutionError(
                "Config update is only supported for Deno and Python tools".to_string(),
            ))
        }
    }
    println!("tool config: {:?}", tool.get_config());
    // Update the tool in the database
    db_clone
        .update_tool(tool)
        .await
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    Ok(json!({
        "success": true,
        "message": format!("Successfully updated config for tool {}", tool_router_key)
    }))
}

async fn select_tool_router_key_from_intent(
    tool_router_key: String,
    bearer: String,
    tool_id: String,
    app_id: String,
    db: Arc<SqliteManager>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
    config: &Map<String, Value>,
    llm_provider: String,
) -> Result<String, ToolError> {
    let all_tool_headers = db
        .get_all_tool_headers()
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get all tool headers: {}", e)))?;

    let mut list_of_tools: String = "".to_string();
    all_tool_headers.iter().for_each(|tool| {
        let tool = db.get_tool_by_key(&tool.tool_router_key);
        if tool.is_err() {
            return;
        }
        let tool = tool.unwrap();
        let tool_config = tool.get_config();
        if tool_config.len() > 0 {
            list_of_tools.push_str(&format!(
                "#{} tool_router_key: {} ; tool_description: {} ; tool_config_keys: {} ;\n ",
                tool_config.len() + 1,
                tool.tool_router_key().to_string_without_version(),
                tool.description(),
                tool_config
                    .iter()
                    .map(|c| {
                        let ToolConfig::BasicConfig(basic_config) = c;
                        basic_config.key_name.clone()
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
        }
    });

    let mut parameters = Map::new();
    parameters.insert(
        "prompt".to_string(),
        json!(format!(
            "List of all tools:
<tools>
{}
</tools>

<instruction>
* 'tool_router_key' have the following format: 'aaa:::bbb:::ccc', exactly 3 sections separated by three triple colons ':::' exactly as above.
* The intent tag has a command name that tries to target one of the tools listed above.
* Select the 'tool_router_key' of the most relevant tool to update given the user intent tag.
* IMPORTANT: Select and write a complete 'tool_router_key' name, and no other text.
* IMPORTANT: The 'tool_router_key' must be exactly as listed above, with no additional text, changes, or formatting.
</instruction>

<intent>
{}, {}
</intent>
",
            list_of_tools,
            tool_router_key,
            config
                .into_iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<String>>()
                .join(", ")
        )),
    );
    let result = LlmPromptProcessorTool::execute(
        bearer,
        tool_id,
        app_id,
        db.clone(),
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
        &parameters,
        llm_provider,
    )
    .await?;

    let tool_router_key = result.get("message").and_then(|v| v.as_str()).unwrap_or("");
    println!("found tool_router_key: {}", tool_router_key);

    // Extract tool key with format aaa:::bbb:::ccc using regex
    let re = regex::Regex::new(r"[-\w]*:::[-\w]*:::[-\w]*").unwrap();
    let tool_router_key = re.find(tool_router_key).map(|m| m.as_str()).unwrap_or(tool_router_key);
    let tool = db
        .get_tool_by_key(&tool_router_key)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get tool by key: {}", e)))?;

    let real_config = tool.get_config();
    let mut real_config_keys = "".to_string();
    real_config.iter().for_each(|c| {
        let ToolConfig::BasicConfig(basic_config) = c;
        real_config_keys.push_str(&format!("{}: {}\n", basic_config.key_name, basic_config.description));
    });
    let message = format!(
        "The tool function command was NOT applied. To apply it, call tool 'Shinkai Tool Config Updater' again with the following parameters:
'tool_router_key': {}
'config': {}",
        tool_router_key, real_config_keys
    );
    println!("Retry prompt message: {}", message);
    Ok(message)
}

#[async_trait]
impl ToolExecutor for ConfigSetupTool {
    async fn execute(
        bearer: String,
        tool_id: String,
        app_id: String,
        db: Arc<SqliteManager>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        llm_provider: String,
    ) -> Result<Value, ToolError> {
        let tool_router_key = parameters
            .get("tool_router_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::ExecutionError("tool_router_key parameter is required".to_string()))?;

        let config: &Map<String, Value> = parameters.get("config").and_then(|v| v.as_object()).ok_or_else(|| {
            ToolError::ExecutionError("config parameter is required and must be an object".to_string())
        })?;

        // Try to update the tool config,
        // If it fails, we will try to find the tool key using the LLM
        let result = config_update(tool_router_key.to_string(), config, db.clone()).await;
        if result.is_ok() {
            return result;
        }

        // If it fails, we will try to find the tool key using the LLM
        let message = select_tool_router_key_from_intent(
            tool_router_key.to_string(),
            bearer,
            tool_id,
            app_id,
            db,
            node_name_clone,
            identity_manager_clone,
            job_manager_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
            config,
            llm_provider,
        )
        .await?;

        // NOTE: "Invalid function arguments" makes the LLM Retry
        Err(ToolError::ExecutionError(format!(
            "[Invalid function arguments] {}",
            message
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_tools_primitives::tools::tool_config::BasicConfig;
    use shinkai_tools_primitives::tools::tool_types::{OperatingSystem, RunnerType, ToolResult};
    use shinkai_tools_primitives::tools::{deno_tools::DenoTool, shinkai_tool::ShinkaiTool};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[test]
    fn test_tool_router_key() {
        let config_setup_tool = ConfigSetupTool::new();
        assert_eq!(
            config_setup_tool.tool.tool_router_key,
            "local:::__official_shinkai:::shinkai_tool_config_updater"
        );
    }

    fn create_deno_tool() -> ShinkaiTool {
        let mut initial_tool = ShinkaiTool::Deno(
            DenoTool {
                name: "Test Tool".to_string(),
                homepage: Some("http://127.0.0.1/index.html".to_string()),
                author: "Test Author".to_string(),
                version: "1.0.0".to_string(),
                mcp_enabled: Some(false),
                js_code: "console.log('test');".to_string(),
                tools: vec![],
                config: vec![
                    ToolConfig::BasicConfig(BasicConfig {
                        key_name: "api_key".to_string(),
                        description: "API Key".to_string(),
                        required: true,
                        type_name: Some("string".to_string()),
                        key_value: Some(serde_json::Value::String("old_key".to_string())),
                    }),
                    ToolConfig::BasicConfig(BasicConfig {
                        key_name: "secret".to_string(),
                        description: "Secret".to_string(),
                        required: false,
                        type_name: Some("string".to_string()),
                        key_value: Some(serde_json::Value::String("old_secret".to_string())),
                    }),
                ],
                oauth: None,
                description: "Test tool".to_string(),
                keywords: vec![],
                input_args: Parameters::new(),
                activated: true,
                embedding: None,
                result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                output_arg: ToolOutputArg::empty(),
                sql_tables: None,
                sql_queries: None,
                file_inbox: None,
                assets: None,
                runner: RunnerType::Any,
                operating_system: vec![OperatingSystem::Windows],
                tool_set: None,
            },
            true,
        );
        initial_tool.set_embedding(vec![0.0; 384]);
        initial_tool
    }

    #[tokio::test]
    async fn test_update_tool_config() {
        // Setup test database
        let db = Arc::new(setup_test_db());

        // Create a test Deno tool with initial config
        let initial_tool = create_deno_tool();
        let tool_router_key = initial_tool
            .tool_router_key()
            .to_string_without_version()
            .to_lowercase();

        // Add tool to database
        db.add_tool(initial_tool).await.unwrap();

        // Create parameters for config update
        let mut parameters = Map::new();
        parameters.insert("tool_router_key".to_string(), json!(tool_router_key));
        parameters.insert("config".to_string(), json!({"api_key": "new_key"}));

        let config = parameters.get("config").unwrap().as_object().unwrap();
        let result = config_update(tool_router_key.clone(), config, db.clone()).await;
        assert!(result.is_ok());

        // Verify the config was updated
        let updated_tool = db.get_tool_by_key(&tool_router_key).unwrap();
        match updated_tool {
            ShinkaiTool::Deno(deno_tool, _) => {
                assert_eq!(deno_tool.config.len(), 2);
                let ToolConfig::BasicConfig(config1) = &deno_tool.config[0];
                let ToolConfig::BasicConfig(config2) = &deno_tool.config[1];
                assert_eq!(config1.key_name, "api_key");
                assert_eq!(config1.description, "API Key");
                assert!(config1.required);
                assert_eq!(config1.type_name, Some("string".to_string()));
                assert_eq!(
                    config1.key_value,
                    Some(serde_json::Value::String("new_key".to_string()))
                );
                assert_eq!(config2.key_name, "secret");
                assert_eq!(config2.description, "Secret");
                assert!(!config2.required);
                assert_eq!(config2.type_name, Some("string".to_string()));
                assert_eq!(
                    config2.key_value,
                    Some(serde_json::Value::String("old_secret".to_string()))
                );
            }
            _ => panic!("Expected Deno tool"),
        }
    }

    #[tokio::test]
    async fn test_update_tool_config_no_changes() {
        // Setup test database
        let db = Arc::new(setup_test_db());

        // Create a test Deno tool with initial config
        let initial_tool = create_deno_tool();
        let tool_router_key = initial_tool
            .tool_router_key()
            .to_string_without_version()
            .to_lowercase();

        // Add tool to database
        db.add_tool(initial_tool).await.unwrap();

        // Create parameters for config update with non-existent field
        let mut parameters = Map::new();
        parameters.insert("tool_router_key".to_string(), json!(tool_router_key));
        parameters.insert("config".to_string(), json!({"non_existent_field": "some_value"}));

        let config = parameters.get("config").unwrap().as_object().unwrap();
        let result = config_update(tool_router_key.clone(), config, db.clone()).await;
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.to_string().contains("No config fields were updated"));
        }
    }
}
