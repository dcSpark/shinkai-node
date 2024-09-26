use std::{sync::Arc, time::Instant};

use async_channel::Sender;
use reqwest::StatusCode;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;
use shinkai_message_primitives::schemas::custom_prompt::CustomPrompt;
use tokio::sync::RwLock;

use crate::network::{node_error::NodeError, Node};

impl Node {
    pub async fn v2_api_add_custom_prompt(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
        bearer: String,
        prompt: CustomPrompt,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Save the new prompt to the LanceShinkaiDb
        match lance_db.write().await.set_prompt(prompt.clone()).await {
            Ok(_) => {
                let _ = res.send(Ok(prompt)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add custom prompt to LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_delete_custom_prompt(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
        bearer: String,
        prompt_name: String,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the prompt before deleting
        let prompt = lance_db.read().await.get_prompt(&prompt_name).await;

        // Delete the prompt from the LanceShinkaiDb
        match lance_db.write().await.remove_prompt(&prompt_name).await {
            Ok(_) => {
                match prompt {
                    Ok(Some(prompt)) => {
                        let _ = res.send(Ok(prompt)).await;
                    }
                    _ => {
                        let api_error = APIError {
                            code: StatusCode::NOT_FOUND.as_u16(),
                            error: "Not Found".to_string(),
                            message: "Prompt not found".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to delete custom prompt from LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_all_custom_prompts(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
        bearer: String,
        res: Sender<Result<Vec<CustomPrompt>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get all prompts from the LanceShinkaiDb
        match lance_db.read().await.get_all_prompts().await {
            Ok(prompts) => {
                // Set embeddings to None before returning
                let prompts_without_embeddings: Vec<CustomPrompt> = prompts
                    .into_iter()
                    .map(|mut prompt| {
                        prompt.embedding = None;
                        prompt
                    })
                    .collect();

                let _ = res.send(Ok(prompts_without_embeddings)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get all custom prompts from LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_custom_prompt(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
        bearer: String,
        prompt_name: String,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the prompt from the LanceShinkaiDb
        match lance_db.read().await.get_prompt(&prompt_name).await {
            Ok(Some(prompt)) => {
                let _ = res.send(Ok(prompt)).await;
                Ok(())
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Custom prompt not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get custom prompt from LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_search_custom_prompts(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
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

        // Perform the internal search using LanceShinkaiDb
        match lance_db.read().await.prompt_vector_search(&query, 5).await {
            Ok(prompts) => {
                // Set embeddings to None before returning
                let prompts_without_embeddings: Vec<CustomPrompt> = prompts
                    .into_iter()
                    .map(|mut prompt| {
                        prompt.embedding = None;
                        prompt
                    })
                    .collect();

                // Log the elapsed time if LOG_ALL is set to 1
                if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                    let elapsed_time = start_time.elapsed();
                    println!("Time taken for custom prompt search: {:?}", elapsed_time);
                    println!("Number of custom prompt results: {}", prompts_without_embeddings.len());
                }
                let _ = res.send(Ok(prompts_without_embeddings)).await;
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
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
        bearer: String,
        prompt: CustomPrompt,
        res: Sender<Result<CustomPrompt, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Update the prompt in the LanceShinkaiDb
        match lance_db.write().await.set_prompt(prompt.clone()).await {
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
