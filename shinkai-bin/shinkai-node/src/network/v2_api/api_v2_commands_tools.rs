use crate::{
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{node_error::NodeError, Node},
    tools::{
        tool_definitions::definition_generation::{generate_tool_definitions, get_all_deno_tools},
        tool_execution::execution_coordinator::{execute_code, execute_tool_cmd},
        tool_generation::v2_create_and_send_job_message,
        tool_prompts::{generate_code_prompt, tool_metadata_implementation_prompt},
    },
    utils::environment::NodeEnvironment,
};
use std::io::Read;

use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{json, Map, Value};

use shinkai_http_api::node_api_router::{APIError, SendResponseBodyData};
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxName, job::JobLike, job_config::JobConfig, shinkai_name::ShinkaiSubidentityType},
    shinkai_message::shinkai_message_schemas::{CallbackAction, JobCreationInfo, MessageSchemaType},
    shinkai_utils::{
        job_scope::JobScope, shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key,
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
use shinkai_tools_primitives::tools::{
    deno_tools::DenoTool,
    error::ToolError,
    python_tools::PythonTool,
    shinkai_tool::ShinkaiTool,
    tool_config::{OAuth, ToolConfig},
    tool_output_arg::ToolOutputArg,
    tool_playground::ToolPlayground,
};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use std::{fs::File, io::Write, path::Path, sync::Arc, time::Instant};
use tokio::sync::Mutex;
use zip::{write::FileOptions, ZipWriter};

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use chrono::Utc;

use std::path::PathBuf;
use tokio::fs;

impl Node {
    /// Searches for Shinkai tools using both vector and full-text search (FTS) methods.
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
            db.tool_vector_search_with_vector_limited(embedding, 5, tools)
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
        new_tool: ShinkaiTool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Save the new tool to the LanceShinkaiDb
        let save_result = db.add_tool(new_tool).await;

        match save_result {
            Ok(tool) => {
                let tool_key = tool.tool_router_key();
                let response = json!({ "status": "success", "message": format!("Tool added with key: {}", tool_key) });
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
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let toolkit_name = {
            let name = format!(
                "{}_{}",
                payload
                    .metadata
                    .name
                    .to_lowercase()
                    .replace(" ", "_")
                    .replace("-", "_")
                    .replace(":", "_"),
                payload
                    .metadata
                    .author
                    .to_lowercase()
                    .replace(" ", "_")
                    .replace("-", "_")
                    .replace(":", "_")
            );
            // Use a regex to filter out unwanted characters
            let re = regex::Regex::new(r"[^a-z0-9_]").unwrap();
            re.replace_all(&name, "").to_string()
        };

        let mut updated_payload = payload.clone();

        let shinkai_tool = match payload.language {
            CodeLanguage::Typescript => {
                let tool = DenoTool {
                    toolkit_name,
                    name: payload.metadata.name.clone(),
                    author: payload.metadata.author.clone(),
                    js_code: payload.code.clone(),
                    tools: payload.metadata.tools.clone(),
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
                };
                ShinkaiTool::Deno(tool, false)
            }
            CodeLanguage::Python => {
                let tool = PythonTool {
                    toolkit_name,
                    name: payload.metadata.name.clone(),
                    author: payload.metadata.author.clone(),
                    py_code: payload.code.clone(),
                    tools: payload.metadata.tools.clone(),
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
                };
                ShinkaiTool::Python(tool, false)
            }
        };

        updated_payload.tool_router_key = Some(shinkai_tool.tool_router_key());

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
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Asset file {} does not exist", file_name.clone()),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        }

        // Copy asset to permanent tool_storage folder {storage}/tool_storage/{tool_key}.assets/
        let mut perm_file_path = PathBuf::from(storage_path.clone());
        perm_file_path.push(".tools_storage");
        perm_file_path.push("tools");
        perm_file_path.push(shinkai_tool.tool_router_key());
        if let Err(err) = std::fs::create_dir_all(&perm_file_path) {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to create permanent storage directory: {}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
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
                let copy_res = std::fs::copy(tool_path, perm_path);
                if copy_res.is_err() {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!(
                            "Failed to copy asset file {} to permanent storage: {}",
                            file_name.clone(),
                            copy_res.err().unwrap()
                        ),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        }

        // Function to handle saving metadata and sending response
        async fn save_metadata_and_respond(
            db: Arc<SqliteManager>,
            res: &Sender<Result<Value, APIError>>,
            updated_payload: ToolPlayground,
            tool: ShinkaiTool,
        ) -> Result<(), NodeError> {
            // Acquire a write lock on the db
            let db_write = db;

            if let Err(err) = db_write.set_tool_playground(&updated_payload) {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to save playground tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            match serde_json::to_value(&tool) {
                Ok(tool_json) => {
                    let response = json!({
                        "shinkai_tool": tool_json,
                        "metadata": updated_payload
                    });
                    let _ = res.send(Ok(response)).await;
                }
                Err(_) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to serialize tool to JSON".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                }
            }

            Ok(())
        }

        // Create a longer-lived binding for the db clone

        match db.tool_exists(&shinkai_tool.tool_router_key()) {
            Ok(true) => {
                // Tool already exists, update it
                match db.update_tool(shinkai_tool).await {
                    Ok(tool) => save_metadata_and_respond(db, &res, updated_payload, tool).await,
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to update tool in SqliteManager: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Ok(false) => {
                // Add the tool to the LanceShinkaiDb
                match db.add_tool(shinkai_tool.clone()).await {
                    Ok(tool) => save_metadata_and_respond(db, &res, updated_payload, tool).await,
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to add tool to SqliteManager: {}", err),
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
                    message: format!("Failed to check if tool exists: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
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
                match db_write.remove_tool(&tool_key) {
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
        tools: Vec<String>,
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
        vector_fs: Arc<VectorFS>,
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
            vector_fs,
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
        tools: Vec<String>,
        parameters: Map<String, Value>,
        extra_config: Map<String, Value>,
        oauth: Option<Vec<OAuth>>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        node_name: ShinkaiName,
        mounts: Option<Vec<String>>,
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
        tools: Vec<String>,
        code: String,
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

        let is_memory_required = tools
            .iter()
            .any(|tool| tool.contains("local:::rust_toolkit:::shinkai_sqlite_query_executor"));
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

        let metadata_prompt =
            match tool_metadata_implementation_prompt(language.clone(), code.clone(), tools.clone()).await {
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
        tools: Vec<String>,
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
        let is_memory_required = tools
            .clone()
            .iter()
            .any(|tool| tool.contains("local:::rust_toolkit:::shinkai_sqlite_query_executor"));

        // Determine the code generation prompt so we can update the message with the custom prompt if required
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
            res,
        )
        .await
    }

    pub async fn generate_tool_metadata_implementation(
        bearer: String,
        job_id: String,
        language: CodeLanguage,
        tools: Vec<String>,
        db: Arc<SqliteManager>,
        node_name_clone: ShinkaiName,
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

        // We can automatically extract the code (last message from the AI in the job inbox) using the job_id
        let job = match db.get_job_with_options(&job_id, true, true) {
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

        // Generate the implementation
        let metadata = match tool_metadata_implementation_prompt(language, code, tools).await {
            Ok(metadata) => metadata,
            Err(err) => {
                let _ = res.send(Err(err)).await;
                return Ok(());
            }
        };

        // We auto create a new job with the same configuration as the one from job_id
        let job_creation_info = JobCreationInfo {
            scope: job.scope_with_files().cloned().unwrap_or(JobScope::new_default()),
            is_hidden: Some(job.is_hidden()),
            associated_ui: None,
        };

        match v2_create_and_send_job_message(
            bearer,
            job_creation_info,
            job.parent_agent_or_llm_provider_id.clone(),
            metadata,
            db,
            node_name_clone,
            identity_manager_clone,
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

        // Determine if it's an AI or user message, if it's a user message then we need to return an error
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
        // Update the scheduled time to now so the messages are content wise the same but produce a different hash
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
        let llm_provider = match db.get_job_with_options(&job_id, false, false) {
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
            files_inbox: "".to_string(),
            parent: None,
            sheet_job_data: None,
            callback: None,
            metadata: None,
            tool_key: None,
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
            "".to_string(),
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
                let tool_bytes = serde_json::to_vec(&tool).unwrap();

                let name = format!("{}.zip", tool.tool_router_key().replace(':', "_"));
                let path = Path::new(&name);
                let file = File::create(&path).map_err(|e| NodeError::from(e.to_string()))?;

                let mut zip = ZipWriter::new(file);

                let assets = PathBuf::from(&node_env.node_storage_path.unwrap_or_default())
                    .join(".tools_storage")
                    .join("tools")
                    .join(tool.tool_router_key());
                if assets.exists() {
                    for entry in std::fs::read_dir(assets).unwrap() {
                        let entry = entry.unwrap();
                        let path = entry.path();
                        if path.is_file() {
                            zip.start_file::<_, ()>(
                                path.file_name().unwrap().to_str().unwrap(),
                                FileOptions::default(),
                            )
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
                fs::remove_file(&path)
                    .await
                    .map_err(|e| NodeError::from(e.to_string()))?;
                let _ = res.send(Ok(file_bytes)).await;
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

    pub async fn v2_api_import_tool(
        db: Arc<SqliteManager>,
        bearer: String,
        node_env: NodeEnvironment,
        url: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let result = Self::v2_api_import_tool_internal(db, node_env, url).await;
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

    async fn v2_api_import_tool_internal(
        db: Arc<SqliteManager>,
        node_env: NodeEnvironment,
        url: String,
    ) -> Result<Value, APIError> {
        // Download the zip file
        let response = match reqwest::get(&url).await {
            Ok(response) => response,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Download Failed".to_string(),
                    message: format!("Failed to download tool from URL: {}", err),
                });
            }
        };

        // Get the bytes from the response
        let bytes = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(err) => {
                return Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Download Failed".to_string(),
                    message: format!("Failed to read response bytes: {}", err),
                });
            }
        };

        // Create a cursor from the bytes
        let cursor = std::io::Cursor::new(bytes);

        // Create a zip archive from the cursor
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
            let mut tool_file = match archive.by_name("__tool.json") {
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
        let mut db_write = db;
        match db_write.add_tool(tool).await {
            Ok(tool) => {
                let archive_clone = archive.clone();
                let files = archive_clone.file_names();
                for file in files {
                    println!("File: {:?}", file);
                    if file == "__tool.json" {
                        continue;
                    }
                    let mut buffer = Vec::new();
                    {
                        let file = archive.by_name(file);
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
                        .join(tool.tool_router_key());
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
                    "tool_key": tool.tool_router_key(),
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
        tool_id: String,
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
        tool_id: String,
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
        match db_write.remove_tool(&tool_key) {
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
                    "version": "v0.1"
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
                    "version": "v0.1"
                }
            }],
            "type": "Workflow"
        });

        let merged_value = Node::merge_json(existing_tool_value, input_value);
        assert_eq!(merged_value, expected_merged_value);
    }
}
