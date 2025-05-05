use crate::{
    llm_provider::job_manager::JobManager,
    managers::{tool_router::ToolRouter, IdentityManager},
    network::{
        node_error::NodeError,
        node_shareable_logic::{download_zip_from_url, ZipFileContents},
        zip_export_import::zip_export_import::{generate_tool_zip, import_dependencies_tools, import_tool},
        Node,
    },
    tools::{
        tool_definitions::definition_generation::{generate_tool_definitions, get_all_tools},
        tool_execution::execution_coordinator::{execute_code, execute_mcp_tool_cmd, execute_tool_cmd},
        tool_generation::v2_create_and_send_job_message,
        tool_prompts::{generate_code_prompt, tool_metadata_implementation_prompt},
    },
    utils::environment::NodeEnvironment,
};
use async_channel::Sender;
use chrono::Utc;
use ed25519_dalek::{ed25519::signature::SignerMut, SigningKey};
use reqwest::StatusCode;
use serde_json::{json, Map, Value};
use shinkai_http_api::node_api_router::{APIError, SendResponseBodyData};
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName, indexable_version::IndexableVersion, job::JobLike, job_config::JobConfig,
        shinkai_name::ShinkaiSubidentityType, tool_router_key::ToolRouterKey,
    },
    schemas::{
        shinkai_name::ShinkaiName,
        shinkai_tools::{CodeLanguage, DynamicToolType},
    },
    shinkai_message::shinkai_message_schemas::JobMessage,
    shinkai_message::shinkai_message_schemas::{CallbackAction, JobCreationInfo, MessageSchemaType},
    shinkai_utils::{
        job_scope::MinimalJobScope, shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::clone_signature_secret_key,
    },
};
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::{
    deno_tools::DenoTool,
    error::ToolError,
    parameters::Parameters,
    python_tools::PythonTool,
    shinkai_tool::ShinkaiToolHeader,
    shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets},
    tool_config::{OAuth, ToolConfig},
    tool_output_arg::ToolOutputArg,
    tool_playground::{ToolPlayground, ToolPlaygroundMetadata},
    tool_types::{OperatingSystem, RunnerType, ToolResult},
};
use std::{collections::HashMap, path::PathBuf};
use std::{env, io::Read, sync::Arc, time::Instant};
use tokio::fs;
use tokio::{process::Command, sync::Mutex};
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

// Helper function to serialize Vec<ToolConfig> into the specific object format
fn serialize_tool_config_to_schema_and_form_data(configs: &Vec<ToolConfig>) -> Value {
    let mut schema_properties = Map::new();
    let mut schema_required = Vec::new();
    let mut config_form_data = Map::new();

    for config in configs {
        if let ToolConfig::BasicConfig(basic) = config {
            // --- Build Schema Part ---
            let mut property_details = Map::new();
            property_details.insert("description".to_string(), json!(basic.description));
            let type_value = basic
                .type_name
                .as_ref()
                .map_or_else(|| "string".to_string(), |t| t.clone());
            property_details.insert("type".to_string(), json!(type_value));
            schema_properties.insert(basic.key_name.clone(), Value::Object(property_details));

            if basic.required {
                schema_required.push(json!(basic.key_name.clone()));
            }

            // --- Build configFormData Part ---
            // Use json! macro which converts Option<String> to Value::String or Value::Null
            config_form_data.insert(basic.key_name.clone(), json!(basic.key_value));
        }
        // Note: Still ignores non-BasicConfig variants for both parts.
    }

    // Construct the final schema object
    let schema_object = json!({
        "type": "object",
        "properties": schema_properties,
        "required": schema_required
    });

    // Construct the final configFormData object
    let config_form_data_object = Value::Object(config_form_data);

    // Construct the final return object containing both parts
    json!({
        "schema": schema_object,
        "configFormData": config_form_data_object
    })
}

impl Node {
    /// Searches for Shinkai tools using both vector and full-text search (FTS)
    /// methods.
    ///
    /// The function returns a total of 10 results based on the following logic:
    /// 1. All FTS results are added first.
    /// 2. If there is a vector search result with a score under 0.2, it is added as the second result.
    /// 3. Remaining FTS results are added.
    /// 4. If there are remaining slots after adding FTS results, they are filled with additional vector search results.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference-counted, read-write lock on the SqliteManager.
    /// * `bearer` - A string representing the bearer token for authentication.
    /// * `query` - A string containing the search query.
    /// * `res` - A channel sender for sending the search results or errors.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the search operation.
    pub async fn v2_api_search_shinkai_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        query: String,
        agent_or_llm: Option<String>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Sanitize the query to handle special characters
        let sanitized_query = query.replace(|c: char| !c.is_alphanumeric() && c != ' ', " ");

        // Attempt to get the agent's tools if agent_or_llm is provided
        let allowed_tools = if let Some(agent_id) = agent_or_llm {
            match db.get_agent(&agent_id) {
                Ok(Some(agent)) => Some(agent.tools),
                Ok(None) | Err(_) => None,
            }
        } else {
            None
        };

        // Start the timer for logging purposes
        let start_time = Instant::now();

        // Start the timer for vector search
        let vector_start_time = Instant::now();

        // Use different search method based on whether we have allowed_tools
        let vector_search_result = if let Some(tools) = allowed_tools {
            // First generate the embedding from the query
            let embedding = db
                .generate_embeddings(&sanitized_query)
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

            // Then use the embedding with the limited search
            let tool_names = tools
                .iter()
                .map(|tool| tool.to_string_without_version())
                .collect::<Vec<String>>();
            db.tool_vector_search_with_vector_limited(embedding, 5, tool_names)
        } else {
            db.tool_vector_search(&sanitized_query, 5, false, true).await
        };

        let vector_elapsed_time = vector_start_time.elapsed();
        println!("Time taken for vector search: {:?}", vector_elapsed_time);

        // Start the timer for FTS search
        let fts_start_time = Instant::now();
        let fts_search_result = db.search_tools_fts(&sanitized_query);
        let fts_elapsed_time = fts_start_time.elapsed();
        println!("Time taken for FTS search: {:?}", fts_elapsed_time);

        match (vector_search_result, fts_search_result) {
            (Ok(vector_tools), Ok(fts_tools)) => {
                let mut combined_tools = Vec::new();
                let mut seen_ids = std::collections::HashSet::new();

                // Always add the first FTS result
                if let Some(first_fts_tool) = fts_tools.first() {
                    if seen_ids.insert(first_fts_tool.tool_router_key.clone()) {
                        combined_tools.push(first_fts_tool.clone());
                    }
                }

                // Check if the top vector search result has a score under 0.2
                if let Some((tool, score)) = vector_tools.first() {
                    if *score < 0.2 {
                        if seen_ids.insert(tool.tool_router_key.clone()) {
                            combined_tools.push(tool.clone());
                        }
                    }
                }

                // Add remaining FTS results
                for tool in fts_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                // Add remaining vector search results
                for (tool, _) in vector_tools.iter().skip(1) {
                    if seen_ids.insert(tool.tool_router_key.clone()) {
                        combined_tools.push(tool.clone());
                    }
                }

                // Serialize the combined results to JSON
                let tools_json = serde_json::to_value(combined_tools).map_err(|err| NodeError {
                    message: format!("Failed to serialize tools: {}", err),
                })?;

                // Log the elapsed time and result count if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    let elapsed_time = start_time.elapsed();
                    let result_count = tools_json.as_array().map_or(0, |arr| arr.len());
                    println!("Time taken for tool search: {:?}", elapsed_time);
                    println!("Number of tool results: {}", result_count);
                }
                let _ = res.send(Ok(tools_json)).await;
                Ok(())
            }
            (Err(err), _) | (_, Err(err)) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to search tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_list_all_shinkai_tools(
        db: Arc<SqliteManager>,
        bearer: String,
        node_name: ShinkaiName,
        category: Option<String>,
        tool_router: Option<Arc<ToolRouter>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all tools
        match db.get_all_tool_headers() {
            Ok(tools) => {
                // Group tools by their base key (without version)
                use std::collections::HashMap;
                let mut tool_groups: HashMap<String, Vec<ShinkaiToolHeader>> = HashMap::new();

                for tool in tools {
                    let tool_router_key = tool.tool_router_key.clone();
                    tool_groups.entry(tool_router_key).or_default().push(tool);
                }

                // For each group, keep only the tool with the highest version
                let mut latest_tools = Vec::new();
                for (_, mut group) in tool_groups {
                    if group.len() == 1 {
                        latest_tools.push(group.pop().unwrap());
                    } else {
                        // Sort by version in descending order
                        group.sort_by(|a, b| {
                            let a_version = IndexableVersion::from_string(&a.version.clone())
                                .unwrap_or(IndexableVersion::from_number(0));
                            let b_version = IndexableVersion::from_string(&b.version.clone())
                                .unwrap_or(IndexableVersion::from_number(0));
                            b_version.cmp(&a_version)
                        });

                        // Take the first one (highest version)
                        latest_tools.push(group.remove(0));
                    }
                }

                // Filter by category if provided
                // Downloaded -> anything else
                // Default Tools -> is default
                // System Tools -> Rust tools
                // My Tools -> author localhost.* or author == MY_ID
                let filtered_tools = if let Some(category) = category {
                    match category.to_lowercase().as_str() {
                        "downloaded" => {
                            // Get default tool keys as a HashSet for O(1) lookups if ToolRouter is provided
                            let default_tool_keys = if let Some(router) = &tool_router {
                                Some(router.get_default_tool_router_keys_as_set().await)
                            } else {
                                None
                            };

                            let node_name_string = node_name.get_node_name_string();

                            latest_tools
                                .into_iter()
                                .filter(|tool| {
                                    // Not default
                                    let is_not_default = if let Some(default_keys) = &default_tool_keys {
                                        !default_keys.contains(&tool.tool_router_key)
                                    } else {
                                        true // If we can't determine default tools, assume it's not default
                                    };

                                    // Author doesn't start with "localhost."
                                    let is_not_localhost = !tool.author.starts_with("localhost.");

                                    // Author is not the same as node_name.get_node_name_string()
                                    let is_not_node_name = tool.author != node_name_string;

                                    // Not a Rust tool
                                    let is_not_rust = !matches!(tool.tool_type.to_lowercase().as_str(), "rust");

                                    is_not_default && is_not_localhost && is_not_node_name && is_not_rust
                                })
                                .collect()
                        }
                        "default" => {
                            // Get default tool keys as a HashSet for O(1) lookups if ToolRouter is provided
                            let default_tool_keys = if let Some(router) = &tool_router {
                                Some(router.get_default_tool_router_keys_as_set().await)
                            } else {
                                None
                            };

                            if let Some(default_keys) = &default_tool_keys {
                                // Use O(1) lookup with HashSet
                                latest_tools
                                    .into_iter()
                                    .filter(|tool| default_keys.contains(&tool.tool_router_key))
                                    .collect()
                            } else {
                                // Fallback if ToolRouter not provided
                                latest_tools
                            }
                        }
                        "system" => latest_tools
                            .into_iter()
                            .filter(|tool| matches!(tool.tool_type.to_lowercase().as_str(), "rust"))
                            .collect(),
                        "my_tools" => {
                            let node_name_string = node_name.get_node_name_string();
                            latest_tools
                                .into_iter()
                                .filter(|tool| tool.author.starts_with("localhost.") || tool.author == node_name_string)
                                .collect()
                        }
                        _ => latest_tools, // If an unknown category is provided, return all tools
                    }
                } else {
                    latest_tools
                };

                let t = filtered_tools.iter().map(|tool| json!(tool)).collect();
                let _ = res.send(Ok(t)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_list_all_mcp_shinkai_tools(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        category: Option<String>,
        tool_router: Option<Arc<ToolRouter>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let _bearer = Self::get_bearer_token(db.clone(), &res).await?;

        // List all tools
        match db.get_all_tool_headers() {
            Ok(tools) => {
                // Group tools by their base key (without version)
                use std::collections::HashMap;
                let mut tool_groups: HashMap<String, Vec<ShinkaiToolHeader>> = HashMap::new();

                for tool in tools {
                    if tool.mcp_enabled.unwrap_or(false) {
                        let tool_router_key = tool.tool_router_key.clone();
                        tool_groups.entry(tool_router_key).or_default().push(tool);
                    }
                }

                // For each group, keep only the tool with the highest version
                let mut latest_tools = Vec::new();
                for (_, mut group) in tool_groups {
                    if group.len() == 1 {
                        latest_tools.push(group.pop().unwrap());
                    } else {
                        // Sort by version in descending order
                        group.sort_by(|a, b| {
                            let a_version = IndexableVersion::from_string(&a.version.clone())
                                .unwrap_or(IndexableVersion::from_number(0));
                            let b_version = IndexableVersion::from_string(&b.version.clone())
                                .unwrap_or(IndexableVersion::from_number(0));
                            b_version.cmp(&a_version)
                        });

                        // Take the first one (highest version)
                        latest_tools.push(group.remove(0));
                    }
                }

                // Filter by category if provided
                // Downloaded -> anything else
                // Default Tools -> is default
                // System Tools -> Rust tools
                // My Tools -> author localhost.* or author == MY_ID
                let filtered_tools = if let Some(category) = category {
                    match category.to_lowercase().as_str() {
                        "downloaded" => {
                            // Get default tool keys as a HashSet for O(1) lookups if ToolRouter is provided
                            let default_tool_keys = if let Some(router) = &tool_router {
                                Some(router.get_default_tool_router_keys_as_set().await)
                            } else {
                                None
                            };

                            let node_name_string = node_name.get_node_name_string();

                            latest_tools
                                .into_iter()
                                .filter(|tool| {
                                    // If tool is not mcp enabled, return false
                                    if !tool.mcp_enabled.unwrap_or(false) {
                                        return false;
                                    }
                                    // Not default
                                    let is_not_default = if let Some(default_keys) = &default_tool_keys {
                                        !default_keys.contains(&tool.tool_router_key)
                                    } else {
                                        true // If we can't determine default tools, assume it's not default
                                    };

                                    // Author doesn't start with "localhost."
                                    let is_not_localhost = !tool.author.starts_with("localhost.");

                                    // Author is not the same as node_name.get_node_name_string()
                                    let is_not_node_name = tool.author != node_name_string;

                                    // Not a Rust tool
                                    let is_not_rust = !matches!(tool.tool_type.to_lowercase().as_str(), "rust");

                                    is_not_default && is_not_localhost && is_not_node_name && is_not_rust
                                })
                                .collect()
                        }
                        "default" => {
                            // Get default tool keys as a HashSet for O(1) lookups if ToolRouter is provided
                            let default_tool_keys = if let Some(router) = &tool_router {
                                Some(router.get_default_tool_router_keys_as_set().await)
                            } else {
                                None
                            };

                            if let Some(default_keys) = &default_tool_keys {
                                // Use O(1) lookup with HashSet
                                latest_tools
                                    .into_iter()
                                    .filter(|tool| default_keys.contains(&tool.tool_router_key))
                                    .collect()
                            } else {
                                // Fallback if ToolRouter not provided
                                latest_tools
                            }
                        }
                        "system" => latest_tools
                            .into_iter()
                            .filter(|tool| matches!(tool.tool_type.to_lowercase().as_str(), "rust"))
                            .collect(),
                        "my_tools" => {
                            let node_name_string = node_name.get_node_name_string();
                            latest_tools
                                .into_iter()
                                .filter(|tool| tool.author.starts_with("localhost.") || tool.author == node_name_string)
                                .collect()
                        }
                        _ => latest_tools, // If an unknown category is provided, return all tools
                    }
                } else {
                    latest_tools
                };
                let t = filtered_tools
                    .iter()
                    .filter(|tool| tool.mcp_enabled.unwrap_or(false))
                    .map(|tool| json!(tool))
                    .collect();
                let _ = res.send(Ok(t)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list mcp tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    /// Merges tool, handling both existing and new configs.
    /// This method supports modifying existing config items and adding new ones,
    /// but does not support deleting config items.
    pub fn merge_tool(existing_tool_value: &Value, input_value: &Value) -> Value {
        let mut merged_value = Self::merge_json(existing_tool_value.clone(), input_value.clone());

        if let Some(Value::Array(input_configs)) = input_value
            .get("content")
            .and_then(|v| v.as_array().and_then(|v| v.first()).and_then(|v| v.get("config")))
        {
            let existing_configs = existing_tool_value
                .get("content")
                .and_then(|v| v.as_array().and_then(|v| v.first()).and_then(|v| v.get("config")))
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .to_vec();
            let mut new_configs = existing_configs;

            // For each input config, either merge with existing or add new
            for input_config in input_configs {
                let input_key_name = input_config
                    .get("BasicConfig")
                    .and_then(|c| c.get("key_name"))
                    .and_then(|k| k.as_str());

                if let Some(key_name) = input_key_name {
                    // Try to find matching existing config
                    if let Some(existing_idx) = new_configs.iter().position(|c| {
                        c.get("BasicConfig")
                            .and_then(|c| c.get("key_name"))
                            .and_then(|k| k.as_str())
                            == Some(key_name)
                    }) {
                        // Merge with existing config
                        new_configs[existing_idx] =
                            Self::merge_json(new_configs[existing_idx].clone(), input_config.clone());
                    } else {
                        // Add new config
                        new_configs.push(input_config.clone());
                    }
                }
            }

            // Update the merged value with new configs
            if let Some(content_array) = merged_value.get_mut("content").and_then(|v| v.as_array_mut()) {
                if let Some(first_content) = content_array.get_mut(0) {
                    if let Some(obj) = first_content.as_object_mut() {
                        obj.insert("config".to_string(), Value::Array(new_configs));
                    }
                }
            }
        }

        merged_value
    }

    pub async fn v2_api_set_shinkai_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_router_key: String,
        input_value: Value,
        res: Sender<Result<ShinkaiTool, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the full tool from db
        let existing_tool = match db.get_tool_by_key(&tool_router_key) {
            Ok(tool) => tool,
            Err(SqliteManagerError::ToolNotFound(_)) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Tool not found in LanceShinkaiDb".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Convert existing_tool to Value
        let existing_tool_value = match serde_json::to_value(&existing_tool) {
            Ok(value) => value,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert existing tool to Value: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let merged_value = Self::merge_tool(&existing_tool_value, &input_value);

        // Convert merged_value to ShinkaiTool
        let merged_tool: ShinkaiTool = match serde_json::from_value(merged_value) {
            Ok(tool) => tool,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert merged Value to ShinkaiTool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Save the tool to the LanceShinkaiDb
        let save_result = db.update_tool(merged_tool).await;

        match save_result {
            Ok(tool) => {
                let _ = res.send(Ok(tool)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add tool to LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_add_shinkai_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        node_env: NodeEnvironment,
        new_tool_with_assets: ShinkaiToolWithAssets,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let new_tool = new_tool_with_assets.tool;
        let dependencies = new_tool.get_tools();
        for dependency in dependencies {
            let tool = db.get_tool_by_key(&dependency.to_string_without_version());
            if tool.is_err() {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Tool not found: {}", dependency.to_string_without_version()),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        let save_result = db.add_tool(new_tool).await;

        match save_result {
            Ok(tool) => {
                let tool_key = tool.tool_router_key();

                if let Some(assets) = new_tool_with_assets.assets {
                    if !assets.is_empty() {
                        let file_path = PathBuf::from(&node_env.node_storage_path.clone().unwrap_or_default())
                            .join(".tools_storage")
                            .join("tools")
                            .join(tool.tool_router_key().convert_to_path());
                        if !file_path.exists() {
                            let s = std::fs::create_dir_all(&file_path);
                            if s.is_err() {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Failed to create directory".to_string(),
                                    message: format!("Failed to create directory: {}", s.err().unwrap()),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                        // Create the assets
                        for asset in assets {
                            let asset_path = file_path.join(asset.file_name);
                            let asset_content = base64::decode(asset.data).unwrap();
                            let status = fs::write(asset_path, asset_content).await;
                            if status.is_err() {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Failed to create directory".to_string(),
                                    message: format!("Failed to create directory: {}", status.err().unwrap()),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                    }
                }

                let response = json!({ "status": "success", "message": format!("Tool added with key: {}", tool_key.to_string_without_version()) });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add tool to LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_shinkai_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_key: String,
        serialize_config: bool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool from the database using the tool_key directly
        match db.get_tool_by_key(&tool_key) {
            // Use tool_key directly
            Ok(tool) => {
                // Serialize the tool object to JSON value first.
                let mut response_value = match serde_json::to_value(&tool) {
                    Ok(val) => val,
                    Err(e) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize tool: {:?}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };

                // If serialize_config is true, replace the 'config' field
                if serialize_config {
                    if let Value::Object(ref mut map) = response_value {
                        // Get the original config Vec from the tool struct
                        let original_config = tool.get_config(); // Assumes ShinkaiTool implements GetConfig trait or similar
                                                                 // Serialize the config vector using the updated helper function
                        let serialized_config_data = serialize_tool_config_to_schema_and_form_data(&original_config); // Use new function name
                                                                                                                      // Replace the existing 'config' field in the JSON map with the new structure
                        if let Some(Value::Object(ref mut contents_map)) = map
                            .get_mut("content")
                            .and_then(|v| v.as_array_mut())
                            .and_then(|arr| arr.get_mut(0))
                        {
                            contents_map.insert(
                                "configurations".to_string(),
                                serialized_config_data.get("schema").unwrap().clone(),
                            );
                            contents_map.insert(
                                "configFormData".to_string(),
                                serialized_config_data.get("configFormData").unwrap().clone(),
                            );
                        }
                    }
                }
                // Otherwise, the default serialization (config as array) is used.

                let _ = res.send(Ok(response_value)).await;
                Ok(())
            }
            Err(SqliteManagerError::ToolNotFound(_)) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Tool not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get tool: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub fn merge_json(existing: Value, input: Value) -> Value {
        match (existing, input) {
            (Value::Object(mut existing_map), Value::Object(input_map)) => {
                for (key, input_value) in input_map {
                    let existing_value = existing_map.remove(&key).unwrap_or(Value::Null);
                    existing_map.insert(key, Self::merge_json(existing_value, input_value));
                }
                Value::Object(existing_map)
            }
            (Value::Array(mut existing_array), Value::Array(input_array)) => {
                for (i, input_value) in input_array.into_iter().enumerate() {
                    if i < existing_array.len() {
                        existing_array[i] = Self::merge_json(existing_array[i].take(), input_value);
                    } else {
                        existing_array.push(input_value);
                    }
                }
                Value::Array(existing_array)
            }
            (_, input) => input,
        }
    }

    pub async fn v2_api_set_playground_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        payload: ToolPlayground,
        node_env: NodeEnvironment,
        _tool_id: String,
        app_id: String,
        original_tool_key_path: Option<String>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let result = Self::set_playground_tool(db, payload, node_env, app_id, original_tool_key_path).await;
        let _ = match result {
            Ok(result) => res.send(Ok(result)).await,
            Err(err) => res.send(Err(err)).await,
        };
        return Ok(());
    }

    async fn set_playground_tool(
        db: Arc<SqliteManager>,
        payload: ToolPlayground,
        node_env: NodeEnvironment,
        app_id: String,
        original_tool_key_path: Option<String>,
    ) -> Result<Value, APIError> {
        let mut updated_payload = payload.clone();
        let dependencies = updated_payload.metadata.tools.clone().unwrap_or_default();
        for dependency in dependencies {
            let _ = db
                .get_tool_by_key(&dependency.to_string_without_version())
                .map_err(|e| APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get tool: {}", e),
                })?;
        }

        let shinkai_tool = match payload.language {
            CodeLanguage::Typescript => {
                let tool_router_key = ToolRouterKey::new(
                    "local".to_string(),
                    payload.metadata.author.clone(),
                    payload.metadata.name.clone(),
                    None,
                );
                let tool = DenoTool {
                    name: payload.metadata.name.clone(),
                    tool_router_key: Some(tool_router_key.clone()),
                    homepage: payload.metadata.homepage.clone(),
                    author: payload.metadata.author.clone(),
                    version: payload.metadata.version.clone(),
                    js_code: payload.code.clone(),
                    tools: payload.metadata.tools.clone().unwrap_or_default(),
                    config: payload.metadata.configurations.clone(),
                    oauth: payload.metadata.oauth.clone(),
                    description: payload.metadata.description.clone(),
                    keywords: payload.metadata.keywords.clone(),
                    input_args: payload.metadata.parameters.clone(),
                    output_arg: ToolOutputArg { json: "".to_string() },
                    activated: false, // TODO: maybe we want to add this as an option in the UI?
                    mcp_enabled: Some(false),
                    embedding: None,
                    result: payload.metadata.result,
                    sql_tables: Some(payload.metadata.sql_tables),
                    sql_queries: Some(payload.metadata.sql_queries),
                    file_inbox: None,
                    assets: payload.assets.clone(),
                    runner: payload.metadata.runner,
                    operating_system: payload.metadata.operating_system,
                    tool_set: payload.metadata.tool_set,
                };
                ShinkaiTool::Deno(tool, false)
            }
            CodeLanguage::Python => {
                let tool_router_key = ToolRouterKey::new(
                    "local".to_string(),
                    payload.metadata.author.clone(),
                    payload.metadata.name.clone(),
                    None,
                );

                let tool = PythonTool {
                    name: payload.metadata.name.clone(),
                    tool_router_key: Some(tool_router_key.clone()),
                    homepage: payload.metadata.homepage.clone(),
                    version: payload.metadata.version.clone(),
                    author: payload.metadata.author.clone(),
                    py_code: payload.code.clone(),
                    tools: payload.metadata.tools.clone().unwrap_or_default(),
                    config: payload.metadata.configurations.clone(),
                    oauth: payload.metadata.oauth.clone(),
                    description: payload.metadata.description.clone(),
                    keywords: payload.metadata.keywords.clone(),
                    input_args: payload.metadata.parameters.clone(),
                    output_arg: ToolOutputArg { json: "".to_string() },
                    activated: false, // TODO: maybe we want to add this as an option in the UI?
                    mcp_enabled: Some(false),
                    embedding: None,
                    result: payload.metadata.result,
                    sql_tables: Some(payload.metadata.sql_tables),
                    sql_queries: Some(payload.metadata.sql_queries),
                    file_inbox: None,
                    assets: payload.assets.clone(),
                    runner: payload.metadata.runner,
                    operating_system: payload.metadata.operating_system,
                    tool_set: payload.metadata.tool_set,
                };
                ShinkaiTool::Python(tool, false)
            }
        };

        updated_payload.tool_router_key = Some(shinkai_tool.tool_router_key().to_string_without_version());

        let mut delete_old_tool = false;
        if let Some(original_tool_key_path) = original_tool_key_path.clone() {
            let original_tool_key = ToolRouterKey::from_string(&original_tool_key_path)?;
            println!("old tool_key: {:?}", original_tool_key);
            delete_old_tool = original_tool_key.to_string_without_version()
                != shinkai_tool.tool_router_key().to_string_without_version();
        }
        println!("new tool_key: {:?}", updated_payload.tool_router_key);
        println!("delete_old_tool: {:?}", delete_old_tool);

        let storage_path = node_env.node_storage_path.unwrap_or_default();
        // Check all asset files exist in the {storage}/tool_storage/assets/{app_id}/
        let mut origin_path: PathBuf = PathBuf::from(storage_path.clone());
        origin_path.push(".tools_storage");
        origin_path.push("playground");
        origin_path.push(app_id);
        // Read all files from origin directory
        let origin_files = if origin_path.exists() {
            Some(std::fs::read_dir(&origin_path).map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to read origin directory: {}", e),
            })?)
        } else {
            None
        };
        // Create destination directory path
        let mut perm_file_path = PathBuf::from(storage_path.clone());
        perm_file_path.push(".tools_storage");
        perm_file_path.push("tools");
        perm_file_path.push(shinkai_tool.tool_router_key().convert_to_path());

        // Clear destination directory if it exists
        if perm_file_path.exists() {
            std::fs::remove_dir_all(&perm_file_path).map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to clear destination directory: {}", e),
            })?;
        }

        // Create destination directory
        std::fs::create_dir_all(&perm_file_path).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to create permanent storage directory: {}", e),
        })?;

        // Copy all files from origin to destination
        if let Some(origin_files) = origin_files {
            for entry in origin_files {
                let entry = entry.map_err(|e| APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to read directory entry: {}", e),
                })?;

                let file_name = entry.file_name();
                let mut dest_path = perm_file_path.clone();
                dest_path.push(&file_name);

                println!(
                    "copying {} to {}",
                    entry.path().to_string_lossy(),
                    dest_path.to_string_lossy()
                );

                std::fs::copy(entry.path(), dest_path).map_err(|e| APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to copy file {}: {}", file_name.to_string_lossy(), e),
                })?;
            }
        }

        // Create a longer-lived binding for the db clone
        let version = shinkai_tool.version_indexable()?;
        let version = Some(version);
        let exists = db
            .tool_exists(&shinkai_tool.tool_router_key().to_string_without_version(), version)
            .map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to check if tool exists: {}", e),
            })?;

        let tool = match exists {
            // Tool already exists, update it
            true => db.update_tool(shinkai_tool).await,
            // Add the tool to the LanceShinkaiDb
            false => db.add_tool(shinkai_tool.clone()).await,
        }
        .map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to add tool to SqliteManager: {}", e),
        })?;

        db.set_tool_playground(&updated_payload).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to save playground tool: {}", e),
        })?;

        // Let's delete the old tool if it exists
        if delete_old_tool {
            if let Some(original_tool_key_path) = original_tool_key_path {
                let original_tool_key = ToolRouterKey::from_string(&original_tool_key_path)?;
                println!(
                    "removing tool with key: {:?}",
                    original_tool_key.to_string_without_version()
                );
                let delete_tool_playground = db.remove_tool_playground(&original_tool_key.to_string_without_version());
                let delete_tool = db.remove_tool(&original_tool_key.to_string_without_version(), None);
                println!("remove tool playground: {:?}", delete_tool_playground);
                println!("remove tool: {:?}", delete_tool);
            }
        }
        // Return playground as Value
        let tool_json = serde_json::to_value(&tool).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to serialize tool to JSON: {}", e),
        })?;

        return Ok(json!({
            "shinkai_tool": tool_json,
            "metadata": updated_payload
        }));
    }

    pub async fn v2_api_list_playground_tools(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all playground tools
        match db.get_all_tool_playground() {
            Ok(tools) => {
                let response = json!(tools);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list playground tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_remove_playground_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Remove the playground tool from the SqliteManager
        let db_write = db;
        match db_write.remove_tool_playground(&tool_key) {
            Ok(_) => {
                // Also remove the underlying tool from the SqliteManager
                match db_write.remove_tool(&tool_key, None) {
                    Ok(_) => {
                        let response =
                            json!({ "status": "success", "message": "Tool and underlying data removed successfully" });
                        let _ = res.send(Ok(response)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to remove underlying tool: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove playground tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_playground_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the playground tool
        match db.get_tool_playground(&tool_key) {
            Ok(tool) => {
                let response = json!(tool);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(SqliteManagerError::ToolPlaygroundNotFound(_)) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Playground tool not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get playground tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    // ------------------------------------------------------------
    // TOOLS
    // ------------------------------------------------------------
    // TODO Check if this is needed.
    pub async fn get_tool_definitions(
        bearer: String,
        db: Arc<SqliteManager>,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let definitions = generate_tool_definitions(tools, language, db, false).await;
        match definitions {
            Ok(definitions) => {
                let mut map: Map<String, Value> = Map::new();
                definitions.into_iter().for_each(|(key, value)| {
                    map.insert(key, Value::String(value));
                });

                let _ = res.send(Ok(Value::Object(map))).await;
            }
            Err(e) => {
                let _ = res.send(Err(e)).await;
            }
        }
        Ok(())
    }

    pub async fn execute_tool(
        bearer: String,
        node_name: ShinkaiName,
        db: Arc<SqliteManager>,
        tool_router_key: String,
        parameters: Map<String, Value>,
        tool_id: String,
        app_id: String,
        agent_id: Option<String>,
        llm_provider: String,
        extra_config: Map<String, Value>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        mounts: Option<Vec<String>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert extra_config to Vec<ToolConfig> using basic_config_from_value
        let tool_configs = ToolConfig::basic_config_from_value(&Value::Object(extra_config));

        // Execute the tool directly
        let result = execute_tool_cmd(
            bearer,
            node_name,
            db,
            // vector_fs,
            tool_router_key.clone(),
            parameters,
            tool_id,
            app_id,
            agent_id,
            llm_provider,
            tool_configs,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
            mounts,
        )
        .await;

        match result {
            Ok(result) => {
                println!("[execute_command] Tool execution successful: {}", tool_router_key);
                let _ = res.send(Ok(result)).await;
            }
            Err(e) => {
                println!("[execute_command] Tool execution failed {}: {}", tool_router_key, e);
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Error executing tool: {}", e),
                    }))
                    .await;
            }
        }

        Ok(())
    }

    pub async fn execute_mcp_tool(
        node_name: ShinkaiName, // No Bearer token needed because this is an internal tool
        db: Arc<SqliteManager>,
        tool_router_key: String,
        parameters: Map<String, Value>,
        tool_id: String,
        app_id: String,
        agent_id: Option<String>,
        extra_config: Map<String, Value>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        mounts: Option<Vec<String>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let bearer = Self::get_bearer_token(db.clone(), &res).await?;
        // Convert extra_config to Vec<ToolConfig> using basic_config_from_value
        let tool_configs = ToolConfig::basic_config_from_value(&Value::Object(extra_config));
        let result = execute_mcp_tool_cmd(
            bearer,
            node_name,
            db,
            tool_router_key,
            parameters,
            tool_id,
            app_id,
            agent_id,
            tool_configs,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
            mounts,
        )
        .await;

        match result {
            Ok(result) => {
                let _ = res.send(Ok(result)).await;
            }
            Err(e) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Error executing tool: {}", e),
                    }))
                    .await;
            }
        }
        Ok(())
    }

    pub async fn run_execute_code(
        bearer: String,
        db: Arc<SqliteManager>,
        tool_type: DynamicToolType,
        code: String,
        tools: Vec<ToolRouterKey>,
        parameters: Map<String, Value>,
        extra_config: Map<String, Value>,
        oauth: Option<Vec<OAuth>>,
        tool_id: String,
        app_id: String,
        agent_id: Option<String>,
        llm_provider: String,
        node_name: ShinkaiName,
        mounts: Option<Vec<String>>,
        runner: Option<RunnerType>,
        operating_system: Option<Vec<OperatingSystem>>,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert extra_config to Vec<ToolConfig> using basic_config_from_value
        let tool_configs = ToolConfig::basic_config_from_value(&Value::Object(extra_config));

        // Convert oauth to Vec<ToolConfig> if you have a similar method for OAuth
        // let oauth_configs = ToolConfig::oauth_from_value(&Value::Object(oauth));

        // Execute the tool directly
        let result = execute_code(
            tool_type.clone(),
            code,
            tools,
            parameters,
            tool_configs,
            oauth,
            db,
            tool_id,
            app_id,
            agent_id,
            llm_provider,
            bearer,
            node_name,
            mounts,
            runner,
            operating_system,
            identity_manager_clone,
            job_manager_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
        )
        .await;

        match result {
            Ok(result) => {
                println!("[execute_command] Tool execution successful: {}", tool_type);
                let _ = res.send(Ok(result)).await;
            }
            Err(e) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Error executing tool: {}", e),
                    }))
                    .await;
            }
        }

        Ok(())
    }

    pub async fn generate_tool_fetch_query(
        bearer: String,
        db: Arc<SqliteManager>,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        code: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // This is used only to generate example prompts.
        // We only use the minimal number of tools to generate the prompts.
        let tool_definitions = match generate_tool_definitions(tools.clone(), language.clone(), db.clone(), true).await
        {
            Ok(definitions) => definitions,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let is_memory_required = tools.iter().any(|tool| {
            tool.to_string_without_version() == "local:::__official_shinkai:::shinkai_sqlite_query_executor"
        });
        let code_prompt =
            match generate_code_prompt(language.clone(), is_memory_required, "".to_string(), tool_definitions).await {
                Ok(prompt) => prompt,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to generate code prompt: {:?}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

        let metadata_prompt = match tool_metadata_implementation_prompt(
            language.clone(),
            code.clone(),
            tools.clone(),
            identity_manager.clone(),
        )
        .await
        {
            Ok(prompt) => prompt,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let all_tools: Vec<ToolRouterKey> = db
            .clone()
            .get_all_tool_headers()?
            .into_iter()
            .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                Ok(tool_router_key) => Some(tool_router_key),
                Err(_) => None,
            })
            .collect();
        let library_code = match generate_tool_definitions(all_tools, language.clone(), db.clone(), false).await {
            Ok(code) => code,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // This is used to generate the headers for the tool prompt.
        let header_code = match generate_tool_definitions(tools.clone(), language.clone(), db.clone(), true).await {
            Ok(code) => code,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let _ = res
            .send(Ok(json!({
                "availableTools": get_all_tools(db.clone()).await.into_iter().map(|tool| tool.tool_router_key).collect::<Vec<String>>(),
                "libraryCode": library_code.clone(),
                "headers": header_code.clone(),
                "codePrompt": code_prompt.clone(),
                "metadataPrompt": metadata_prompt.clone(),
            })))
            .await;
        Ok(())
    }

    async fn is_code_generator(
        db: Arc<SqliteManager>,
        job_id: &str,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
    ) -> bool {
        // Retrieve the job to get the llm_provider
        let llm_provider = match db.get_job_with_options(job_id, false) {
            Ok(job) => job.parent_agent_or_llm_provider_id.clone(),
            Err(_) => return false,
        };

        let main_identity = {
            let identity_manager = identity_manager_clone.lock().await;
            match identity_manager.get_main_identity() {
                Some(identity) => identity.clone(),
                None => return false,
            }
        };

        // Create a new job message
        let sender = match ShinkaiName::new(main_identity.get_full_identity_name()) {
            Ok(name) => name,
            Err(_) => return false,
        };

        match db.get_llm_provider(&llm_provider, &sender) {
            Ok(llm_provider) => {
                if let Some(llm_provider) = llm_provider {
                    let provider = llm_provider.get_provider_string();
                    let model = llm_provider.get_model_string().to_lowercase();
                    println!("provider: {}", provider);
                    println!("model: {}", model);

                    if provider == "shinkai-backend"
                        && (model == "code_generator" || model == "code_generator_no_feedback")
                    {
                        return true;
                    }
                }
            }
            Err(_) => return false,
        };
        return false;
    }

    pub async fn generate_tool_implementation(
        bearer: String,
        db: Arc<SqliteManager>,
        job_message: JobMessage,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        post_check: bool,
        raw: bool,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        // Note: Later (inside v2_job_message), we validate the bearer token again,
        // we do it here to make sure we have a valid bearer token at this point
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        // Generate tool definitions
        let is_code_generator =
            Self::is_code_generator(db.clone(), &job_message.job_id, identity_manager_clone.clone()).await;

        println!("is_code_generator: {}", is_code_generator);

        // If it's the code_generator - we get all the tools - as the code_generator decides which tools to use
        let tools = if is_code_generator {
            // Only this list will be passed as valid functions to the code generator.
            let valid_tool_list: Vec<String> = vec![
                "local:::__official_shinkai:::shinkai_llm_prompt_processor",
                "local:::__official_shinkai:::x_twitter_post",
                "local:::__official_shinkai:::duckduckgo_search",
                "local:::__official_shinkai:::x_twitter_search",
            ]
            .iter()
            .map(|t| t.to_string())
            .collect();
            let user_tools: Vec<String> = tools.iter().map(|tools| tools.to_string_with_version()).collect();
            let all_tool_headers = db.clone().get_all_tool_headers()?;
            all_tool_headers
                .into_iter()
                .map(|tool| ToolRouterKey::from_string(&tool.tool_router_key))
                .filter(|tool| tool.is_ok())
                .map(|tool| tool.unwrap())
                .filter(|tool| {
                    let t = tool.to_string_without_version();
                    user_tools.contains(&t) || valid_tool_list.contains(&t)
                })
                .collect::<Vec<ToolRouterKey>>()
        } else {
            // If its a code-generation prompt, we only use the minimal number of tools
            // to generate the prompts.
            tools.clone()
        };

        let tool_definitions = match generate_tool_definitions(tools.clone(), language.clone(), db.clone(), true).await
        {
            Ok(definitions) => definitions,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate tool definitions: {:?}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let prompt = job_message.content.clone();
        let is_memory_required = tools.clone().iter().any(|tool| {
            tool.to_string_without_version() == "local:::__official_shinkai:::shinkai_sqlite_query_executor"
        });

        // Determine the code generation prompt so we can update the message with the
        // custom prompt if required
        let generate_code_prompt = match raw {
            true => prompt,
            false => match generate_code_prompt(language.clone(), is_memory_required, prompt, tool_definitions).await {
                Ok(prompt) => prompt,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to generate code prompt: {:?}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            },
        };

        // Disable tools for this job
        if let Err(err) = Self::disable_tools_for_job(db.clone(), bearer.clone(), job_message.job_id.clone()).await {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: err,
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // We copy the job message and update the content with the custom prompt
        let mut job_message_clone = job_message.clone();
        job_message_clone.content = generate_code_prompt;

        if post_check {
            let callback_action =
                CallbackAction::ImplementationCheck(language.to_dynamic_tool_type().unwrap(), tools.clone());
            job_message_clone.callback = Some(Box::new(callback_action));
        }

        Node::v2_job_message(
            db,
            node_name_clone,
            identity_manager_clone,
            job_manager_clone,
            bearer,
            job_message_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
            Some(true),
            res,
        )
        .await
    }

    pub async fn generate_tool_metadata_implementation(
        bearer: String,
        job_id: String,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        db: Arc<SqliteManager>,
        node_name_clone: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // We can automatically extract the code (last message from the AI in the job
        // inbox) using the job_id
        let job = match db.get_job_with_options(&job_id, true) {
            Ok(job) => job,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve job: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Disable tools for this job
        if let Err(err) = Self::disable_tools_for_job(db.clone(), bearer.clone(), job_id.clone()).await {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: err,
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let last_message = {
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())?;
            let messages = match db.get_last_messages_from_inbox(inbox_name.to_string(), 2, None) {
                Ok(messages) => messages,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to retrieve last messages from inbox: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };
            if messages.len() < 2 {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Most likely the LLM hasn't processed the code task yet".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            };

            // Handle the last message safely
            if let Some(last_message) = messages.last().and_then(|msg| msg.last()) {
                // Use last_message here
                last_message.clone()
            } else {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Failed to retrieve the last message".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let code = match last_message.get_message_content() {
            Ok(code) => code,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve the last message content: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let language_str = match language.clone() {
            CodeLanguage::Typescript => "typescript",
            CodeLanguage::Python => "python",
        };
        let start_pattern = &format!("```{}", language_str);
        let end_pattern = "```";
        // Extract code from triple backticks if present
        let code = if code.contains(start_pattern) {
            let start = code.find(start_pattern).unwrap_or(0);
            let end = code[(start + start_pattern.len())..]
                .find(end_pattern)
                .map(|i| i + start + start_pattern.len())
                .unwrap_or(code.len());

            // Skip language identifier if present
            let content_start = if code[start..].starts_with(start_pattern) {
                start + start_pattern.len()
            } else {
                start
            };

            code[content_start..end].trim().to_string()
        } else {
            code
        };

        // Generate the implementation
        let mut metadata =
            match tool_metadata_implementation_prompt(language.clone(), code, tools, identity_manager.clone()).await {
                Ok(metadata) => metadata,
                Err(err) => {
                    let _ = res.send(Err(err)).await;
                    return Ok(());
                }
            };

        // We auto create a new job with the same configuration as the one from job_id
        let job_creation_info = JobCreationInfo {
            scope: job.scope().clone(),
            is_hidden: Some(job.is_hidden()),
            associated_ui: None,
        };

        let is_code_generator = Self::is_code_generator(db.clone(), &job_id, identity_manager.clone()).await;
        if is_code_generator {
            metadata = format!(
                r"{}

<job_id>{}-{}</job_id>
",
                metadata, node_name_clone.node_name, job_id
            );
        }

        match v2_create_and_send_job_message(
            bearer,
            job_creation_info,
            job.parent_agent_or_llm_provider_id.clone(),
            metadata,
            None, // tools
            None, // fs_file_paths
            None, // job_filenames
            db,
            node_name_clone,
            identity_manager,
            job_manager_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
        )
        .await
        {
            Ok(job_id) => {
                let _ = res
                    .send(Ok(json!({
                        "job_id": job_id,
                    })))
                    .await;
                return Ok(());
            }
            Err(err) => {
                let _ = res.send(Err(err)).await;
                return Ok(());
            }
        }
    }

    pub async fn v2_api_tool_implementation_undo_to(
        bearer: String,
        db: Arc<SqliteManager>,
        message_hash: String,
        job_id: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Use the fetch_message_and_hash method to retrieve the message
        let (message, _hash) = match db.fetch_message_and_hash(&message_hash) {
            Ok(result) => result,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Message not found: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Determine if it's an AI or user message, if it's a user message then we need
        // to return an error
        if message.is_receiver_subidentity_agent() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Undo operation not allowed for user messages".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let mut new_message = message.clone();
        // Update the scheduled time to now so the messages are content wise the same
        // but produce a different hash
        new_message.external_metadata.scheduled_time = Utc::now().to_rfc3339();

        let inbox_name = match InboxName::get_job_inbox_name_from_params(job_id.clone()) {
            Ok(inbox) => inbox.to_string(),
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get job inbox name: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Add the message as a response to the job inbox
        let parent_hash = match db.get_parent_message_hash(&inbox_name, &message_hash) {
            Ok(hash) => {
                if let Some(hash) = hash {
                    hash
                } else {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to get message parent key".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get message parent key: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let undo_result = db
            .add_message_to_job_inbox(&job_id, &new_message, Some(parent_hash), None)
            .await;

        match undo_result {
            Ok(_) => {
                let response = json!({ "status": "success", "message": "Undo operation successful" });
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to undo tool implementation: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    async fn update_job_with_code(
        db: Arc<SqliteManager>,
        job_id: String,
        code: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
    ) -> Result<(), APIError> {
        // Get the main identity from the identity manager
        let main_identity = {
            let identity_manager = identity_manager.lock().await;
            match identity_manager.get_main_identity() {
                Some(identity) => identity.clone(),
                None => {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to get main identity".to_string(),
                    });
                }
            }
        };

        // Retrieve the job to get the llm_provider
        let llm_provider = match db.get_job_with_options(&job_id, false) {
            Ok(job) => job.parent_agent_or_llm_provider_id.clone(),
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve job: {}", err),
                });
            }
        };

        // Create a new job message
        let sender = match ShinkaiName::new(main_identity.get_full_identity_name()) {
            Ok(name) => name,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create sender name: {}", err),
                });
            }
        };

        let recipient = match ShinkaiName::from_node_and_profile_names_and_type_and_name(
            node_name.node_name.clone(),
            "main".to_string(),
            ShinkaiSubidentityType::Agent,
            llm_provider,
        ) {
            Ok(name) => name,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create recipient name: {}", err),
                });
            }
        };

        let job_message = JobMessage {
            job_id: job_id.clone(),
            content: format!("<input_command>Update the code to: {}</input_command>", code),
            parent: None,
            sheet_job_data: None,
            callback: None,
            metadata: None,
            tool_key: None,
            fs_files_paths: vec![],
            job_filenames: vec![],
            tools: None,
        };

        let shinkai_message = match Self::api_v2_create_shinkai_message(
            sender,
            recipient,
            &serde_json::to_string(&job_message).unwrap(),
            MessageSchemaType::JobMessageSchema,
            node_encryption_sk,
            node_signing_sk.clone(),
            node_encryption_pk,
            Some(job_id.clone()),
        ) {
            Ok(message) => message,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create Shinkai message: {}", err),
                });
            }
        };

        // Add the Shinkai message to the job inbox
        let add_message_result = db.add_message_to_job_inbox(&job_id, &shinkai_message, None, None).await;

        if let Err(err) = add_message_result {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to add Shinkai message to job inbox: {}", err),
            });
        }

        // Create the AI message
        let identity_secret_key_clone = clone_signature_secret_key(&node_signing_sk);
        // TODO This should be retrieved from the job (?) or from the endpoint
        let language = "typescript";
        let ai_message_content = format!("```{}\n{}\n```", language, code);
        let ai_shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            ai_message_content,
            vec![],
            None,
            identity_secret_key_clone,
            node_name.node_name.clone(),
            node_name.node_name.clone(),
        )
        .expect("Failed to build AI message");

        // Add the AI message to the job inbox
        let add_ai_message_result = db
            .add_message_to_job_inbox(&job_id, &ai_shinkai_message, None, None)
            .await;

        if let Err(err) = add_ai_message_result {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to add AI message to job inbox: {}", err),
            });
        }

        Ok(())
    }

    pub async fn v2_api_tool_implementation_code_update(
        bearer: String,
        db: Arc<SqliteManager>,
        job_id: String,
        code: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Update the job with the code using the helper function
        let update_result = Self::update_job_with_code(
            db.clone(),
            job_id.clone(),
            code.clone(),
            identity_manager.clone(),
            node_name.clone(),
            node_encryption_sk.clone(),
            node_encryption_pk.clone(),
            node_signing_sk.clone(),
        )
        .await;

        // Send success or error response
        match update_result {
            Ok(_) => {
                let response = json!({ "status": "success", "message": "Code update operation successful" });
                let _ = res.send(Ok(response)).await;
            }
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_export_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        shinkai_name: ShinkaiName,
        node_env: NodeEnvironment,
        tool_key_path: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match db.get_tool_by_key(&tool_key_path.clone()) {
            Ok(tool) => {
                let file_bytes = generate_tool_zip(db.clone(), shinkai_name.clone(), node_env, tool, true).await;
                match file_bytes {
                    Ok(file_bytes) => {
                        let _ = res.send(Ok(file_bytes)).await;
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to generate tool zip: {}", err.message),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to export tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_publish_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        shinkai_name: ShinkaiName,
        node_env: NodeEnvironment,
        tool_key_path: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        signing_secret_key: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let response = Self::publish_tool(
            db.clone(),
            shinkai_name,
            node_env,
            tool_key_path,
            identity_manager,
            signing_secret_key,
        )
        .await;

        match response {
            Ok(response) => {
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let _ = res.send(Err(err)).await;
            }
        }
        Ok(())
    }

    async fn publish_tool(
        db: Arc<SqliteManager>,
        shinkai_name: ShinkaiName,
        node_env: NodeEnvironment,
        tool_key_path: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        signing_secret_key: SigningKey,
    ) -> Result<Value, APIError> {
        // Generate zip file.
        let tool = db.get_tool_by_key(&tool_key_path.clone()).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to get tool: {}", e),
        })?;

        let file_bytes: Vec<u8> = generate_tool_zip(db.clone(), shinkai_name.clone(), node_env, tool, true)
            .await
            .map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to get tool zip: {}", e),
            })?;

        let identity_manager = identity_manager.lock().await;
        let local_node_name = identity_manager.local_node_name.clone();
        let identity_name = local_node_name.to_string();
        drop(identity_manager);

        // Hash
        let hash_raw = blake3::hash(&file_bytes.clone());
        let hash_hex = hash_raw.to_hex();
        let hash = hash_hex.to_string();

        // Signature
        let signature = signing_secret_key
            .clone()
            .try_sign(hash_hex.as_bytes())
            .map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to sign tool: {}", e),
            })?;

        let signature_bytes = signature.to_bytes();
        let signature_hex = hex::encode(signature_bytes);

        // Publish the tool to the store.
        let client = reqwest::Client::new();
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes)
                    .file_name(format!("{}.zip", tool_key_path.replace(':', "_"))),
            )
            .text("type", "Tool")
            .text("routerKey", tool_key_path.clone())
            .text("hash", hash.clone())
            .text("signature", signature_hex.clone())
            .text("identity", identity_name.clone());

        println!("[Publish Tool] Type: {}", "tool");
        println!("[Publish Tool] Router Key: {}", tool_key_path.clone());
        println!("[Publish Tool] Hash: {}", hash.clone());
        println!("[Publish Tool] Signature: {}", signature_hex.clone());
        println!("[Publish Tool] Identity: {}", identity_name.clone());

        let store_url = env::var("SHINKAI_STORE_URL").unwrap_or("https://store-api.shinkai.com".to_string());
        let response = client
            .post(format!("{}/store/revisions", store_url))
            .multipart(form)
            .send()
            .await
            .map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to publish tool: {}", e),
            })?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default().clone();
        println!("Response: {:?}", response_text);
        if status.is_success() {
            let r = json!({
                "status": "success",
                "message": "Tool published successfully",
                "tool_key": tool_key_path.clone(),
            });
            let r: Value = match r {
                Value::Object(mut map) => {
                    let response_json = serde_json::from_str(&response_text).unwrap_or_default();
                    map.insert("response".to_string(), response_json);
                    Value::Object(map)
                }
                o => o,
            };
            return Ok(r);
        } else {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Store Upload Error".to_string(),
                message: format!("Failed to upload to store: {}: {}", status, response_text),
            };
            return Err(api_error);
        }
    }

    pub async fn v2_api_import_tool_url(
        db: Arc<SqliteManager>,
        bearer: String,
        node_env: NodeEnvironment,
        url: String,
        node_name: String,
        signing_secret_key: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let result = Self::v2_api_import_tool_url_internal(db, node_env, url, node_name, signing_secret_key).await;
        let _ = match result {
            Ok(response) => res.send(Ok(response)).await,
            Err(err) => res.send(Err(err)).await,
        };
        Ok(())
    }

    pub async fn v2_api_import_tool_url_internal(
        db: Arc<SqliteManager>,
        node_env: NodeEnvironment,
        url: String,
        node_name: String,
        signing_secret_key: SigningKey,
    ) -> Result<Value, APIError> {
        let zip_contents: ZipFileContents =
            match download_zip_from_url(url, "__tool.json".to_string(), node_name, signing_secret_key).await {
                Ok(contents) => contents,
                Err(err) => return Err(err),
            };

        // Parse the JSON into a ShinkaiTool
        let tool: ShinkaiTool = match serde_json::from_slice(&zip_contents.buffer) {
            Ok(tool) => tool,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Tool JSON".to_string(),
                    message: format!("Failed to parse tool.json: {}", err),
                });
            }
        };

        let import_status = import_dependencies_tools(db.clone(), node_env.clone(), zip_contents.archive.clone()).await;
        if let Err(err) = import_status {
            return Err(err);
        }
        import_tool(db, node_env, zip_contents, tool).await
    }

    /// Resolves a Shinkai file protocol URL into actual file bytes.
    ///
    /// The Shinkai file protocol follows the format:
    /// `shinkai://file/{node_name}/{app-id}/{full-path}` This function
    /// validates the protocol format, constructs the actual file path in the
    /// node's storage, and returns the file contents as bytes.
    ///
    /// # Arguments
    /// * `bearer` - Bearer token for authentication
    /// * `db` - SQLite database manager for token validation
    /// * `shinkai_file_protocol` - The Shinkai file protocol URL to resolve
    /// * `node_storage_path` - Base path where tool files are stored
    /// * `res` - Channel sender to return the result
    ///
    /// # Returns
    /// * `Ok(())` - Operation completed (result sent through res channel)
    /// * `Err(NodeError)` - If there was an error in the operation
    ///
    /// # Protocol Format Example
    /// ```text
    /// shinkai://file/node123/app-456/path/to/file.txt
    /// ```
    ///
    /// The function will look for this file in:
    /// `{node_storage_path}/tools_storage/app-456/path/to/file.txt`
    pub async fn v2_api_resolve_shinkai_file_protocol(
        bearer: String,
        db: Arc<SqliteManager>,
        shinkai_file_protocol: String,
        node_storage_path: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Parse the shinkai file protocol
        // Format: shinkai://file/{node_name}/{app-id}/{full-path}
        let parts: Vec<&str> = shinkai_file_protocol.split('/').collect();
        if parts.len() < 5 || !shinkai_file_protocol.starts_with("shinkai://file/") {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Invalid Protocol".to_string(),
                message: "Invalid shinkai file protocol format".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // TODO This should be verified (?)
        let _user_name = parts[3];
        let app_id = parts[4];
        let remaining_path = parts[5..].join("/");

        // Construct the full file path
        let mut file_path = PathBuf::from(&node_storage_path);
        file_path.push("tools_storage");
        file_path.push(app_id);
        file_path.push(&remaining_path);

        // Read and return the file directly
        match fs::read(&file_path).await {
            Ok(contents) => {
                let _ = res.send(Ok(contents)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "File Not Found".to_string(),
                    message: format!("Failed to read file: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn disable_tools_for_job(db: Arc<SqliteManager>, bearer: String, job_id: String) -> Result<(), String> {
        // Get the current job config
        let (config_res_sender, config_res_receiver) = async_channel::bounded(1);

        let _ = Node::v2_api_get_job_config(db.clone(), bearer.clone(), job_id.clone(), config_res_sender).await;

        let current_config = match config_res_receiver.recv().await {
            Ok(Ok(config)) => config,
            Ok(Err(api_error)) => {
                return Err(format!("API error while getting job config: {}", api_error.message));
            }
            Err(err) => {
                return Err(format!("Failed to receive job config: {}", err));
            }
        };

        // Update the config to disable tools
        let new_config = JobConfig {
            use_tools: Some(false),
            ..current_config
        };

        // if new_config.use_tools is already false, don't update the config
        if !new_config.use_tools.unwrap_or(true) {
            return Ok(());
        }

        // Update the job config
        let (update_res_sender, update_res_receiver) = async_channel::bounded(1);

        let _ = Node::v2_api_update_job_config(
            db.clone(),
            bearer.clone(),
            job_id.clone(),
            new_config,
            update_res_sender,
        )
        .await;

        match update_res_receiver.recv().await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(api_error)) => Err(format!("API error while updating job config: {}", api_error.message)),
            Err(err) => Err(format!("Failed to update job config: {}", err)),
        }
    }

    pub async fn v2_api_upload_tool_asset(
        db: Arc<SqliteManager>,
        bearer: String,
        _tool_id: String,
        app_id: String,
        file_name: String,
        file_data: Vec<u8>,
        node_env: NodeEnvironment,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut file_path = PathBuf::from(&node_env.node_storage_path.unwrap_or_default());
        file_path.push(".tools_storage");
        file_path.push("playground");
        file_path.push(app_id);
        // Create directories if they don't exist
        if !file_path.exists() {
            std::fs::create_dir_all(&file_path)?;
        }
        file_path.push(&file_name);
        std::fs::write(&file_path, &file_data)?;

        let response = json!({
            "status": "success",
            "message": "Tool asset uploaded successfully",
            "file": file_data.len(),
            "file_name": file_name
        });
        let _ = res.send(Ok(response)).await;
        Ok(())
    }

    pub async fn v2_api_list_tool_assets(
        db: Arc<SqliteManager>,
        bearer: String,
        _tool_id: String,
        app_id: String,
        node_env: NodeEnvironment,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut file_path = PathBuf::from(&node_env.node_storage_path.unwrap_or_default());
        file_path.push(".tools_storage");
        file_path.push("playground");
        file_path.push(app_id);
        let files = std::fs::read_dir(&file_path);
        if files.is_err() {
            let _ = res.send(Ok(vec![])).await;
            return Ok(());
        }
        let files = files.unwrap();
        let file_names = files
            .map(|file| file.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        let _ = res.send(Ok(file_names)).await;
        Ok(())
    }

    pub async fn v2_api_delete_tool_asset(
        db: Arc<SqliteManager>,
        bearer: String,
        _tool_id: String,
        app_id: String,
        file_name: String,
        node_env: NodeEnvironment,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut file_path = PathBuf::from(&node_env.node_storage_path.unwrap_or_default());
        file_path.push(".tools_storage");
        file_path.push("playground");
        file_path.push(app_id);
        file_path.push(&file_name);
        let stat = std::fs::remove_file(&file_path).map_err(|err| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Failed to delete file".to_string(),
            message: format!("Failed to delete file: {}", err),
        });
        match stat {
            Ok(_) => {
                let response = json!({
                    "status": "success",
                    "message": "Tool asset deleted successfully",
                    "file_name": file_name
                });
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let _ = res.send(Err(err)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_remove_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        // Acquire a write lock on the database
        let db_write = db;

        let tool_router_key = ToolRouterKey::from_string(&tool_key);
        if tool_router_key.is_err() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Invalid tool key: {}", tool_router_key.err().unwrap()),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        let tool_router_key = tool_router_key.unwrap();
        let tool_key_name = tool_router_key.to_string_without_version();
        let version = tool_router_key.version;

        // Attempt to remove the playground tool first, warn on failure but continue
        if let Err(e) = db_write.remove_tool_playground(&tool_key) {
            log::warn!(
                "Attempt to remove associated playground tool for key '{}' failed (this might be expected if none exists): {}. Continuing with main tool removal.",
                tool_key,
                e
            );
        }

        // Remove the tool from the database
        match db_write.remove_tool(&tool_key_name, version) {
            Ok(_) => {
                let response = json!({ "status": "success", "message": "Tool and associated playground (if any) removed successfully" });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_enable_all_tools(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get all tools
        match db.get_all_tool_headers() {
            Ok(tools) => {
                let mut tool_statuses: Vec<(String, bool)> = Vec::new();

                for tool in tools {
                    let version = match IndexableVersion::from_string(&tool.version) {
                        Ok(v) => v,
                        Err(_) => {
                            tool_statuses.push((tool.tool_router_key, false));
                            continue;
                        }
                    };

                    match db.get_tool_by_key_and_version(&tool.tool_router_key, Some(version)) {
                        Ok(mut shinkai_tool) => {
                            if shinkai_tool.can_be_enabled() {
                                shinkai_tool.enable();
                                if shinkai_tool.is_enabled() {
                                    let _ = db.update_tool(shinkai_tool.clone()).await.is_ok();
                                }
                            }
                            let activated = shinkai_tool.is_enabled();
                            tool_statuses.push((tool.tool_router_key, activated));
                        }
                        Err(_) => {
                            tool_statuses.push((tool.tool_router_key, false));
                        }
                    }
                }

                let response = json!(tool_statuses
                    .into_iter()
                    .map(|(key, activated)| { (key, json!({"activated": activated})) })
                    .collect::<Map<String, Value>>());
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_disable_all_tools(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get all tools
        match db.get_all_tool_headers() {
            Ok(tools) => {
                let mut tool_statuses: Vec<(String, bool)> = Vec::new();

                for tool in tools {
                    let version = match IndexableVersion::from_string(&tool.version) {
                        Ok(v) => v,
                        Err(_) => {
                            tool_statuses.push((tool.tool_router_key, false));
                            continue;
                        }
                    };

                    match db.get_tool_by_key_and_version(&tool.tool_router_key, Some(version)) {
                        Ok(mut shinkai_tool) => {
                            shinkai_tool.disable();
                            if !shinkai_tool.is_enabled() {
                                let _ = db.update_tool(shinkai_tool.clone()).await.is_ok();
                            }

                            let activated = shinkai_tool.is_enabled();
                            tool_statuses.push((tool.tool_router_key, activated));
                        }
                        Err(_) => {
                            tool_statuses.push((tool.tool_router_key, false));
                        }
                    }
                }

                let response = json!(tool_statuses
                    .into_iter()
                    .map(|(key, activated)| { (key, json!({"activated": activated})) })
                    .collect::<Map<String, Value>>());
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_duplicate_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_key_path: String,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Option<Arc<Mutex<JobManager>>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let result = Self::duplicate_tool(
            db,
            tool_key_path,
            node_name,
            identity_manager,
            job_manager,
            bearer,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
        )
        .await;
        let _ = match result {
            Ok(result) => res.send(Ok(result)).await,
            Err(err) => res.send(Err(err)).await,
        };
        Ok(())
    }

    async fn create_job_for_duplicate_tool(
        db_clone: Arc<SqliteManager>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Option<Arc<Mutex<JobManager>>>,
        bearer: String,
        llm_provider: String,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
    ) -> Result<String, APIError> {
        let (res_sender, res_receiver) = async_channel::bounded(1);

        let job_creation_info = JobCreationInfo {
            scope: MinimalJobScope::default(),
            is_hidden: Some(true),
            associated_ui: None,
        };
        let job_manager = match job_manager_clone {
            Some(job_manager) => job_manager,
            None => {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Job manager not found".to_string(),
                })
            }
        };
        let _ = Node::v2_create_new_job(
            db_clone.clone(),
            node_name_clone.clone(),
            identity_manager_clone.clone(),
            job_manager.clone(),
            bearer.clone(),
            job_creation_info,
            llm_provider,
            encryption_secret_key_clone.clone(),
            encryption_public_key_clone.clone(),
            signing_secret_key_clone.clone(),
            res_sender,
        )
        .await;

        let job_id = res_receiver
            .recv()
            .await
            .map_err(|e| Node::generic_api_error(&e.to_string()))
            .map_err(|_| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: "Failed to create job".to_string(),
            })?;

        return job_id;
    }

    async fn duplicate_tool(
        db: Arc<SqliteManager>,
        tool_key_path: String,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Option<Arc<Mutex<JobManager>>>,
        bearer: String,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
    ) -> Result<Value, APIError> {
        // Get the original tool
        let original_tool = db.get_tool_by_key(&tool_key_path).map_err(|_| APIError {
            code: StatusCode::NOT_FOUND.as_u16(),
            error: "Not Found".to_string(),
            message: format!("Tool not found: {}", tool_key_path),
        })?;

        let llm_providers = db.get_all_llm_providers().map_err(|_| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: "Failed to get all llm providers".to_string(),
        })?;
        if llm_providers.is_empty() {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: "No LLM providers found".to_string(),
            });
        }
        let llm_provider = llm_providers[0].clone();

        // Create a copy of the tool with "_copy" appended to the name
        let mut new_tool = original_tool.clone();
        let new_name = format!(
            "{}_{}",
            original_tool.name(),
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        new_tool.update_name(new_name.clone());
        new_tool.update_author(node_name.node_name.clone());

        // Update the tool_router_key for Deno tools since they store it explicitly
        if let ShinkaiTool::Deno(deno_tool, enabled) = &mut new_tool {
            if deno_tool.tool_router_key.is_some() {
                deno_tool.tool_router_key = Some(ToolRouterKey::new(
                    "local".to_string(),
                    node_name.node_name.clone(),
                    new_name,
                    None,
                ));
            }
        }

        // Try to get the original playground tool, or create one from the tool data
        let (new_playground, is_new_playground) = match db.get_tool_playground(&tool_key_path) {
            Ok(playground) => {
                let mut new_playground = playground.clone();
                new_playground.metadata.name = new_tool.name();
                new_playground.metadata.author = new_tool.author();
                new_playground.job_id = Self::fork_job(
                    db.clone(),
                    node_name.clone(),
                    identity_manager.clone(),
                    playground.job_id,
                    None,
                    encryption_secret_key.clone(),
                    encryption_public_key.clone(),
                    signing_secret_key.clone(),
                )
                .await?;
                new_playground.job_id_history = vec![];
                new_playground.tool_router_key = Some(new_tool.tool_router_key().to_string_without_version());
                (new_playground, false)
            }
            Err(_) => {
                // Create a new playground from the tool data
                let output = new_tool.output_arg();
                let output_json = output.json;
                // Attempt to parse the output_json into a meaningful result
                let result: ToolResult = if !output_json.is_empty() {
                    match serde_json::from_str::<serde_json::Value>(&output_json) {
                        Ok(value) => {
                            // Extract type from the value if possible
                            let result_type = if value.is_object() {
                                "object"
                            } else if value.is_array() {
                                "array"
                            } else if value.is_string() {
                                "string"
                            } else if value.is_number() {
                                "number"
                            } else if value.is_boolean() {
                                "boolean"
                            } else {
                                "object"
                            };

                            ToolResult::new(result_type.to_string(), value, vec![])
                        }
                        Err(_) => ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                    }
                } else {
                    // Default to a basic object result when we can't extract anything meaningful
                    ToolResult::new("object".to_string(), serde_json::Value::Null, vec![])
                };

                // Properly extract metadata from the original tool
                let language = match original_tool {
                    ShinkaiTool::Deno(_, _) => CodeLanguage::Typescript,
                    ShinkaiTool::Python(_, _) => CodeLanguage::Python,
                    _ => CodeLanguage::Typescript, // Default to typescript for other types
                };

                // Create playground with properly extracted metadata
                let playground = ToolPlayground {
                    language,
                    metadata: ToolPlaygroundMetadata {
                        name: new_tool.name(),
                        homepage: new_tool.get_homepage(),
                        version: new_tool.version(),
                        description: new_tool.description(),
                        author: new_tool.author(),
                        keywords: new_tool.get_keywords(),
                        configurations: new_tool.get_config(),
                        parameters: new_tool.input_args(),
                        result,
                        sql_tables: new_tool.sql_tables(),
                        sql_queries: new_tool.sql_queries(),
                        tools: Some(new_tool.get_tools()),
                        oauth: new_tool.get_oauth(),
                        runner: new_tool.get_runner(),
                        operating_system: new_tool.get_operating_system(),
                        tool_set: new_tool.get_tool_set(),
                    },
                    tool_router_key: Some(new_tool.tool_router_key().to_string_without_version()),
                    job_id: Self::create_job_for_duplicate_tool(
                        db.clone(),
                        node_name.clone(),
                        identity_manager.clone(),
                        job_manager.clone(),
                        bearer.clone(),
                        llm_provider.id.clone(),
                        encryption_secret_key.clone(),
                        encryption_public_key.clone(),
                        signing_secret_key.clone(),
                    )
                    .await?,
                    job_id_history: vec![],
                    code: new_tool.get_code(),
                    assets: new_tool.get_assets(),
                };

                (playground, true)
            }
        };

        // Add the new tool to the database
        let new_tool = db.add_tool(new_tool).await.map_err(|_| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: "Failed to add duplicated tool".to_string(),
        })?;

        // Add the new playground tool
        db.set_tool_playground(&new_playground).map_err(|_| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: "Failed to add duplicated playground tool".to_string(),
        })?;

        // If we created a new playground (tool_playground was not found), initialize the job with code
        if is_new_playground {
            // Update the job with an initial message containing the code
            if let Err(e) = Self::update_job_with_code(
                db.clone(),
                new_playground.job_id.clone(),
                new_playground.code.clone(),
                identity_manager.clone(),
                node_name.clone(),
                encryption_secret_key.clone(),
                encryption_public_key.clone(),
                signing_secret_key.clone(),
            )
            .await
            {
                eprintln!("Failed to update job with initial code: {:?}", e);
                // Continue anyway since this is not critical
            }
        }

        // Return the new tool's router key
        let response = json!({
            "tool_router_key": new_tool.tool_router_key().to_string_without_version(),
            "version": new_tool.version(),
            "job_id": new_playground.job_id,
        });
        Ok(response)
    }

    pub async fn install_tool_from_u8(
        db: Arc<SqliteManager>,
        node_env: NodeEnvironment,
        zip_data: Vec<u8>,
    ) -> Result<Value, APIError> {
        // Create a cursor from the zip data
        let cursor = std::io::Cursor::new(zip_data);
        let mut archive = match zip::ZipArchive::new(cursor) {
            Ok(archive) => archive,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Zip File".to_string(),
                    message: format!("Failed to read zip archive: {}", err),
                });
            }
        };

        // Extract and parse tool.json
        let mut buffer: Vec<u8> = Vec::new();
        {
            let mut file = match archive.by_name("__tool.json") {
                Ok(file) => file,
                Err(_) => {
                    return Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Invalid Zip File".to_string(),
                        message: "Archive does not contain __tool.json".to_string(),
                    });
                }
            };

            if let Err(err) = file.read_to_end(&mut buffer) {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Tool JSON".to_string(),
                    message: format!("Failed to read tool.json: {}", err),
                });
            }
        }

        // Parse the JSON into a ShinkaiTool
        let tool: ShinkaiTool = match serde_json::from_slice(&buffer) {
            Ok(tool) => tool,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Tool JSON".to_string(),
                    message: format!("Failed to parse tool.json: {}", err),
                });
            }
        };
        let zip_contents = ZipFileContents { buffer, archive };
        let import_status = import_dependencies_tools(db.clone(), node_env.clone(), zip_contents.archive.clone()).await;
        if let Err(err) = import_status {
            return Err(err);
        }
        return import_tool(db, node_env, zip_contents, tool).await;
    }

    pub async fn v2_api_import_tool_zip(
        db: Arc<SqliteManager>,
        bearer: String,
        node_env: NodeEnvironment,
        file_data: Vec<u8>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let result = Self::install_tool_from_u8(db, node_env, file_data).await;
        let _ = res.send(result).await;
        Ok(())
    }

    pub async fn v2_api_store_proxy(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_router_key: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let store_url = env::var("SHINKAI_STORE_URL").unwrap_or("https://store-api.shinkai.com".to_string());

        let client = reqwest::Client::new();

        // Make parallel requests using tokio::try_join!
        let assets_future = client
            .get(format!("{}/store/products/{}/assets", store_url, tool_router_key))
            .send();
        let product_future = client
            .get(format!("{}/store/products/{}", store_url, tool_router_key))
            .send();

        let (assets_response, product_response) = match tokio::try_join!(assets_future, product_future) {
            Ok((assets, product)) => (assets, product),
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Store Request Failed".to_string(),
                    message: format!("Failed to fetch from store: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Process responses
        let assets_json = match assets_response.json::<Value>().await {
            Ok(json) => json,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Invalid Assets Response".to_string(),
                    message: format!("Failed to parse assets response: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let product_json = match product_response.json::<Value>().await {
            Ok(json) => json,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Invalid Product Response".to_string(),
                    message: format!("Failed to parse product response: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Combine responses
        let response = json!({
            "assets": assets_json,
            "product": product_json
        });

        let _ = res.send(Ok(response)).await;
        Ok(())
    }

    pub async fn v2_api_standalone_playground(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        bearer: String,
        node_env: NodeEnvironment,
        code: Option<String>,
        metadata: Option<Value>,
        assets: Option<Vec<String>>,
        language: CodeLanguage,
        tools: Option<Vec<ToolRouterKey>>,
        parameters: Option<Value>,
        config: Option<Value>,
        oauth: Option<Vec<OAuth>>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let result = Self::create_standalone_playground(
            node_name,
            code,
            metadata,
            assets,
            language,
            tools,
            parameters,
            config,
            oauth,
            tool_id,
            app_id,
            llm_provider,
            bearer,
            node_env,
            db.clone(),
        )
        .await;

        let _ = match result {
            Ok(result) => res.send(Ok(result)).await,
            Err(err) => res.send(Err(err)).await,
        };
        Ok(())
    }

    async fn create_standalone_playground(
        node_name: ShinkaiName,
        code: Option<String>,
        metadata: Option<Value>,
        assets: Option<Vec<String>>,
        language: CodeLanguage,
        _tools: Option<Vec<ToolRouterKey>>,
        _parameters: Option<Value>,
        _config: Option<Value>,
        _oauth: Option<Vec<OAuth>>,
        _tool_id: String,
        app_id: String,
        llm_provider: String,
        bearer: String,
        node_env: NodeEnvironment,
        db: Arc<SqliteManager>,
    ) -> Result<Value, APIError> {
        let mut temp_dir = std::env::temp_dir();
        temp_dir.push(format!("shinkai_playground_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to create temporary directory".to_string(),
            message: e.to_string(),
        })?;

        // Download and extract template from GitHub
        println!(
            "[Step 1] Downloading template from GitHub to {}",
            temp_dir.to_string_lossy().to_string()
        );

        let zip_url =
            "https://pub-e508ac8b539c45edb9724730588f74cc.r2.dev/shinkai-tool-boilerplate-feature-user-template.zip";

        let response = reqwest::get(zip_url).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to download template".to_string(),
            message: e.to_string(),
        })?;
        if !response.status().is_success() {
            return Err(APIError {
                code: 500,
                error: "Failed to download template".to_string(),
                message: format!("Failed to download template: {}", response.status()),
            });
        }

        let response_bytes = response.bytes().await.map_err(|e| APIError {
            code: 500,
            error: "Failed to read template bytes".to_string(),
            message: e.to_string(),
        })?;

        let zip_reader = std::io::Cursor::new(response_bytes);
        let mut archive = zip::ZipArchive::new(zip_reader).map_err(|e| APIError {
            code: 500,
            error: "Failed to read ZIP archive".to_string(),
            message: e.to_string(),
        })?;

        archive.extract(&temp_dir).map_err(|e| APIError {
            code: 500,
            error: "Failed to extract template".to_string(),
            message: e.to_string(),
        })?;

        // Move contents from the extracted subdirectory to temp_dir
        let extracted_dir = temp_dir.join("shinkai-tool-boilerplate-feature-user-template");
        for entry in std::fs::read_dir(&extracted_dir).map_err(|e| APIError {
            code: 500,
            error: "Failed to read extracted directory".to_string(),
            message: e.to_string(),
        })? {
            let entry = entry.map_err(|e| APIError {
                code: 500,
                error: "Failed to read directory entry".to_string(),
                message: e.to_string(),
            })?;
            let target = temp_dir.join(entry.file_name());
            if entry.path() != target {
                std::fs::rename(entry.path(), target).map_err(|e| APIError {
                    code: 500,
                    error: "Failed to move template files".to_string(),
                    message: e.to_string(),
                })?;
            }
        }
        std::fs::remove_dir_all(&extracted_dir).map_err(|e| APIError {
            code: 500,
            error: "Failed to cleanup extracted directory".to_string(),
            message: e.to_string(),
        })?;

        // Install dependencies
        println!(
            "[Step 3] Installing dependencies: npm ci @ {}",
            temp_dir.to_string_lossy().to_string()
        );

        let npm_binary = env::var("NPM_BINARY_LOCATION").unwrap_or_else(|_| "npm".to_string());
        let result = Command::new(npm_binary)
            .current_dir(&temp_dir)
            .args(["ci"])
            .output()
            .await;

        if let Ok(output) = result {
            println!("{}", String::from_utf8_lossy(&output.stdout));
            println!("{}", String::from_utf8_lossy(&output.stderr));
        } else {
            println!("Failed to install dependencies (npm ci)");
        }

        // Get all tool-key-paths
        let tool_list: Vec<ToolRouterKey> = db
            .get_all_tool_headers()
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to get tool headers".to_string(),
                message: e.to_string(),
            })?
            .into_iter()
            .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                Ok(tool_router_key) => Some(tool_router_key),
                Err(_) => None,
            })
            .collect::<Vec<ToolRouterKey>>();

        println!("[Step 4] Updating shinkai-local-tools & shinkai-local-support files");
        // Update shinkai-local-tools & shinkai-local-support files
        let tool_definitions = generate_tool_definitions(tool_list.clone(), language.clone(), db, false).await?;
        for (tool_key, tool_definition) in tool_definitions {
            match language.clone() {
                CodeLanguage::Typescript => {
                    let tool_file_path = temp_dir
                        .clone()
                        .join(PathBuf::from("my-tool-typescript"))
                        .join(format!("{}.ts", tool_key));
                    fs::write(&tool_file_path, tool_definition.clone())
                        .await
                        .map_err(|e| APIError {
                            code: 500,
                            error: "Failed to write tool file".to_string(),
                            message: e.to_string(),
                        })?;
                }
                CodeLanguage::Python => {
                    let tool_file_path = temp_dir
                        .clone()
                        .join(PathBuf::from("my-tool-python"))
                        .join(format!("{}.py", tool_key));
                    fs::write(&tool_file_path, tool_definition.clone())
                        .await
                        .map_err(|e| APIError {
                            code: 500,
                            error: "Failed to write tool file".to_string(),
                            message: e.to_string(),
                        })?;
                }
            }
        }

        println!("[Step 5] Removing folder based on language");
        // Remove folder based on language
        let mut env_language = "";
        match language {
            CodeLanguage::Python => {
                env_language = "Python";
                let _ = std::fs::remove_dir_all(temp_dir.join("my-tool-typescript"));
            }
            CodeLanguage::Typescript => {
                env_language = "Typescript";
                let _ = std::fs::remove_dir_all(temp_dir.join("my-tool-python"));
            }
        }
        let _ = std::fs::remove_file(temp_dir.join(".env.example"));

        // Handle code replacement if provided
        if let Some(code_content) = code {
            let tool_file_path = match language {
                CodeLanguage::Python => temp_dir.join("my-tool-python").join("tool.py"),
                CodeLanguage::Typescript => temp_dir.join("my-tool-typescript").join("tool.ts"),
            };
            fs::write(&tool_file_path, code_content).await.map_err(|e| APIError {
                code: 500,
                error: "Failed to write tool code file".to_string(),
                message: e.to_string(),
            })?;
        }

        // Handle metadata replacement if provided
        if let Some(metadata_content) = metadata {
            let metadata_file_path = match language {
                CodeLanguage::Python => temp_dir.join("my-tool-python").join("metadata.json"),
                CodeLanguage::Typescript => temp_dir.join("my-tool-typescript").join("metadata.json"),
            };
            fs::write(
                &metadata_file_path,
                serde_json::to_string_pretty(&metadata_content).map_err(|e| APIError {
                    code: 500,
                    error: "Failed to serialize metadata".to_string(),
                    message: e.to_string(),
                })?,
            )
            .await
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to write metadata file".to_string(),
                message: e.to_string(),
            })?;
        }

        // Handle assets if provided
        if let Some(asset_paths) = assets {
            let assets_dir = match language {
                CodeLanguage::Python => temp_dir.join("my-tool-python").join("assets"),
                CodeLanguage::Typescript => temp_dir.join("my-tool-typescript").join("assets"),
            };

            // Create assets directory if it doesn't exist
            fs::create_dir_all(&assets_dir).await.map_err(|e| APIError {
                code: 500,
                error: "Failed to create assets directory".to_string(),
                message: e.to_string(),
            })?;

            let node_storage_path = node_env.node_storage_path.clone().unwrap_or_else(|| "".to_string());
            let node_assets_path = PathBuf::from(&node_storage_path)
                .join(".tools_storage")
                .join("playground")
                .join(app_id.clone());

            // Copy each asset file
            for file_name in asset_paths {
                let source_path = node_assets_path.join(file_name.clone());
                let target_path = assets_dir.join(file_name.clone());

                fs::copy(&source_path, &target_path).await.map_err(|e| APIError {
                    code: 500,
                    error: "Failed to copy asset file".to_string(),
                    message: format!(
                        "Failed to copy {} to {}: {}",
                        source_path.display(),
                        target_path.display(),
                        e
                    ),
                })?;
            }
        }

        println!("[Step 6] Creating .env file");

        let api_url = format!(
            "http://{}:{}",
            node_env.api_listen_address.ip(),
            node_env.api_listen_address.port()
        );
        let random_uuid = uuid::Uuid::new_v4().to_string();
        let identity = node_name.get_node_name_string();
        // Create .env file with environment variables
        let env_content = format!(
            r#"
NODE_URL={api_url}
API_KEY={bearer}
LLM_PROVIDER={llm_provider}
DEBUG_HTTP_REQUESTS=false
X_SHINKAI_APP_ID=app-id-{random_uuid}
IDENTITY="{identity}"
LANGUAGE={env_language}
            "#,
        );
        fs::write(temp_dir.join(".env"), env_content)
            .await
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to create .env file".to_string(),
                message: e.to_string(),
            })?;

        println!("[Step 7] Writing tool keys to .tool-key-path file");
        // Write tool keys to .tool-key-path file
        let tool_key_path = temp_dir.join(".tool-key-path");
        std::fs::write(
            &tool_key_path,
            tool_list
                .iter()
                .map(|tool| tool.to_string_without_version())
                .collect::<Vec<String>>()
                .join("\n"),
        )
        .map_err(|e| APIError {
            code: 500,
            error: "Failed to write tool keys".to_string(),
            message: e.to_string(),
        })?;

        println!(
            "Playground created successfully: {}",
            temp_dir.to_string_lossy().to_string()
        );

        println!("[Step 8] Launching IDE");
        // Finally launch the playground
        // First try to open with cursor
        let cursor_open = Command::new("cursor").arg(temp_dir.clone()).spawn();
        if cursor_open.is_err() {
            // If cursor fails try with the "code" command
            let code_open = Command::new("code").arg(temp_dir.clone()).spawn();
            if code_open.is_err() {
                // If cursor and code fails, try with open
                // Ignore error if any.
                let _ = open::that(temp_dir.clone());
            }
        }

        Ok(json!({
            "status": "success",
            "playground_path": temp_dir.to_string_lossy().to_string(),
        }))
    }

    pub async fn v2_api_list_all_shinkai_tools_versions(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all tools
        match db.get_all_tool_headers() {
            Ok(tools) => {
                // Group tools by their base key (without version)
                use std::collections::HashMap;
                let mut tool_groups: HashMap<String, Vec<ShinkaiToolHeader>> = HashMap::new();

                for tool in tools {
                    let tool_router_key = tool.tool_router_key.clone();
                    tool_groups.entry(tool_router_key).or_default().push(tool);
                }

                // For each group, sort versions and create the response structure
                let mut result = Vec::new();
                for (key, mut group) in tool_groups {
                    // Sort by version in descending order
                    group.sort_by(|a, b| {
                        let a_version = IndexableVersion::from_string(&a.version.clone())
                            .unwrap_or(IndexableVersion::from_number(0));
                        let b_version = IndexableVersion::from_string(&b.version.clone())
                            .unwrap_or(IndexableVersion::from_number(0));
                        b_version.cmp(&a_version)
                    });

                    // Extract versions
                    let versions: Vec<String> = group.iter().map(|tool| tool.version.clone()).collect();

                    result.push(json!({
                        "tool_router_key": key,
                        "versions": versions,
                    }));
                }

                let _ = res.send(Ok(json!(result))).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list tools: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_set_tool_enabled(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_router_key: String,
        enabled: bool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool first to verify it exists
        let mut tool = match db.get_tool_by_key(&tool_router_key) {
            Ok(t) => t,
            Err(_) => {
                let err = APIError {
                    code: 404,
                    error: "Tool not found".to_string(),
                    message: format!("Tool not found: {}", tool_router_key),
                };
                let _ = res.send(Err(err)).await;
                return Ok(());
            }
        };
        // Check if the tool can be enabled
        if enabled && !tool.can_be_enabled() {
            let err = APIError {
                code: 400,
                error: "Tool Cannot Be Enabled".to_string(),
                message: "Tool Cannot Be Enabled".to_string(),
            };
            let _ = res.send(Err(err)).await;
            return Ok(());
        }
        // Enable or disable the tool
        if enabled {
            tool.enable();
        } else {
            tool.disable();
            tool.disable_mcp();
        }

        if let Err(e) = db.update_tool(tool).await {
            let err = APIError {
                code: 500,
                error: "Failed to update tool".to_string(),
                message: format!("Failed to update tool: {}", e),
            };
            let _ = res.send(Err(err)).await;
            return Ok(());
        }

        let response = json!({
            "tool_router_key": tool_router_key,
            "enabled": enabled,
            "success": true
        });
        let _ = res.send(Ok(response)).await;
        Ok(())
    }

    pub async fn v2_api_set_tool_mcp_enabled(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_router_key: String,
        mcp_enabled: bool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        // Get the tool first to verify it exists
        match db.get_tool_by_key(&tool_router_key) {
            Ok(mut tool) => {
                if mcp_enabled {
                    tool.enable_mcp();
                } else {
                    tool.disable_mcp();
                }

                // Save the updated tool
                match db.update_tool(tool).await {
                    Ok(_) => {
                        let response = json!({
                            "tool_router_key": tool_router_key,
                            "mcp_enabled": mcp_enabled,
                            "success": true
                        });
                        let _ = res.send(Ok(response)).await;
                    }
                    Err(e) => {
                        let _ = res
                            .send(Err(APIError {
                                code: 500,
                                error: "Failed to update tool".to_string(),
                                message: format!("Failed to update tool: {}", e),
                            }))
                            .await;
                    }
                }
            }
            Err(_) => {
                let _ = res
                    .send(Err(APIError {
                        code: 404,
                        error: "Tool not found".to_string(),
                        message: format!("Tool with key '{}' not found", tool_router_key),
                    }))
                    .await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_copy_tool_assets(
        db: Arc<SqliteManager>,
        bearer: String,
        node_env: NodeEnvironment,
        is_first_playground: bool,
        first_path: String,
        is_second_playground: bool,
        second_path: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let response = Self::v2_api_copy_tool_assets_internal(
            node_env,
            is_first_playground,
            first_path,
            is_second_playground,
            second_path,
        )
        .await;

        let _ = res.send(response).await;

        Ok(())
    }

    async fn v2_api_copy_tool_assets_internal(
        node_env: NodeEnvironment,
        is_first_playground: bool,
        first_path: String,
        is_second_playground: bool,
        second_path: String,
    ) -> Result<Value, APIError> {
        let storage_path = node_env.node_storage_path.unwrap_or_default();

        // Create source path
        let mut source_path = PathBuf::from(storage_path.clone());
        source_path.push(".tools_storage");
        if is_first_playground {
            source_path.push("playground");
            source_path.push(first_path);
        } else {
            source_path.push("tools");
            source_path.push(ToolRouterKey::from_string(&first_path)?.convert_to_path());
        }

        // Create destination path
        let mut dest_path = PathBuf::from(storage_path);
        dest_path.push(".tools_storage");
        if is_second_playground {
            dest_path.push("playground");
            dest_path.push(second_path);
        } else {
            dest_path.push("tools");
            dest_path.push(ToolRouterKey::from_string(&second_path)?.convert_to_path());
        }

        // Verify source exists
        if !source_path.exists() {
            return Ok(json!({
                "success": false,
                "message": "Nothing to copy. Source path does not exist"
            }));
        }

        if dest_path.exists() {
            std::fs::remove_dir_all(&dest_path).map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Failed to remove destination directory".to_string(),
                message: format!("Error removing destination directory: {}", e),
            })?;
        }

        std::fs::create_dir_all(&dest_path).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Failed to create destination directory".to_string(),
            message: format!("Error creating destination directory: {}", e),
        })?;

        // Copy all files from source to destination
        let entries = std::fs::read_dir(&source_path).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Failed to read source directory".to_string(),
            message: format!("Error reading source directory: {}", e),
        })?;

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(e) => {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Failed to read directory entry".to_string(),
                        message: format!("Error reading directory entry: {}", e),
                    });
                }
            };

            let file_name = entry.file_name();
            let mut dest_file = dest_path.clone();
            dest_file.push(file_name);

            match entry.file_type() {
                Ok(file_type) if file_type.is_file() => {
                    std::fs::copy(entry.path(), dest_file).map_err(|e| APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Failed to copy file".to_string(),
                        message: format!("Error copying file: {}", e),
                    })?;
                }
                Ok(_) => continue, // Skip if not a file
                Err(e) => {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Failed to get file type".to_string(),
                        message: format!("Error getting file type: {}", e),
                    });
                }
            }
        }

        Ok(json!({
            "success": true,
            "message": "Tool assets copied successfully"
        }))
    }

    pub async fn check_tool(
        bearer: String,
        db: Arc<SqliteManager>,
        code: String,
        language: CodeLanguage,
        additional_headers: Option<HashMap<String, String>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let tools: Vec<ToolRouterKey> = db
            .get_all_tool_headers()
            .map_err(|_| ToolError::ExecutionError("Failed to get tool headers".to_string()))?
            .iter()
            .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                Ok(tool_router_key) => Some(tool_router_key),
                Err(_) => None,
            })
            .collect();

        let warnings = match language {
            CodeLanguage::Typescript => {
                let mut support_files = generate_tool_definitions(tools, CodeLanguage::Typescript, db.clone(), false)
                    .await
                    .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
                // External additional headers can override the default support files
                match additional_headers {
                    Some(headers) => {
                        for (key, value) in headers {
                            support_files.insert(key, value);
                        }
                    }
                    None => (),
                }
                let tool = DenoTool::new(
                    "".to_string(),
                    None,
                    "".to_string(),
                    "".to_string(),
                    Some(false),
                    code.clone(),
                    vec![],
                    vec![],
                    "".to_string(),
                    vec![],
                    Parameters::new(),
                    ToolOutputArg { json: "".to_string() },
                    true,
                    None,
                    ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                    None,
                    None,
                    None,
                    None,
                    None,
                    RunnerType::Any,
                    vec![],
                    None,
                );
                tool.check_code(code.clone(), support_files).await
            }
            CodeLanguage::Python => {
                let mut support_files = generate_tool_definitions(tools, CodeLanguage::Python, db.clone(), false)
                    .await
                    .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
                // External additional headers can override the default support files
                match additional_headers {
                    Some(headers) => {
                        for (key, value) in headers {
                            support_files.insert(key, value);
                        }
                    }
                    None => (),
                }

                let tool: PythonTool = PythonTool {
                    version: "".to_string(),
                    name: "".to_string(),
                    tool_router_key: None,
                    homepage: None,
                    author: "".to_string(),
                    mcp_enabled: Some(false),
                    py_code: code.clone(),
                    tools: vec![],
                    config: vec![],
                    description: "".to_string(),
                    keywords: vec![],
                    input_args: Parameters::new(),
                    output_arg: ToolOutputArg { json: "".to_string() },
                    activated: true,
                    embedding: None,
                    result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                    sql_tables: None,
                    sql_queries: None,
                    file_inbox: None,
                    oauth: None,
                    assets: None,
                    runner: RunnerType::Any,
                    operating_system: vec![],
                    tool_set: None,
                };
                tool.check_code(code.clone(), support_files).await
            }
        };

        match warnings {
            Ok(warnings) => {
                let _ = res
                    .send(Ok(json!({
                        "warnings": warnings,
                        "success": true
                    })))
                    .await;
            }
            Err(e) => {
                let _ = res
                    .send(Err(APIError {
                        code: 500,
                        error: "Check Failed".to_string(),
                        message: format!("Tool check failed: {}", e),
                    }))
                    .await;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // TODO: update to not use workflow
    #[test]
    fn test_merge_json() {
        let existing_tool_value = json!({
            "content": [{
                "embedding": {
                    "id": "",
                    "vector": [0.1, 0.2, 0.3]
                },
                "workflow": {
                    "author": "@@official.shinkai",
                    "description": "Reviews in depth all the content to generate a summary.",
                    "name": "Extensive_summary",
                    "raw": "workflow Extensive_summary v0.1 { ... }",
                    "steps": [
                        {
                            "body": [
                                {
                                    "type": "composite",
                                    "value": [
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$PROMPT",
                                                "value": "Summarize this: "
                                            }
                                        },
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$EMBEDDINGS",
                                                "value": {
                                                    "args": [],
                                                    "name": "process_embeddings_in_job_scope"
                                                }
                                            }
                                        }
                                    ]
                                }
                            ],
                            "name": "Initialize"
                        },
                        {
                            "body": [
                                {
                                    "type": "registeroperation",
                                    "value": {
                                        "register": "$RESULT",
                                        "value": {
                                            "args": ["$PROMPT", "$EMBEDDINGS"],
                                            "name": "multi_inference"
                                        }
                                    }
                                }
                            ],
                            "name": "Summarize"
                        }
                    ],
                    "sticky": true,
                    "version": "0.1"
                }
            }],
            "type": "Workflow"
        });

        let input_value = json!({
            "content": [{
                "workflow": {
                    "description": "New description"
                }
            }],
            "type": "Workflow"
        });

        let expected_merged_value = json!({
            "content": [{
                "embedding": {
                    "id": "",
                    "vector": [0.1, 0.2, 0.3]
                },
                "workflow": {
                    "author": "@@official.shinkai",
                    "description": "New description",
                    "name": "Extensive_summary",
                    "raw": "workflow Extensive_summary v0.1 { ... }",
                    "steps": [
                        {
                            "body": [
                                {
                                    "type": "composite",
                                    "value": [
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$PROMPT",
                                                "value": "Summarize this: "
                                            }
                                        },
                                        {
                                            "type": "registeroperation",
                                            "value": {
                                                "register": "$EMBEDDINGS",
                                                "value": {
                                                    "args": [],
                                                    "name": "process_embeddings_in_job_scope"
                                                }
                                            }
                                        }
                                    ]
                                }
                            ],
                            "name": "Initialize"
                        },
                        {
                            "body": [
                                {
                                    "type": "registeroperation",
                                    "value": {
                                        "register": "$RESULT",
                                        "value": {
                                            "args": ["$PROMPT", "$EMBEDDINGS"],
                                            "name": "multi_inference"
                                        }
                                    }
                                }
                            ],
                            "name": "Summarize"
                        }
                    ],
                    "sticky": true,
                    "version": "0.1"
                }
            }],
            "type": "Workflow"
        });

        let merged_value = Node::merge_tool(&existing_tool_value, &input_value);
        assert_eq!(merged_value, expected_merged_value);
    }

    #[test]
    fn test_merge_tool_configs_update_existing() {
        let existing_tool = json!({
            "content": [{
                "config": [
                    {
                        "BasicConfig": {
                            "key_name": "api_key",
                            "description": "API Key",
                            "required": true,
                            "type_name": "string",
                            "key_value": "old_key"
                        }
                    }
                ]
            }]
        });

        let input_value = json!({
            "content": [{
                "config": [
                    {
                        "BasicConfig": {
                            "key_name": "api_key",
                            "description": "Updated API Key",
                        }
                    }
                ]
            }]
        });

        let merged = Node::merge_tool(&existing_tool, &input_value);
        let merged_config = merged["content"][0]["config"][0]["BasicConfig"].as_object().unwrap();
        assert_eq!(merged_config["description"], "Updated API Key");
    }

    #[test]
    fn test_merge_tool_configs_add_new() {
        let existing_tool = json!({
            "content": [{
                "config": [
                    {
                        "BasicConfig": {
                            "key_name": "api_key",
                            "description": "API Key",
                            "required": true,
                            "type_name": "string",
                            "key_value": "old_key"
                        }
                    }
                ]
            }]
        });

        let input_value = json!({
            "content": [{
                "config": [
                    {
                        "BasicConfig": {
                            "key_name": "new_config",
                            "description": "New Config",
                            "required": false,
                            "type_name": "string",
                            "key_value": "new_value"
                        }
                    }
                ]
            }]
        });

        let merged = Node::merge_tool(&existing_tool, &input_value);
        let merged_configs = merged["content"][0]["config"].as_array().unwrap();
        assert_eq!(merged_configs.len(), 2);
    }

    #[test]
    fn test_merge_tool_configs_empty_existing() {
        let existing_tool = json!({
            "content": [{
                "config": []
            }]
        });

        let input_value = json!({
            "content": [{
                "config": [
                    {
                        "BasicConfig": {
                            "key_name": "new_config",
                            "description": "New Config",
                            "required": false,
                            "type_name": "string",
                            "key_value": "new_value"
                        }
                    }
                ]
            }]
        });

        let merged = Node::merge_tool(&existing_tool, &input_value);
        let merged_configs = merged["content"][0]["config"].as_array().unwrap();
        assert_eq!(merged_configs.len(), 1);
    }

    #[test]
    fn test_merge_tool_configs_update_dont_override_wrong_key() {
        let existing_tool = json!({
            "content": [{
                "config": [
                    {
                        "BasicConfig": {
                            "key_name": "api_key",
                            "description": "API Key",
                            "required": true,
                            "type_name": "string",
                            "key_value": "old_key"
                        }
                    },
                    {
                        "BasicConfig": {
                            "key_name": "api_key2",
                            "description": "API Key 2",
                            "required": true,
                            "type_name": "string",
                            "key_value": "old_key2"
                        }
                    }
                ]
            }]
        });

        let input_value = json!({
            "content": [{
                "config": [
                    {
                        "BasicConfig": {
                            "key_name": "api_key2",
                            "description": "Updated API Key 2",
                        }
                    }
                ]
            }]
        });

        let merged = Node::merge_tool(&existing_tool, &input_value);
        let merged_config = merged["content"][0]["config"][0]["BasicConfig"].as_object().unwrap();
        assert_eq!(merged_config["key_name"], "api_key");
        assert_eq!(merged["content"][0]["config"].as_array().unwrap().len(), 2);
        assert_eq!(merged["content"][0]["config"][1]["BasicConfig"]["key_name"], "api_key2");
    }
}
