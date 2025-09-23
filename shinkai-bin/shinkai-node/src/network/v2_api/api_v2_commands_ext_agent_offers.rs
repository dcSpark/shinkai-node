use std::sync::Arc;
use tokio::sync::Mutex;

use async_channel::Sender;
use reqwest::StatusCode;

use chrono::Utc;
use serde_json::{json, Value};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::{
    shinkai_name::ShinkaiName, shinkai_tool_offering::ShinkaiToolOffering, tool_router_key::ToolRouterKey,
};
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use shinkai_tools_primitives::tools::{
    network_tool::NetworkTool,
    shinkai_tool::{ShinkaiTool, ShinkaiToolHeader},
};

use crate::network::{
    agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager, node_error::NodeError, Node,
};

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
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Try to retrieve the tool offering; it's fine if it doesn't exist
        let tool_offering = match db.get_tool_offering(&tool_key_name) {
            Ok(offering) => Some(offering),
            Err(SqliteManagerError::ToolOfferingNotFound(_)) => None,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve tool offering: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Remove the tool offering (no-op if it doesn't exist)
        match db.remove_tool_offering(&tool_key_name) {
            Ok(_) => {
                let response = match tool_offering {
                    Some(off) => json!({ "removed": off }),
                    None => json!({}),
                };
                let _ = res.send(Ok(response)).await;
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

    pub async fn v2_api_get_tool_with_offering(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        bearer: String,
        tool_key_name: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let tool_offering = match db.get_tool_offering(&tool_key_name) {
            Ok(offering) => offering,
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

        let tool = match db.get_tool_by_key(&tool_key_name) {
            Ok(tool) => tool,
            Err(SqliteManagerError::ToolNotFound(_)) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Tool not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
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
        };

        let mut header = tool.to_header();
        header.sanitize_config();

        // Use the existing tool key from the offering
        let tool_router_key_result =
            ToolRouterKey::to_network_router_key(&tool_offering.tool_key, &node_name.to_string());

        let tool_router_key_str = match tool_router_key_result {
            Ok(key) => key,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create network router key: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let network_tool = NetworkTool {
            name: header.name,
            description: header.description,
            version: header.version,
            author: header.author,
            mcp_enabled: header.mcp_enabled,
            provider: node_name,
            tool_router_key: tool_router_key_str,
            usage_type: tool_offering.usage_type.clone(),
            activated: header.enabled,
            config: header.config.unwrap_or_default(),
            input_args: header.input_args,
            output_arg: header.output_arg,
            embedding: None,
            restrictions: None,
        };

        let response = json!({
            "network_tool": network_tool,
            "tool_offering": tool_offering
        });
        let _ = res.send(Ok(response)).await;

        Ok(())
    }

    pub async fn v2_api_get_tools_with_offerings(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let tool_offerings = match db.get_all_tool_offerings() {
            Ok(offerings) => offerings,
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

        let mut results = Vec::new();
        for offering in tool_offerings {
            let tool = match db.get_tool_by_key(&offering.tool_key) {
                Ok(tool) => tool,
                Err(SqliteManagerError::ToolNotFound(_)) => {
                    let api_error = APIError {
                        code: StatusCode::NOT_FOUND.as_u16(),
                        error: "Not Found".to_string(),
                        message: format!("Tool not found for key {}", offering.tool_key),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
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
            };

            let mut header = tool.to_header();
            header.sanitize_config();

            // Use the existing tool key from the offering
            let tool_router_key_result =
                ToolRouterKey::to_network_router_key(&offering.tool_key, &node_name.to_string());

            let tool_router_key_str = match tool_router_key_result {
                Ok(key) => key,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to create network router key: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

            let network_tool = NetworkTool {
                name: header.name,
                description: header.description,
                version: header.version,
                author: header.author,
                mcp_enabled: header.mcp_enabled,
                provider: node_name.clone(),
                tool_router_key: tool_router_key_str,
                usage_type: offering.usage_type.clone(),
                activated: header.enabled,
                config: header.config.unwrap_or_default(),
                input_args: header.input_args,
                output_arg: header.output_arg,
                embedding: None,
                restrictions: None,
            };

            results.push(json!({
                "network_tool": network_tool,
                "tool_offering": offering
            }));
        }

        let _ = res.send(Ok(json!(results))).await;

        Ok(())
    }

    pub async fn v2_api_get_agent_network_offering(
        db: Arc<SqliteManager>,
        my_agent_offerings_manager: Arc<Mutex<MyAgentOfferingsManager>>,
        bearer: String,
        node_name: String,
        auto_check: bool,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let node_name = match ShinkaiName::new(node_name) {
            Ok(name) => name,
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Invalid identity".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        {
            let manager = my_agent_offerings_manager.lock().await;
            let _ = manager.request_agent_network_offering(node_name.clone()).await;
        }

        let start = std::time::Instant::now();
        loop {
            {
                let manager = my_agent_offerings_manager.lock().await;
                if let Some((offerings, ts)) = manager.get_agent_network_offering(&node_name.to_string()) {
                    let value = json!({"offerings": offerings, "last_updated": ts.to_rfc3339()});
                    let _ = res.send(Ok(value)).await;
                    break;
                }
            }

            if !auto_check || start.elapsed() >= std::time::Duration::from_secs(5) {
                let _ = res.send(Ok(json!({"offerings": null, "last_updated": null}))).await;
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        Ok(())
    }
}
