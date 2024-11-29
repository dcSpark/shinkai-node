use std::sync::Arc;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;

use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_tool_offering::UsageTypeInquiry;
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use tokio::sync::{Mutex, RwLock};

use crate::network::{
    agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager, node_error::NodeError, Node,
};

impl Node {
    pub async fn v2_api_request_invoice(
        db: Arc<RwLock<SqliteManager>>,
        my_agent_payments_manager: Arc<Mutex<MyAgentOfferingsManager>>,
        bearer: String,
        tool_key_name: String,
        usage: UsageTypeInquiry,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Fetch the tool from lance_db
        let network_tool = {
            match db.read().await.get_tool_by_key(&tool_key_name) {
                Ok(tool) => match tool {
                    ShinkaiTool::Network(network_tool, _) => network_tool,
                    _ => {
                        let api_error = APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: "Tool is not a NetworkTool".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                },
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
            }
        };

        // Lock the payments manager
        let manager = my_agent_payments_manager.lock().await;

        // Request the invoice
        match manager.network_request_invoice(network_tool, usage).await {
            Ok(invoice_request) => {
                let invoice_value = match serde_json::to_value(invoice_request) {
                    Ok(value) => value,
                    Err(e) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize invoice request: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };
                let _ = res.send(Ok(invoice_value)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to request invoice: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_pay_invoice(
        db: Arc<RwLock<SqliteManager>>,
        my_agent_offerings_manager: Arc<Mutex<MyAgentOfferingsManager>>,
        bearer: String,
        invoice_id: String,
        data_for_tool: Value,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Step 1: Get the invoice from the database
        let invoice = match db.read().await.get_invoice(&invoice_id) {
            Ok(invoice) => invoice,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get invoice: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Step 2: Verify the invoice
        let is_valid = match my_agent_offerings_manager.lock().await.verify_invoice(&invoice).await {
            Ok(valid) => valid,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to verify invoice: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        if !is_valid {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invoice is not valid".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Step 3: Check that the invoice is not expired
        if invoice.expiration_time < chrono::Utc::now() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invoice has expired".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Step 4: Check that the data_for_tool is valid
        let tool_key_name = invoice.shinkai_offering.tool_key.clone();
        let tool = {
            match db.read().await.get_tool_by_key(&tool_key_name) {
                Ok(tool) => tool,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Check if the tool has the required input_args
        let required_args = match tool {
            ShinkaiTool::Network(network_tool, _) => network_tool.input_args,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Tool is not a NetworkTool".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validate that data_for_tool contains all the required input_args
        for arg in required_args.iter().filter(|arg| arg.is_required) {
            if !data_for_tool.get(&arg.name).is_some() {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Missing required argument: {}", arg.name),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        // Step 5: Pay the invoice
        let payment = match my_agent_offerings_manager
            .lock()
            .await
            .pay_invoice_and_send_receipt(invoice_id, data_for_tool)
            .await
        {
            Ok(payment) => payment,
            Err(e) => {
                // Use regex to extract a more human-readable error message
                let error_message = e.to_string();
                let human_readable_message = if let Ok(regex) = regex::Regex::new(r#"message: \\"(.*?)\\""#) {
                    if let Some(captures) = regex.captures(&error_message) {
                        captures
                            .get(1)
                            .map_or(error_message.clone(), |m| m.as_str().to_string())
                    } else {
                        error_message.clone()
                    }
                } else {
                    error_message.clone()
                };

                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to pay invoice: {}", human_readable_message),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Send success response with payment details
        let payment_value = match serde_json::to_value(payment) {
            Ok(value) => value,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to serialize payment: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let _ = res.send(Ok(payment_value)).await;
        Ok(())
    }

    pub async fn v2_api_list_invoices(
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Fetch the list of invoices from the database
        match db.read().await.get_all_invoices() {
            Ok(invoices) => {
                let invoices_value = match serde_json::to_value(invoices) {
                    Ok(value) => value,
                    Err(e) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize invoices: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };
                let _ = res.send(Ok(invoices_value)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to fetch invoices: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
