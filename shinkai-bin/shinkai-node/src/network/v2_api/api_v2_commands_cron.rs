use crate::network::{node_error::NodeError, Node};
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::crontab::{CronTask, CronTaskAction};
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::RwLock;

impl Node {
    pub async fn v2_api_add_cron_task(
        db: Arc<RwLock<SqliteManager>>,
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

        // Add the cron task
        match db.write().await.add_cron_task(&name, &description, &cron, &action) {
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
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        res: Sender<Result<Vec<CronTask>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all cron tasks
        match db.read().await.get_all_cron_tasks() {
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
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Option<CronTask>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the specific cron task
        match db.read().await.get_cron_task(task_id) {
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
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Remove the cron task
        match db.write().await.remove_cron_task(task_id) {
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
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the logs for the cron task
        match db.read().await.get_cron_task_executions(task_id) {
            Ok(logs) => {
                let response = json!(logs);
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
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        task_id: i64,
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

        // Update the cron task
        match db.write().await.update_cron_task(task_id, &name, &description, &cron, &action) {
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
}
