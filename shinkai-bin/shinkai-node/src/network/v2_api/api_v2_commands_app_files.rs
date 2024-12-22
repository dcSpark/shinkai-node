use crate::network::node_error::NodeError;
use crate::network::Node;
use crate::utils::environment::NodeEnvironment;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_sqlite::SqliteManager;

use std::path::PathBuf;
use std::sync::Arc;

use shinkai_http_api::node_api_router::APIError;

pub fn get_app_folder_path(node_env: NodeEnvironment, app_id: String) -> PathBuf {
    let mut origin_path: PathBuf = PathBuf::from(node_env.node_storage_path.clone().unwrap_or_default());
    origin_path.push("app_files");
    origin_path.push(app_id);
    origin_path
}

impl Node {
    pub async fn v2_api_upload_app_file(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_id: String,
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
        let app_folder_path = get_app_folder_path(node_env, app_id);
        if !app_folder_path.exists() {
            let result = std::fs::create_dir_all(&app_folder_path);
            if let Err(err) = result {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create directory: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }
        let file_path = app_folder_path.join(file_name.clone());
        if let Err(err) = std::fs::write(&file_path, file_data) {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to write file: {}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        let _ = res.send(Ok(Value::String(file_name))).await;
        Ok(())
    }

    pub async fn v2_api_get_app_file(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        node_env: NodeEnvironment,
        res: Sender<Result<Vec<u8>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let app_folder_path = get_app_folder_path(node_env, app_id);
        let file_path = app_folder_path.join(file_name.clone());
        if !file_path.exists() {
            let api_error = APIError {
                code: StatusCode::NOT_FOUND.as_u16(),
                error: "Not Found".to_string(),
                message: format!("File {} not found", file_name),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        let file_bytes = std::fs::read(&file_path)?;
        let _ = res.send(Ok(file_bytes)).await;
        Ok(())
    }

    pub async fn v2_api_update_app_file(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_id: String,
        app_id: String,
        file_name: String,
        new_name: Option<String>,
        file_data: Option<Vec<u8>>,
        node_env: NodeEnvironment,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let app_folder_path = get_app_folder_path(node_env, app_id);
        let file_path = app_folder_path.join(file_name.clone());
        if !file_path.exists() {
            let api_error = APIError {
                code: StatusCode::NOT_FOUND.as_u16(),
                error: "Not Found".to_string(),
                message: format!("File {} not found", file_name),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        if let Some(file_data) = file_data {
            std::fs::write(&file_path, file_data)?;
        }

        if let Some(new_name) = new_name.clone() {
            let new_file_path = app_folder_path.join(new_name.clone());
            std::fs::rename(&file_path, &new_file_path)?;
        }

        let _ = res.send(Ok(Value::String(new_name.unwrap_or_default()))).await;
        Ok(())
    }

    pub async fn v2_api_list_app_files(
        db: Arc<SqliteManager>,
        bearer: String,
        tool_id: String,
        app_id: String,
        node_env: NodeEnvironment,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let app_folder_path = get_app_folder_path(node_env, app_id);
        let result = Self::v2_api_list_app_files_internal(app_folder_path);
        match result {
            Ok(file_list) => {
                let _ = res
                    .send(Ok(Value::Array(
                        file_list.iter().map(|file| Value::String(file.clone())).collect(),
                    )))
                    .await;
                Ok(())
            }
            Err(err) => {
                let _ = res.send(Err(err)).await;
                Ok(())
            }
        }
    }

    pub fn v2_api_list_app_files_internal(app_folder_path: PathBuf) -> Result<Vec<String>, APIError> {
        let files = std::fs::read_dir(&app_folder_path);
        if let Err(err) = files {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to read directory: {}", err),
            };
            return Err(api_error);
        }
        let mut file_list = Vec::new();

        let files = files.unwrap();
        for file in files {
            if let Ok(file) = file {
                let file_name = file.file_name().to_string_lossy().to_string();
                file_list.push(file_name);
            }
        }
        Ok(file_list)
    }

    pub async fn v2_api_delete_app_file(
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
        let app_folder_path = get_app_folder_path(node_env, app_id);
        let file_path = app_folder_path.join(file_name.clone());
        if !file_path.exists() {
            let api_error = APIError {
                code: StatusCode::NOT_FOUND.as_u16(),
                error: "Not Found".to_string(),
                message: format!("File {} not found", file_name),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        std::fs::remove_file(&file_path)?;
        let _ = res.send(Ok(Value::String(file_name))).await;
        Ok(())
    }
}
