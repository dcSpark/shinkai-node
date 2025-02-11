use crate::{
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{node_error::NodeError, node_shareable_logic::download_zip_file, Node},
    tools::{
        tool_definitions::definition_generation::{generate_tool_definitions, get_all_deno_tools},
        tool_execution::execution_coordinator::{execute_code, execute_tool_cmd},
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
    shinkai_message::shinkai_message_schemas::{CallbackAction, JobCreationInfo, MessageSchemaType},
    shinkai_utils::{
        job_scope::MinimalJobScope, shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::clone_signature_secret_key,
    },
};
use shinkai_message_primitives::{
    schemas::{
        shinkai_name::ShinkaiName,
        shinkai_tools::{CodeLanguage, DynamicToolType},
    },
    shinkai_message::shinkai_message_schemas::JobMessage,
};
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::tool_types::{OperatingSystem, RunnerType, ToolResult};
use shinkai_tools_primitives::tools::{
    deno_tools::DenoTool,
    error::ToolError,
    python_tools::PythonTool,
    shinkai_tool::{ShinkaiTool, ShinkaiToolWithAssets},
    tool_config::{OAuth, ToolConfig},
    tool_output_arg::ToolOutputArg,
    tool_playground::{ToolPlayground, ToolPlaygroundMetadata},
};
use std::path::PathBuf;
use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{Read, Write},
    result,
    sync::Arc,
    time::Instant,
};
use tokio::fs;
use tokio::{process::Command, sync::Mutex};
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;
use zip::{write::FileOptions, ZipWriter};

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
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all tools
        match db.get_all_tool_headers() {
            Ok(tools) => {
                let response = json!(tools);
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

        // Merge existing_tool_value with input_value
        let merged_value = Self::merge_json(existing_tool_value, input_value);

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
        payload: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool from the database using get_tool_by_key
        match db.get_tool_by_key(&payload) {
            Ok(tool) => {
                let response = json!(tool);
                let _ = res.send(Ok(response)).await;
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
                let tool = DenoTool {
                    name: payload.metadata.name.clone(),
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
                let tool = PythonTool {
                    name: payload.metadata.name.clone(),
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
        if let Some(assets) = payload.assets.clone() {
            for file_name in assets {
                let mut asset_path: PathBuf = origin_path.clone();
                asset_path.push(file_name.clone());
                if !asset_path.exists() {
                    return Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Asset file {} does not exist", file_name.clone()),
                    });
                }
            }
        }

        // Copy asset to permanent tool_storage folder
        // {storage}/tool_storage/{tool_key}.assets/
        let mut perm_file_path = PathBuf::from(storage_path.clone());
        perm_file_path.push(".tools_storage");
        perm_file_path.push("tools");
        perm_file_path.push(shinkai_tool.tool_router_key().convert_to_path());
        std::fs::create_dir_all(&perm_file_path).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to create permanent storage directory: {}", e),
        })?;
        if let Some(assets) = payload.assets.clone() {
            for file_name in assets {
                let mut tool_path = origin_path.clone();
                tool_path.push(file_name.clone());
                let mut perm_path = perm_file_path.clone();
                perm_path.push(file_name.clone());
                println!(
                    "copying {} to {}",
                    tool_path.to_string_lossy(),
                    perm_path.to_string_lossy()
                );
                let _ = std::fs::copy(tool_path, perm_path).map_err(|e| APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!(
                        "Failed to copy asset file {} to permanent storage: {}",
                        file_name.clone(),
                        e
                    ),
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
        llm_provider: String,
        node_name: ShinkaiName,
        mounts: Option<Vec<String>>,
        runner: Option<RunnerType>,
        operating_system: Option<Vec<OperatingSystem>>,
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
            llm_provider,
            bearer,
            node_name,
            mounts,
            runner,
            operating_system,
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

        let library_code = match generate_tool_definitions(tools.clone(), language.clone(), db.clone(), false).await {
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
                "availableTools": get_all_deno_tools(db.clone()).await.into_iter().map(|tool| tool.tool_router_key).collect::<Vec<String>>(),
                "libraryCode": library_code.clone(),
                "headers": header_code.clone(),
                "codePrompt": code_prompt.clone(),
                "metadataPrompt": metadata_prompt.clone(),
            })))
            .await;
        Ok(())
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
        let metadata =
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

        match v2_create_and_send_job_message(
            bearer,
            job_creation_info,
            job.parent_agent_or_llm_provider_id.clone(),
            metadata,
            None,
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

        // Plan
        // Create the user message and add it
        // The user message should be something like: "Update the code to: <code>"

        // Create the AI message and add it
        // Updated code: <code>
        // Done

        // Get the main identity from the identity manager
        let main_identity = {
            let identity_manager = identity_manager.lock().await;
            match identity_manager.get_main_identity() {
                Some(identity) => identity.clone(),
                None => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to get main identity".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Retrieve the job to get the llm_provider
        let llm_provider = match db.get_job_with_options(&job_id, false) {
            Ok(job) => job.parent_agent_or_llm_provider_id.clone(),
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

        // Create a new job message
        let sender = match ShinkaiName::new(main_identity.get_full_identity_name()) {
            Ok(name) => name,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create sender name: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
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
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create recipient name: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
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
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create Shinkai message: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Add the Shinkai message to the job inbox
        let add_message_result = db.add_message_to_job_inbox(&job_id, &shinkai_message, None, None).await;

        if let Err(err) = add_message_result {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to add Shinkai message to job inbox: {}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
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
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to add AI message to job inbox: {}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Send success response
        let response = json!({ "status": "success", "message": "Code update operation successful" });
        let _ = res.send(Ok(response)).await;

        Ok(())
    }

    async fn get_tool_zip(tool: ShinkaiTool, node_env: NodeEnvironment) -> Result<Vec<u8>, NodeError> {
        let mut tool = tool;
        tool.sanitize_config();

        let tool_bytes = serde_json::to_vec(&tool).unwrap();

        let name = format!(
            "{}.zip",
            tool.tool_router_key().to_string_without_version().replace(':', "_")
        );
        let path = std::env::temp_dir().join(&name);
        let file = File::create(&path).map_err(|e| NodeError::from(e.to_string()))?;

        let mut zip = ZipWriter::new(file);

        let assets = PathBuf::from(&node_env.node_storage_path.unwrap_or_default())
            .join(".tools_storage")
            .join("tools")
            .join(tool.tool_router_key().convert_to_path());

        if assets.exists() {
            for entry in std::fs::read_dir(assets).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_file() {
                    zip.start_file::<_, ()>(path.file_name().unwrap().to_str().unwrap(), FileOptions::default())
                        .unwrap();
                    zip.write_all(&fs::read(path).await.unwrap()).unwrap();
                }
            }
        }

        zip.start_file::<_, ()>("__tool.json", FileOptions::default())
            .map_err(|e| NodeError::from(e.to_string()))?;
        zip.write_all(&tool_bytes).map_err(|e| NodeError::from(e.to_string()))?;
        zip.finish().map_err(|e| NodeError::from(e.to_string()))?;

        println!("Zip file created successfully!");
        let file_bytes = fs::read(&path).await?;
        // Delete the zip file after reading it
        fs::remove_file(&path).await?;
        Ok(file_bytes)
    }

    pub async fn v2_api_export_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        node_env: NodeEnvironment,
        tool_key_path: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let sqlite_manager_read = db;
        match sqlite_manager_read.get_tool_by_key(&tool_key_path.clone()) {
            Ok(tool) => {
                let file_bytes = Self::get_tool_zip(tool, node_env).await;
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
        let response = Self::publish_tool(db, node_env, tool_key_path, identity_manager, signing_secret_key).await;

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

        let file_bytes: Vec<u8> = Self::get_tool_zip(tool, node_env).await.map_err(|e| APIError {
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

    pub async fn v2_api_import_tool(
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

        let result = Self::v2_api_import_tool_internal(db, node_env, url, node_name, signing_secret_key).await;
        match result {
            Ok(response) => {
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let _ = res.send(Err(err)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_import_tool_internal(
        db: Arc<SqliteManager>,
        node_env: NodeEnvironment,
        url: String,
        node_name: String,
        signing_secret_key: SigningKey,
    ) -> Result<Value, APIError> {
        let mut zip_contents =
            match download_zip_file(url, "__tool.json".to_string(), node_name, signing_secret_key).await {
                Ok(contents) => contents,
                Err(err) => {
                    return Err(err);
                }
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

        // Check if the tool can be enabled and enable it if possible
        let mut tool = tool.clone();
        if !tool.is_enabled() && tool.can_be_enabled() {
            tool.enable();
        }

        // check if any version of the tool exists in the database
        let db_tool = match db.get_tool_by_key(&tool.tool_router_key().to_string_without_version()) {
            Ok(tool) => Some(tool),
            Err(_) => None,
        };

        // if the tool exists in the database, check if the version is the same or newer
        if let Some(db_tool) = db_tool.clone() {
            let version_db = db_tool.version_number()?;
            let version_zip = tool.version_number()?;
            if version_db >= version_zip {
                // No need to update
                return Ok(json!({
                    "status": "success",
                    "message": "Tool already up-to-date",
                    "tool_key": tool.tool_router_key().to_string_without_version(),
                    "tool": tool.clone()
                }));
            }
        }

        // Save the tool to the database

        let tool = match db_tool {
            None => db.add_tool(tool).await.map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Database Error".to_string(),
                message: format!("Failed to save tool to database: {}", e),
            })?,
            Some(_) => db.upgrade_tool(tool).await.map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Database Error".to_string(),
                message: format!("Failed to upgrade tool: {}", e),
            })?,
        };

        let archive_clone = zip_contents.archive.clone();
        let files = archive_clone.file_names();
        for file in files {
            if file.contains("__MACOSX/") {
                continue;
            }
            if file == "__tool.json" {
                continue;
            }
            let mut buffer = Vec::new();
            {
                let file = zip_contents.archive.by_name(file);
                let mut tool_file = match file {
                    Ok(file) => file,
                    Err(_) => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid Tool Archive".to_string(),
                            message: "Archive does not contain tool.json".to_string(),
                        });
                    }
                };

                // Read the tool file contents into a buffer
                if let Err(err) = tool_file.read_to_end(&mut buffer) {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Read Error".to_string(),
                        message: format!("Failed to read tool.json contents: {}", err),
                    });
                }
            } // `tool_file` goes out of scope here

            let mut file_path = PathBuf::from(&node_env.node_storage_path.clone().unwrap_or_default())
                .join(".tools_storage")
                .join("tools")
                .join(tool.tool_router_key().convert_to_path());
            if !file_path.exists() {
                let s = std::fs::create_dir_all(&file_path);
                if s.is_err() {
                    return Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Failed to create directory".to_string(),
                        message: format!("Failed to create directory: {}", s.err().unwrap()),
                    });
                }
            }
            file_path.push(file);
            let s = std::fs::write(&file_path, &buffer);
            if s.is_err() {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Failed to write file".to_string(),
                    message: format!("Failed to write file: {}", s.err().unwrap()),
                });
            }
        }

        Ok(json!({
            "status": "success",
            "message": "Tool imported successfully",
            "tool_key": tool.tool_router_key().to_string_without_version(),
            "tool": tool
        }))
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

        // Attempt to remove the playground tool first
        let _ = db_write.remove_tool_playground(&tool_key);

        // Remove the tool from the database
        match db_write.remove_tool(&tool_key, None) {
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
        new_tool.update_name(format!(
            "{}_{}",
            original_tool.name(),
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        ));
        new_tool.update_author(node_name.node_name.clone());

        // Try to get the original playground tool, or create one from the tool data
        let new_playground = match db.get_tool_playground(&tool_key_path) {
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
                new_playground
            }
            Err(_) => {
                // Create a new playground from the tool data
                let output = new_tool.output_arg();
                let output_json = output.json;
                let result: ToolResult = ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]);
                println!("output_json: {:?} | result: {:?}", output_json, result);

                ToolPlayground {
                    language: match original_tool {
                        ShinkaiTool::Deno(_, _) => CodeLanguage::Typescript,
                        ShinkaiTool::Python(_, _) => CodeLanguage::Python,
                        _ => CodeLanguage::Typescript, // Default to typescript for other types
                    },
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
                        bearer,
                        llm_provider.id,
                        encryption_secret_key.clone(),
                        encryption_public_key.clone(),
                        signing_secret_key.clone(),
                    )
                    .await?,
                    job_id_history: vec![],
                    code: new_tool.get_code(),
                    assets: new_tool.get_assets(),
                }
            }
        };

        // Add the new tool to the database
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

        // Return the new tool's router key
        let response = json!({
            "tool_router_key": new_tool.tool_router_key().to_string_without_version(),
            "version": new_tool.version(),
            "job_id": new_playground.job_id,
        });
        Ok(response)
    }

    pub async fn process_tool_zip(
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
        let mut buffer = Vec::new();
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

        // Save the tool to the database
        match db.add_tool(tool).await {
            Ok(tool) => {
                // Process all files except __tool.json
                let mut files_to_process = Vec::new();
                for i in 0..archive.len() {
                    if let Ok(mut file) = archive.by_index(i) {
                        let name = file.name().to_string();
                        if name != "__tool.json" {
                            // Read the file data into memory
                            let mut buffer = Vec::new();
                            if let Err(err) = file.read_to_end(&mut buffer) {
                                return Err(APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Read Error".to_string(),
                                    message: format!("Failed to read file {}: {}", name, err),
                                });
                            }
                            files_to_process.push((name, buffer));
                        }
                    }
                }

                // Process the files after reading them all into memory
                for (name, buffer) in files_to_process {
                    // Create the directory structure if it doesn't exist
                    let file_path = PathBuf::from(&node_env.node_storage_path.clone().unwrap_or_default())
                        .join(".tools_storage")
                        .join("tools")
                        .join(tool.tool_router_key().convert_to_path());
                    if !file_path.exists() {
                        if let Err(err) = std::fs::create_dir_all(&file_path) {
                            return Err(APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Failed to create directory".to_string(),
                                message: format!("Failed to create directory: {}", err),
                            });
                        }
                    }

                    // Write the file
                    let asset_path = file_path.join(&name);
                    if let Err(err) = std::fs::write(asset_path, buffer) {
                        return Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Failed to write file".to_string(),
                            message: format!("Failed to write file {}: {}", name, err),
                        });
                    }
                }

                Ok(json!({
                    "status": "success",
                    "message": "Tool imported successfully",
                    "tool_key": tool.tool_router_key().to_string_without_version(),
                    "tool": tool
                }))
            }
            Err(err) => Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Database Error".to_string(),
                message: format!("Failed to save tool to database: {}", err),
            }),
        }
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

        let result = Self::process_tool_zip(db, node_env, file_data).await;
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
        bearer: String,
        node_env: NodeEnvironment,
        code: String,
        metadata: Value,
        assets: Option<Vec<String>>,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        parameters: Value,
        config: Value,
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
        code: String,
        metadata: Value,
        assets: Option<Vec<String>>,
        language: CodeLanguage,
        tools: Vec<ToolRouterKey>,
        parameters: Value,
        config: Value,
        oauth: Option<Vec<OAuth>>,
        _tool_id: String,
        app_id: String,
        llm_provider: String,
        bearer: String,
        node_env: NodeEnvironment,
        db: Arc<SqliteManager>,
    ) -> Result<Value, APIError> {
        // Create temporal directory
        let mut temp_dir = std::env::temp_dir();
        temp_dir.push(format!("shinkai_playground_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to create temporary directory".to_string(),
            message: e.to_string(),
        })?;
        let mut files_created = HashMap::new();
        let tool_list: Vec<ToolRouterKey> = db
            .get_all_tool_headers()
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to get tool headers".to_string(),
                message: e.to_string(),
            })?
            .into_iter()
            .map(|tool| ToolRouterKey::from_string(&tool.tool_router_key))
            .filter(|tool| tool.is_ok())
            .map(|tool| tool.unwrap())
            .collect::<Vec<ToolRouterKey>>();
        let tool_definitions = generate_tool_definitions(tool_list, language.clone(), db, false).await?;
        for (tool_key, tool_definition) in tool_definitions {
            let mut tool_file = temp_dir.clone();
            match language.clone() {
                CodeLanguage::Typescript => {
                    tool_file.push(format!("{}.ts", tool_key));
                }
                CodeLanguage::Python => {
                    tool_file.push(format!("{}.py", tool_key));
                }
            }
            files_created.insert(tool_file.clone(), tool_definition.clone());
            fs::write(&tool_file, tool_definition.clone())
                .await
                .map_err(|e| APIError {
                    code: 500,
                    error: "Failed to write tool file".to_string(),
                    message: e.to_string(),
                })?;
        }

        // Create tool file
        let tool_filename = match language.clone() {
            CodeLanguage::Typescript => "tool.ts",
            CodeLanguage::Python => "tool.py",
            _ => {
                return Err(APIError {
                    code: 400,
                    error: "Unsupported Language".to_string(),
                    message: "Only TypeScript and Python are supported".to_string(),
                })
            }
        };
        let mut tool_file = temp_dir.clone();
        tool_file.push(tool_filename);
        files_created.insert(tool_file.clone(), code.clone());
        fs::write(&tool_file, code.clone()).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to write tool file".to_string(),
            message: e.to_string(),
        })?;

        // Create metadata.json
        let mut metadata_file = temp_dir.clone();
        metadata_file.push("metadata.json");
        let metadata_string = serde_json::to_string_pretty(&metadata).map_err(|e| APIError {
            code: 500,
            error: "Failed to serialize metadata".to_string(),
            message: e.to_string(),
        })?;
        files_created.insert(metadata_file.clone(), metadata_string.clone());
        fs::write(&metadata_file, metadata_string.clone())
            .await
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to write metadata file".to_string(),
                message: e.to_string(),
            })?;

        // Copy assets if any
        if let Some(asset_files) = assets {
            let mut assets_dir = temp_dir.clone();
            assets_dir.push("assets");
            fs::create_dir_all(&assets_dir).await.map_err(|e| APIError {
                code: 500,
                error: "Failed to create assets directory".to_string(),
                message: e.to_string(),
            })?;

            for asset in asset_files {
                let mut source_path = PathBuf::from(".temporal_store");
                source_path.push(&app_id);
                source_path.push(&asset);

                let mut dest_path = assets_dir.clone();
                dest_path.push(&asset);

                fs::copy(source_path, dest_path).await.map_err(|e| APIError {
                    code: 500,
                    error: "Failed to copy asset".to_string(),
                    message: e.to_string(),
                })?;
            }
        }

        let api_url = format!(
            "http://{}:{}",
            node_env.api_listen_address.ip(),
            node_env.api_listen_address.port()
        );
        let tool_type = match language.clone() {
            CodeLanguage::Typescript => "denodynamic",
            CodeLanguage::Python => "pythondynamic",
            _ => unreachable!(),
        };
        let language_extension = match language.clone() {
            CodeLanguage::Typescript => "ts",
            CodeLanguage::Python => "py",
            _ => unreachable!(),
        };

        // Create launch.js
        let launch_script = format!(
            r#"
// Import required modules
const fs = require('fs');

// Configuration variables
const API_URL = "{api_url}";
const AUTH_TOKEN = "{bearer}";
const APP_ID = "{app_id}";
const LLM_PROVIDER = "{llm_provider}";
const TOOL_TYPE = "{tool_type}";

// Read file contents
const CODE_CONTENT = fs.readFileSync('./tool.{language_extension}', 'utf8');
const TOOLS_CONTENT = JSON.parse(fs.readFileSync('./tools.json', 'utf8'));
const CONFIG_CONTENT = JSON.parse(fs.readFileSync('./config.json', 'utf8'));
const PARAMETERS_CONTENT = JSON.parse(fs.readFileSync('./parameters.json', 'utf8'));
const OAUTH_CONTENT = JSON.parse(fs.readFileSync('./oauth.json', 'utf8'));

// Make the API call
async function makeApiCall() {{
    const body = {{
        code: CODE_CONTENT,
        tools: TOOLS_CONTENT,
        tool_type: TOOL_TYPE,
        llm_provider: LLM_PROVIDER,
        extra_config: CONFIG_CONTENT,
        parameters: PARAMETERS_CONTENT,
        oauth: OAUTH_CONTENT
    }};
    console.log(body);
    try {{
        const response = await fetch(`${{API_URL}}/v2/code_execution`, {{
            method: 'POST',
            headers: {{
                'Authorization': `Bearer ${{AUTH_TOKEN}}`,
                'x-shinkai-tool-id': 'run',
                'x-shinkai-app-id': APP_ID,
                'x-shinkai-llm-provider': LLM_PROVIDER,
                'Content-Type': 'application/json; charset=utf-8'
            }},
            body: JSON.stringify(body)
        }});

        const data = await response.json();
        return data;
    }} catch (error) {{
        return error;
    }}
}}


// Add new function to fetch log file
async function fetchLogFile(filePath) {{
    const encodedPath = encodeURIComponent(filePath);
    try {{
        const response = await fetch(`${{API_URL}}/v2/resolve_shinkai_file_protocol?file=${{encodedPath}}`, {{
            headers: {{
                'Authorization': `Bearer ${{AUTH_TOKEN}}`
            }}
        }});
        const data = await response.text();
        return data;
    }} catch (error) {{
        console.error('Error fetching log:', error);
        return null;
    }}
}}

// Execute the API call
async function start() {{
    try {{
        console.log('Tool Execution Started at ', new Date().toISOString());
        let data = await makeApiCall();
        // Check for log files in the response
        if (data.message && data.message.match(/Files: shinkai:\/\/file\//)) {{
            const file = data.message.match(/shinkai:\/\/file\/[@a-zA-Z0-9/_.-]+/);
            if (file) {{
                const logContent = await fetchLogFile(file[0]);
                console.log('Log content:', logContent);
            }} else {{
                console.log('No file found in the response.');
            }}
        }}
        if (data.__created_files__) {{
            for (const filePath of data.__created_files__) {{
                if (filePath.endsWith('.log')) {{
                    console.log('--------------------------------');
                    console.log('Fetching log file:', filePath);
                    const logContent = await fetchLogFile(filePath);
                    console.log('Log content:', logContent);
                    console.log('--------------------------------');
                }}
            }}
        }}
        console.log('Tool Execution Completed at ', new Date().toISOString());
        console.log('Response:\n', JSON.stringify(data, null, 2));
    }} catch(error) {{
        console.error('Error:', error);
    }}
}}

start().then(() => {{
    console.log('All operations completed');
}});


"#
        );
        let mut launch_file = temp_dir.clone();
        launch_file.push("launch.js");
        files_created.insert(launch_file.clone(), launch_script.clone());
        fs::write(&launch_file, launch_script.clone())
            .await
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to write launch.sh".to_string(),
                message: e.to_string(),
            })?;

        // Set file permissions in a cross-platform way
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perm = std::fs::Permissions::from_mode(0o755);
            fs::set_permissions(&launch_file, perm).await.map_err(|e| APIError {
                code: 500,
                error: "Failed to set permissions".to_string(),
                message: e.to_string(),
            })?;
        }

        // On Windows, executable permissions don't exist in the same way
        #[cfg(windows)]
        {
            // Windows doesn't need special executable permissions for .js files
        }

        // Create .vscode/launch.json
        let mut vscode_dir = temp_dir.clone();
        vscode_dir.push(".vscode");
        fs::create_dir_all(&vscode_dir).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to create .vscode directory".to_string(),
            message: e.to_string(),
        })?;

        let launch_json = json!({
            "version": "2.0.0",
            "configurations": [{
                "name": "Launch Tool",
                "program": "${workspaceFolder}/launch.js",
                "request": "launch",
                "type": "node"
            }]
        });
        let mut launch_json_file = vscode_dir.clone();
        launch_json_file.push("launch.json");
        let launch_json_string = serde_json::to_string_pretty(&launch_json).map_err(|e| APIError {
            code: 500,
            error: "Failed to serialize launch.json".to_string(),
            message: e.to_string(),
        })?;
        files_created.insert(launch_json_file.clone(), launch_json_string.clone());
        fs::write(&launch_json_file, launch_json_string.clone())
            .await
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to write launch.json".to_string(),
                message: e.to_string(),
            })?;

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

        // Create parameters.json
        let mut parameters_file = temp_dir.clone();
        parameters_file.push("parameters.json");
        let parameters_content = serde_json::to_string_pretty(&parameters).map_err(|e| APIError {
            code: 500,
            error: "Failed to serialize parameters".to_string(),
            message: e.to_string(),
        })?;
        files_created.insert(parameters_file.clone(), parameters_content.clone());
        fs::write(&parameters_file, parameters_content)
            .await
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to write parameters.json".to_string(),
                message: e.to_string(),
            })?;

        // Create tools.json
        let mut tools_file = temp_dir.clone();
        tools_file.push("tools.json");
        let tools_content = serde_json::to_string_pretty(
            &tools
                .iter()
                .map(|tool| tool.to_string_without_version())
                .collect::<Vec<String>>(),
        )
        .map_err(|e| APIError {
            code: 500,
            error: "Failed to serialize tools".to_string(),
            message: e.to_string(),
        })?;
        files_created.insert(tools_file.clone(), tools_content.clone());
        fs::write(&tools_file, tools_content).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to write tools.json".to_string(),
            message: e.to_string(),
        })?;

        // Create config.json
        let mut config_file = temp_dir.clone();
        config_file.push("config.json");
        let config_content = serde_json::to_string_pretty(&config).map_err(|e| APIError {
            code: 500,
            error: "Failed to serialize config".to_string(),
            message: e.to_string(),
        })?;
        files_created.insert(config_file.clone(), config_content.clone());
        fs::write(&config_file, config_content).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to write config.json".to_string(),
            message: e.to_string(),
        })?;

        // Create oauth.json if OAuth data is provided
        let oauth_content = if let Some(oauth_data) = oauth {
            serde_json::to_string_pretty(&oauth_data).map_err(|e| APIError {
                code: 500,
                error: "Failed to serialize OAuth data".to_string(),
                message: e.to_string(),
            })?
        } else {
            "[]".to_string()
        };
        let mut oauth_file: PathBuf = temp_dir.clone();
        oauth_file.push("oauth.json");
        files_created.insert(oauth_file.clone(), oauth_content.clone());
        fs::write(&oauth_file, oauth_content).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to write oauth.json".to_string(),
            message: e.to_string(),
        })?;

        // Let's copy assets if any
        let node_storage_path = node_env.node_storage_path.clone().unwrap_or_else(|| "".to_string());
        let assets_path = PathBuf::from(&node_storage_path)
            .join(".tools_storage")
            .join("playground")
            .join(app_id.clone());

        let mut asset_folder = temp_dir.clone();
        asset_folder.push("assets");
        fs::create_dir_all(&asset_folder).await.map_err(|e| APIError {
            code: 500,
            error: "Failed to create assets directory".to_string(),
            message: e.to_string(),
        })?;

        if assets_path.exists() {
            for entry in std::fs::read_dir(assets_path).map_err(|e| APIError {
                code: 500,
                error: "Failed to read assets directory".to_string(),
                message: e.to_string(),
            })? {
                let entry = entry.map_err(|e| APIError {
                    code: 500,
                    error: "Failed to read directory entry".to_string(),
                    message: e.to_string(),
                })?;
                let path = entry.path();
                if path.is_file() {
                    let mut asset_file = asset_folder.clone();
                    asset_file.push(path.file_name().unwrap().to_string_lossy().to_string());
                    files_created.insert(asset_file.clone(), path.to_string_lossy().to_string());
                    fs::copy(path, asset_file).await.map_err(|e| APIError {
                        code: 500,
                        error: "Failed to copy asset".to_string(),
                        message: e.to_string(),
                    })?;
                }
            }
        }

        // Create README.md
        let readme_content = format!(
            r#"# Shinkai Tool Playground

This is a standalone playground for testing your Shinkai tool.

## Structure
- `{}`: The main tool implementation
- `*.ts` or `*.py`: Tool definition files for dependencies
- `metadata.json`: Tool metadata and configuration
- `parameters.json`: Runtime parameters passed to your tool (modify this to test different inputs)
- `tools.json`: Array of tool keys that your tool depends on
- `config.json`: Additional configuration options for tool execution
- `oauth.json`: OAuth credentials configuration
- `assets/`: Directory containing tool assets
- `launch.js`: Script to execute the tool
- `.vscode/`: VS Code configuration for easy execution
- `README.md`: This file

## Configuration Files
- `parameters.json`: Contains the input parameters that will be passed to your tool during execution. Modify this file to test how your tool behaves with different inputs.
- `tools.json`: Lists the tool dependencies required by your implementation. Add tool keys here if your code needs to call other tools.
- `config.json`: Holds additional configuration options that affect tool execution. Use this for environment-specific settings.
- `oauth.json`: Contains OAuth configuration if your tool requires authentication.
- `metadata.json`: Defines your tool's metadata like name, description, version etc.

## Running the Tool
1. Make sure you have Shinkai running locally
2. Configure your test setup:
   - Update input parameters in `parameters.json`
   - Add required tool dependencies to `tools.json`
   - Modify execution settings in `config.json`
   - Configure OAuth settings in `oauth.json` if needed
3. Run the tool using either:
   - VS Code's debug launcher
   - Execute `node launch.js` from the terminal

## Dependencies
You will need node.js to run the tool. If you don't have it, you can install it from [here](https://nodejs.org/en/download/).

Happy coding!"#,
            tool_filename
        );
        let mut readme_file = temp_dir.clone();
        readme_file.push("README.md");
        files_created.insert(readme_file.clone(), readme_content.clone());
        fs::write(&readme_file, readme_content.clone())
            .await
            .map_err(|e| APIError {
                code: 500,
                error: "Failed to write README file".to_string(),
                message: e.to_string(),
            })?;

        Ok(json!({
            "status": "success",
            "playground_path": temp_dir.to_string_lossy().to_string(),
            "files": files_created
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use shinkai_embedding::model_type::EmbeddingModelType;
    use shinkai_embedding::model_type::OllamaTextEmbeddingsInference;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

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

        let merged_value = Node::merge_json(existing_tool_value, input_value);
        assert_eq!(merged_value, expected_merged_value);
    }
}
