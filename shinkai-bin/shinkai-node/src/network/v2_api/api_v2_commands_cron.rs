use crate::{
    cron_tasks::cron_manager::CronManager,
    network::{node_error::NodeError, Node, node_shareable_logic::download_zip_file},
};
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::crontab::{CronTask, CronTaskAction};
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Local;
use std::path::Path;
use std::fs::File;
use std::io::Write;
use tokio::fs;
use zip::{write::FileOptions, ZipWriter};

impl Node {
    pub async fn v2_api_add_cron_task(
        db: Arc<SqliteManager>,
        bearer: String,
        cron: String,
        action: CronTaskAction,
        name: String,
        description: Option<String>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Validate cron expression
        let components: Vec<&str> = cron.split_whitespace().collect();
        if components.len() != 5 {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Invalid Cron Expression".to_string(),
                message: format!(
                    "Cron expression must have exactly 5 components (minute hour day month weekday), found {}. \
                 Example of valid cron: '*/30 * * * *'",
                    components.len()
                ),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Add the cron task
        match db.add_cron_task(&name, description.as_deref(), &cron, &action) {
            Ok(task_id) => {
                let response = json!({ "status": "success", "task_id": task_id });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add cron task: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_list_all_cron_tasks(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Vec<CronTask>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all cron tasks
        match db.get_all_cron_tasks() {
            Ok(tasks) => {
                let _ = res.send(Ok(tasks)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list cron tasks: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_specific_cron_task(
        db: Arc<SqliteManager>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Option<CronTask>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the specific cron task
        match db.get_cron_task(task_id) {
            Ok(task) => {
                let _ = res.send(Ok(task)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get cron task: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_remove_cron_task(
        db: Arc<SqliteManager>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Remove the cron task
        match db.remove_cron_task(task_id) {
            Ok(_) => {
                let response = json!({ "status": "success", "message": "Cron task removed successfully" });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove cron task: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_cron_task_logs(
        db: Arc<SqliteManager>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the logs for the cron task
        match db.get_cron_task_executions(task_id) {
            Ok(logs) => {
                // Map the logs to the desired structure
                let formatted_logs: Vec<_> = logs
                    .into_iter()
                    .map(|log| {
                        json!({
                            "job_id": log.3.as_ref().map_or("", |j| j),
                            "task_id": task_id.to_string(),
                            "execution_time": log.0,
                            "success": log.1,
                            "error_message": log.2.as_ref().map_or("", |e| e)
                        })
                    })
                    .collect();

                let response = json!(formatted_logs);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get cron task logs: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_update_cron_task(
        db: Arc<SqliteManager>,
        bearer: String,
        task_id: i64,
        cron: String,
        action: CronTaskAction,
        name: String,
        description: Option<String>,
        paused: bool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Update the cron task
        match db.update_cron_task(task_id, &name, description.as_deref(), &cron, &action, paused) {
            Ok(_) => {
                let response = json!({ "status": "success", "message": "Cron task updated successfully" });
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update cron task: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_force_execute_cron_task(
        db: Arc<SqliteManager>,
        cron_manager: Arc<Mutex<CronManager>>,
        bearer: String,
        cron_task_id: i64,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Force execute the cron task
        match db.get_cron_task(cron_task_id) {
            Ok(Some(cron_task)) => {
                // Lock the mutex to access the CronManager
                let cron_manager = cron_manager.lock().await;
                if let Err(err) = cron_manager.execute_cron_task_immediately(cron_task).await {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to execute cron task: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                } else {
                    let response = json!({ "status": "success", "message": "Cron task executed successfully" });
                    let _ = res.send(Ok(response)).await;
                }
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Cron task not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve cron task: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_get_cron_schedule(
        db: Arc<SqliteManager>,
        cron_manager: Arc<Mutex<CronManager>>,
        bearer: String,
        res: Sender<Result<Vec<(CronTask, chrono::DateTime<Local>)>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the cron schedule
        match cron_manager.lock().await.get_cron_schedule().await {
            Ok(schedule) => {
                let _ = res.send(Ok(schedule)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get cron schedule: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_import_cron_task(
        db: Arc<SqliteManager>,
        bearer: String,
        url: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Download and validate the zip file
        let zip_contents = match download_zip_file(url, "__cron.json".to_string()).await {
            Ok(contents) => contents,
            Err(err) => {
                let _ = res.send(Err(err)).await;
                return Ok(());
            }
        };

        // Parse the JSON content
        let cron_data: Value = match serde_json::from_slice(&zip_contents.buffer) {
            Ok(data) => data,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid JSON".to_string(),
                    message: format!("Failed to parse cron task JSON: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Extract and validate required fields
        let cron_task = match cron_data.as_object() {
            Some(obj) => {
                let name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| NodeError::from("Missing or invalid 'name' field".to_string()))?;
                let cron = obj.get("cron").and_then(|v| v.as_str()).ok_or_else(|| NodeError::from("Missing or invalid 'cron' field".to_string()))?;
                let action: CronTaskAction = serde_json::from_value(obj.get("action").cloned()
                    .ok_or_else(|| NodeError::from("Missing 'action' field".to_string()))?)
                    .map_err(|e| NodeError::from(format!("Invalid action format: {}", e)))?;
                let description = obj.get("description").and_then(|v| v.as_str()).map(String::from);

                (name.to_string(), cron.to_string(), action, description)
            }
            None => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid JSON".to_string(),
                    message: "JSON must be an object".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Add the cron task to the database
        match db.add_cron_task(&cron_task.0, cron_task.3.as_deref(), &cron_task.1, &cron_task.2) {
            Ok(_) => {
                let response = json!({
                    "status": "success",
                    "message": "Cron task imported successfully"
                });
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Database Error".to_string(),
                    message: format!("Failed to add cron task: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_export_cron_task(
        db: Arc<SqliteManager>,
        bearer: String,
        cron_task_id: i64,
        res: Sender<Result<Vec<u8>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the cron task from the database
        match db.get_cron_task(cron_task_id) {
            Ok(Some(cron_task)) => {
                // Serialize the cron task to JSON bytes
                let cron_task_bytes = match serde_json::to_vec(&cron_task) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize cron task: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };

                // Create a temporary zip file
                let name = format!("cron_task_{}.zip", cron_task_id);
                let path = Path::new(&name);
                let file = match File::create(&path) {
                    Ok(file) => file,
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to create zip file: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };

                let mut zip = ZipWriter::new(file);

                // Add the cron task JSON to the zip file
                if let Err(err) = zip.start_file::<_, ()>("__cron_task.json", FileOptions::default()) {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to create cron task file in zip: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }

                if let Err(err) = zip.write_all(&cron_task_bytes) {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to write cron task data to zip: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }

                // Finalize the zip file
                if let Err(err) = zip.finish() {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to finalize zip file: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }

                // Read the zip file into memory
                match fs::read(&path).await {
                    Ok(file_bytes) => {
                        // Clean up the temporary file
                        if let Err(err) = fs::remove_file(&path).await {
                            eprintln!("Warning: Failed to remove temporary file: {}", err);
                        }
                        let _ = res.send(Ok(file_bytes)).await;
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to read zip file: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Cron task not found: {}", cron_task_id),
                };
                let _ = res.send(Err(api_error)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve cron task: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
