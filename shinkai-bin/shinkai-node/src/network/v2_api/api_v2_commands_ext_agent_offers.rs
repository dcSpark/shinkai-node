use std::sync::Arc;

use async_channel::Sender;
use reqwest::StatusCode;

use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_tool_offering::ShinkaiToolOffering;
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiToolHeader;
use tokio::sync::RwLock;
use serde::Deserialize; // Add for API request structs
use serde_json::Value; // For API responses

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_libs::shinkai_non_rust_code::functions::x402; // For x402::types::PaymentRequirementsRequest

use crate::network::{node_error::NodeError, Node};


// --- Structs for API Request Bodies ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402PaymentRequirementsApiRequest {
    pub tool_key_name: String,
    pub tool_data: Option<String>,
    // pub requester_node_name: Option<String>, // If needed to be specified by the caller
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X402ConfirmPaymentApiRequest {
    pub payment_jwt: String,
    pub tool_key_name: String,
    // pub requester_node_name: Option<String>, // If needed
}


impl Node {
    pub async fn v2_api_get_tool_offering(
        db: Arc<SqliteManager>,
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
        db: Arc<SqliteManager>,
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
        db: Arc<SqliteManager>,
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
            match db.get_tool_by_key(tool_key) {
                Ok(tool) => {
                    let mut tool_header = tool.to_header();
                    tool_header.sanitize_config();
                    tool_header.tool_offering = Some(tool_offering.clone());
                    detailed_tool_headers.push(tool_header);
                }
                Err(SqliteManagerError::ToolNotFound(_)) => {
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
        db: Arc<SqliteManager>,
        bearer: String,
        tool_offering: ShinkaiToolOffering,
        res: Sender<Result<ShinkaiToolOffering, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the tool from the database
        match db.tool_exists(&tool_offering.tool_key, None) {
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

    pub async fn v2_api_request_x402_payment_requirements(
        node: Arc<RwLock<Node>>,
        bearer: String,
        request_body: X402PaymentRequirementsApiRequest,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate bearer token and get associated identity (requester_node_name)
        let db = {
            let r_node = node.read().await;
            r_node.db.clone()
        };
        let requester_identity = match Self::validate_bearer_token_and_get_identity(&bearer, db.clone(), &res).await {
            Ok(identity) => identity,
            Err(_) => return Ok(()), // Error already sent by validate_bearer_token
        };
        // Assuming StandardIdentity has a way to get ShinkaiName, or just use the string form if appropriate
        let requester_node_name = requester_identity.get_full_shinkai_name();


        let tool_key_shinkai_name = match ShinkaiName::new(&request_body.tool_key_name) {
            Ok(name) => name,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "InvalidToolKeyName".to_string(),
                    message: format!("Invalid tool_key_name format: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let x402_req = x402::types::PaymentRequirementsRequest {
            tool_key_name: tool_key_shinkai_name.to_string(), // Assuming x402 type expects String
            tool_data: request_body.tool_data,
        };

        let mut w_node = node.write().await;
        let offerings_manager = Arc::clone(&w_node.ext_agent_offerings_manager);
        drop(w_node); // Release lock on node

        let mut manager_lock = offerings_manager.lock().await;
        
        match manager_lock.network_request_payment_requirements(requester_node_name.clone(), x402_req).await {
            Ok(_) => {
                // The manager method sends the response/error over the network.
                // API returns an immediate acknowledgment.
                let _ = res.send(Ok(serde_json::json!({"status": "processing", "message": "Payment requirements request is being processed."}))).await;
            }
            Err(e) => {
                // This error is if the local processing/setup for network_request_payment_requirements failed
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "ProcessingError".to_string(),
                    message: format!("Failed to initiate payment requirements request: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_confirm_x402_payment(
        node: Arc<RwLock<Node>>,
        bearer: String,
        request_body: X402ConfirmPaymentApiRequest,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let db = {
            let r_node = node.read().await;
            r_node.db.clone()
        };
        let requester_identity = match Self::validate_bearer_token_and_get_identity(&bearer, db.clone(), &res).await {
            Ok(identity) => identity,
            Err(_) => return Ok(()), 
        };
        let requester_node_name = requester_identity.get_full_shinkai_name();

        let tool_key_shinkai_name = match ShinkaiName::new(&request_body.tool_key_name) {
            Ok(name) => name,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "InvalidToolKeyName".to_string(),
                    message: format!("Invalid tool_key_name format: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut w_node = node.write().await;
        let offerings_manager = Arc::clone(&w_node.ext_agent_offerings_manager);
        drop(w_node);

        let mut manager_lock = offerings_manager.lock().await;

        match manager_lock.network_process_payment_confirmation(
            requester_node_name.clone(), 
            request_body.payment_jwt, 
            tool_key_shinkai_name
        ).await {
            Ok(_) => {
                let _ = res.send(Ok(serde_json::json!({"status": "processing", "message": "Payment confirmation is being processed."}))).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "ProcessingError".to_string(),
                    message: format!("Failed to initiate payment confirmation: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }
}
