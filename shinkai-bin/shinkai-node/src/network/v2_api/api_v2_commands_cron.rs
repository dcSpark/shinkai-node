use crate::network::{node_error::NodeError, Node};
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::crontab::{CronTask, CronTaskAction};
use shinkai_sqlite::SqliteManager;
use std::{sync::Arc, time::Instant};
use tokio::sync::{Mutex, RwLock};

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;


impl Node {
    pub async fn v2_api_add_cron_task(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        cron: String,
        action: CronTaskAction,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Add the cron task
        match sqlite_manager.write().await.add_cron_task(&cron, &action) {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        res: Sender<Result<Vec<CronTask>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List all cron tasks
        match sqlite_manager.read().await.get_all_cron_tasks() {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Option<CronTask>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the specific cron task
        match sqlite_manager.read().await.get_cron_task(task_id) {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Remove the cron task
        match sqlite_manager.write().await.remove_cron_task(task_id) {
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        task_id: i64,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the logs for the cron task
        match sqlite_manager.read().await.get_cron_task_executions(task_id) {
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
}
