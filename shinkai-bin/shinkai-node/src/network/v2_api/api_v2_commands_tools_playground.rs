use crate::{
    network::{node_error::NodeError, Node},
    utils::environment::NodeEnvironment,
};
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::{
    shinkai_tools::{CodeLanguage},
    tool_router_key::ToolRouterKey,
};
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::{
    deno_tools::DenoTool,
    python_tools::PythonTool,
    shinkai_tool::{ShinkaiTool},
    tool_output_arg::ToolOutputArg,
    tool_playground::{ToolPlayground},
};
use std::{path::PathBuf, sync::Arc};

impl Node {
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
        let mut origin_path: PathBuf = PathBuf::from(storage_path.clone());
        origin_path.push(".tools_storage");
        origin_path.push("playground");
        origin_path.push(app_id);
        let origin_files = if origin_path.exists() {
            Some(std::fs::read_dir(&origin_path).map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to read origin directory: {}", e),
            })?)
        } else {
            None
        };
        let mut perm_file_path = PathBuf::from(storage_path.clone());
        perm_file_path.push(".tools_storage");
        perm_file_path.push("tools");
        perm_file_path.push(shinkai_tool.tool_router_key().convert_to_path());

        if perm_file_path.exists() {
            std::fs::remove_dir_all(&perm_file_path).map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to clear destination directory: {}", e),
            })?;
        }

        std::fs::create_dir_all(&perm_file_path).map_err(|e| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to create permanent storage directory: {}", e),
        })?;

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
            true => db.update_tool(shinkai_tool).await,
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
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

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
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let db_write = db;
        match db_write.remove_tool_playground(&tool_key) {
            Ok(_) => {
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
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

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
}
