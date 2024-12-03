use crate::{
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{node_error::NodeError, Node},
    tools::{
        llm_language_support::file_support_ts::generate_file_support_ts,
        tool_definitions::definition_generation::{generate_tool_definitions, get_all_deno_tools},
        tool_execution::execution_coordinator::{execute_code, execute_tool},
        tool_generation::v2_create_and_send_job_message,
        tool_prompts::{generate_code_prompt, tool_metadata_implementation_prompt},
    },
};
use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{json, Map, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::{APIError, SendResponseBodyData};
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxName, job::JobLike, shinkai_name::ShinkaiSubidentityType},
    shinkai_message::shinkai_message_schemas::{JobCreationInfo, MessageSchemaType},
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
use shinkai_sqlite::{SqliteManager, SqliteManagerError};
use shinkai_tools_primitives::tools::{
    argument::ToolOutputArg, deno_tools::DenoTool, shinkai_tool::ShinkaiTool, tool_playground::ToolPlayground,
};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use std::{sync::Arc, time::Instant};
use tokio::sync::{Mutex, RwLock};

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use chrono::Utc;

impl Node {
    pub async fn v2_api_search_shinkai_tool(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        query: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Start the timer
        let start_time = Instant::now();

        // Perform the internal search using SqliteManager
        // TODO: implement something like BTS for tools
        match sqlite_manager
            .read()
            .await
            .tool_vector_search(&query, 5, false, true)
            .await
        {
            Ok(tools) => {
                let tools_json = serde_json::to_value(tools).map_err(|err| NodeError {
                    message: format!("Failed to serialize tools: {}", err),
                })?;
                // Log the elapsed time if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    let elapsed_time = start_time.elapsed();
                    let result_count = tools_json.as_array().map_or(0, |arr| arr.len());
                    println!("Time taken for tool search: {:?}", elapsed_time);
                    println!("Number of tool results: {}", result_count);
                }
                let _ = res.send(Ok(tools_json)).await;
                Ok(())
            }
            Err(err) => {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all tools
        match sqlite_manager.read().await.get_all_tool_headers() {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        tool_router_key: String,
        input_value: Value,
        res: Sender<Result<ShinkaiTool, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the full tool from sqlite_manager
        let existing_tool = match sqlite_manager.read().await.get_tool_by_key(&tool_router_key) {
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
        let save_result = sqlite_manager.write().await.update_tool(merged_tool).await;

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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        new_tool: ShinkaiTool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Save the new tool to the LanceShinkaiDb
        let save_result = sqlite_manager.write().await.add_tool(new_tool).await;

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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        payload: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool from the database using get_tool_by_key
        match sqlite_manager.read().await.get_tool_by_key(&payload) {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        payload: ToolPlayground,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // TODO: check that job_id exists
        let mut updated_payload = payload.clone();

        // Create DenoTool
        let tool = DenoTool {
            toolkit_name: {
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
            },
            name: payload.metadata.name.clone(),
            author: payload.metadata.author.clone(),
            js_code: payload.code.clone(),
            tools: payload.metadata.tools.clone(),
            config: payload.metadata.configurations.clone(),
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
        };

        let shinkai_tool = ShinkaiTool::Deno(tool, false); // Same as above
        updated_payload.tool_router_key = Some(shinkai_tool.tool_router_key());

        // Function to handle saving metadata and sending response
        async fn save_metadata_and_respond(
            sqlite_manager: Arc<RwLock<SqliteManager>>,
            res: &Sender<Result<Value, APIError>>,
            updated_payload: ToolPlayground,
            tool: ShinkaiTool,
        ) -> Result<(), NodeError> {
            // Acquire a write lock on the sqlite_manager
            let sqlite_manager_write = sqlite_manager.write().await;

            if let Err(err) = sqlite_manager_write.set_tool_playground(&updated_payload) {
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

        // Create a longer-lived binding for the sqlite_manager clone
        let sqlite_manager_clone = sqlite_manager.clone();
        let sqlite_manager_read = sqlite_manager_clone.read().await;

        match sqlite_manager_read.tool_exists(&shinkai_tool.tool_router_key()) {
            Ok(true) => {
                std::mem::drop(sqlite_manager_read);
                // Tool already exists, update it
                let mut sqlite_manager_write = sqlite_manager.write().await;
                match sqlite_manager_write.update_tool(shinkai_tool).await {
                    Ok(tool) => {
                        std::mem::drop(sqlite_manager_write);
                        save_metadata_and_respond(sqlite_manager, &res, updated_payload, tool).await
                    }
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
                std::mem::drop(sqlite_manager_read);
                let mut sqlite_manager_write = sqlite_manager.write().await;
                match sqlite_manager_write.add_tool(shinkai_tool.clone()).await {
                    Ok(tool) => {
                        std::mem::drop(sqlite_manager_write);
                        save_metadata_and_respond(sqlite_manager, &res, updated_payload, tool).await
                    }
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all playground tools
        match sqlite_manager.read().await.get_all_tool_playground() {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Remove the playground tool from the SqliteManager
        let sqlite_manager_write = sqlite_manager.write().await;
        match sqlite_manager_write.remove_tool_playground(&tool_key) {
            Ok(_) => {
                // Also remove the underlying tool from the SqliteManager
                match sqlite_manager_write.remove_tool(&tool_key) {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the playground tool
        match sqlite_manager.read().await.get_tool_playground(&tool_key) {
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
        db: Arc<ShinkaiDB>,
        language: CodeLanguage,
        tools: Vec<String>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let definitions = generate_tool_definitions(tools, language, sqlite_manager, false).await;
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
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        tool_router_key: String,
        parameters: Map<String, Value>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        extra_config: Option<String>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        signing_secret_key: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Execute the tool directly
        let result = execute_tool(
            bearer,
            node_name,
            db,
            vector_fs,
            sqlite_manager,
            tool_router_key.clone(),
            parameters,
            tool_id,
            app_id,
            llm_provider,
            extra_config,
            identity_manager,
            job_manager,
            encryption_secret_key,
            encryption_public_key,
            signing_secret_key,
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
        db: Arc<ShinkaiDB>,
        tool_type: DynamicToolType,
        code: String,
        tools: Vec<String>,
        parameters: Map<String, Value>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        tool_id: String,
        app_id: String,
        llm_provider: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Execute the tool directly
        let result = execute_code(
            tool_type.clone(),
            code,
            tools,
            parameters,
            None,
            sqlite_manager,
            tool_id,
            app_id,
            llm_provider,
            bearer,
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
        db: Arc<ShinkaiDB>,
        language: CodeLanguage,
        tools: Vec<String>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let tool_definitions =
            match generate_tool_definitions(tools.clone(), language.clone(), sqlite_manager.clone(), true).await {
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
            match tool_metadata_implementation_prompt(language.clone(), "".to_string(), tools.clone()).await {
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

        let library_code =
            match generate_tool_definitions(tools.clone(), language.clone(), sqlite_manager.clone(), false).await {
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

        let header_code =
            match generate_tool_definitions(tools.clone(), language.clone(), sqlite_manager.clone(), true).await {
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
                "availableTools": get_all_deno_tools(sqlite_manager.clone()).await.into_iter().map(|tool| tool.tool_router_key).collect::<Vec<String>>(),
                "libraryCode": library_code.clone(),
                "headers": header_code.clone(),
                "codePrompt": code_prompt.clone(),
                "metadataPrompt": metadata_prompt.clone(),
                "supportLibraryHeaders": generate_file_support_ts(true),
                "supportLibrary": generate_file_support_ts(false),
            })))
            .await;
        Ok(())
    }

    pub async fn generate_tool_implementation(
        bearer: String,
        db: Arc<ShinkaiDB>,
        job_message: JobMessage,
        language: CodeLanguage,
        tools: Vec<String>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        raw: bool,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        // Note: Later (inside v2_job_message), we validate the bearer token again,
        // we do it here to make sure we have a valid bearer token at this point
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        // Generate tool definitions
        let tool_definitions =
            match generate_tool_definitions(tools.clone(), language.clone(), sqlite_manager.clone(), true).await {
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
            false => match generate_code_prompt(language, is_memory_required, prompt, tool_definitions).await {
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

        // We copy the job message and update the content with the custom prompt
        let mut job_message_clone = job_message.clone();
        job_message_clone.content = generate_code_prompt;

        // Send the job message
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
        _sqlite_manager: Arc<RwLock<SqliteManager>>,
        db_clone: Arc<ShinkaiDB>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db_clone.clone(), &res)
            .await
            .is_err()
        {
            return Ok(());
        }

        // We can automatically extract the code (last message from the AI in the job inbox) using the job_id
        let job = match db_clone.get_job_with_options(&job_id, true, true) {
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

        let last_message = {
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())?;
            let messages = match db_clone.get_last_messages_from_inbox(inbox_name.to_string(), 2, None) {
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
            db_clone,
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
        db: Arc<ShinkaiDB>,
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
        db: Arc<ShinkaiDB>,
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
            workflow_code: None,
            workflow_name: None,
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
