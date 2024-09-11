use std::sync::Arc;

use async_channel::Sender;
use reqwest::StatusCode;

use tokio::sync::Mutex;

use crate::{
    db::ShinkaiDB,
    lance_db::shinkai_lance_db::LanceShinkaiDb,
    network::{
        agent_payments_manager::shinkai_tool_offering::ShinkaiToolOffering, node_api_router::APIError,
        node_error::NodeError, Node,
    },
    tools::shinkai_tool::ShinkaiToolHeader,
};

impl Node {
    pub async fn v2_api_get_tool_offering(
        db: Arc<ShinkaiDB>,
        bearer: String,
        tool_key_name: String,
        res: Sender<Result<ShinkaiToolOffering, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Fetch the tool offering
        match db.get_tool_offering(&tool_key_name) {
            Ok(tool_offering) => {
                let _ = res.send(Ok(tool_offering)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve tool offering: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_remove_tool_offering(
        db: Arc<ShinkaiDB>,
        bearer: String,
        tool_key_name: String,
        res: Sender<Result<ShinkaiToolOffering, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Attempt to get the tool offering before removing it
        let tool_offering = match db.get_tool_offering(&tool_key_name) {
            Ok(tool_offering) => tool_offering,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Tool offering not found: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Remove the tool offering
        match db.remove_tool_offering(&tool_key_name) {
            Ok(_) => {
                let _ = res.send(Ok(tool_offering)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove tool offering: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_get_all_tool_offering(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        res: Sender<Result<Vec<ShinkaiToolHeader>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Fetch all tool offerings
        let tool_offerings = match db.get_all_tool_offerings() {
            Ok(tool_offerings) => tool_offerings,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve all tool offerings: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Fetch tools from lance_db using the tool keys
        let mut detailed_tool_headers = Vec::new();
        for tool_offering in tool_offerings {
            let tool_key = &tool_offering.tool_key;
            match lance_db.lock().await.get_tool(tool_key).await {
                Ok(Some(tool)) => {
                    let mut tool_header = tool.to_header();
                    tool_header.sanitize_config();
                    tool_header.tool_offering = Some(tool_offering.clone());
                    detailed_tool_headers.push(tool_header);
                }
                Ok(None) => {
                    let api_error = APIError {
                        code: StatusCode::NOT_FOUND.as_u16(),
                        error: "Not Found".to_string(),
                        message: format!("Tool not found for key {}", tool_key),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to retrieve tool details for key {}: {}", tool_key, err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        }

        let _ = res.send(Ok(detailed_tool_headers)).await;

        Ok(())
    }

    pub async fn v2_api_set_tool_offering(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
        bearer: String,
        tool_offering: ShinkaiToolOffering,
        res: Sender<Result<ShinkaiToolOffering, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool from the database
        match lance_db.lock().await.tool_exists(&tool_offering.tool_key).await {
            Ok(exists) => {
                if !exists {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Tool does not exist".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        // Save the tool offering
        match db.set_tool_offering(tool_offering.clone()) {
            Ok(_) => {
                let _ = res.send(Ok(tool_offering)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to set tool offering: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
