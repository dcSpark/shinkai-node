use std::sync::Arc;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{
    db::ShinkaiDB,
    lance_db::shinkai_lance_db::LanceShinkaiDb,
    network::{
        agent_payments_manager::{
            my_agent_offerings_manager::MyAgentOfferingsManager, shinkai_tool_offering::UsageTypeInquiry,
        },
        node_api_router::APIError,
        node_error::NodeError,
        Node,
    },
    tools::shinkai_tool::ShinkaiTool,
};

impl Node {
    pub async fn v2_api_request_invoice(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
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
            let lance_db_lock = lance_db.lock().await;
            match lance_db_lock.get_tool(&tool_key_name).await {
                Ok(Some(tool)) => match tool {
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
                Ok(None) => {
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
        match manager.request_invoice(network_tool, usage).await {
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
}
