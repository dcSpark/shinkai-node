use std::{sync::Arc, time::Instant};

use async_channel::Sender;
use reqwest::StatusCode;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::custom_prompt::CustomPrompt;
use shinkai_sqlite::SqliteManager;
use tokio::sync::RwLock;

use crate::network::{node_error::NodeError, Node};

impl Node {
    pub async fn v2_api_add_custom_prompt(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        prompt: CustomPrompt,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Save the new prompt to the LanceShinkaiDb
        match sqlite_manager.write().await.add_prompt(&prompt).await {
            Ok(_) => {
                let _ = res.send(Ok(prompt)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add custom prompt to SqliteManager: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_delete_custom_prompt(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        prompt_name: String,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the prompt before deleting
        let prompt = sqlite_manager.read().await.get_prompts(Some(&prompt_name), None, None);

        // Check for errors or multiple prompts
        match prompt {
            Ok(prompts) if prompts.len() == 1 => {
                let prompt = prompts.into_iter().next().unwrap();
                // Delete the prompt from the LanceShinkaiDb
                match sqlite_manager.write().await.remove_prompt(&prompt_name) {
                    Ok(_) => {
                        let _ = res.send(Ok(prompt)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to delete custom prompt from SqliteManager: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Ok(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Multiple prompts found with the same name".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve custom prompt: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_all_custom_prompts(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        res: Sender<Result<Vec<CustomPrompt>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get all prompts from the LanceShinkaiDb
        match sqlite_manager.read().await.get_all_prompts() {
            Ok(prompts) => {
                let _ = res.send(Ok(prompts)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get all custom prompts from SqliteManager: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_custom_prompt(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        prompt_name: String,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the prompt from the LanceShinkaiDb with optional filters
        match sqlite_manager.read().await.get_prompts(Some(&prompt_name), None, None) {
            Ok(prompts) if prompts.len() == 1 => {
                let prompt = prompts.into_iter().next().unwrap();
                let _ = res.send(Ok(prompt)).await;
                Ok(())
            }
            Ok(prompts) if prompts.is_empty() => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Custom prompt not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Ok(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Multiple prompts found with the same name".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get custom prompt from SqliteManager: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_search_custom_prompts(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        query: String,
        res: Sender<Result<Vec<CustomPrompt>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Start the timer
        let start_time = Instant::now();

        // Perform the internal search using SqliteManager
        match sqlite_manager.read().await.prompt_vector_search(&query, 20).await {
            Ok(prompts) => {
                // Log the elapsed time if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    let elapsed_time = start_time.elapsed();
                    println!("Time taken for custom prompt search: {:?}", elapsed_time);
                    println!("Number of custom prompt results: {}", prompts.len());
                }
                let _ = res.send(Ok(prompts)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to search custom prompts: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_update_custom_prompt(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        bearer: String,
        prompt: CustomPrompt,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Update the prompt in the LanceShinkaiDb
        match sqlite_manager.write().await.update_prompt(&prompt).await {
            Ok(_) => {
                let _ = res.send(Ok(prompt)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update custom prompt in LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }
}
