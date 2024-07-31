use super::{node_api::APIError, node_error::NodeError, Node};
use crate::managers::sheet_manager::SheetManager;
use crate::managers::IdentityManager;

use async_channel::Sender;

use reqwest::StatusCode;
use serde_json::{json, Value as JsonValue};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{APIRemoveColumnPayload, APISetColumnPayload, MessageSchemaType},
    },
};

use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn api_set_column(
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let (payload, requester_name) = match Self::validate_and_extract_payload::<APISetColumnPayload>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::SetColumn,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let sheet_manager = match sheet_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "SheetManager is not available".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Perform the logic to set the column using SheetManager
        match sheet_manager_guard.set_column(&payload.sheet_id, payload.column).await {
            Ok(_) => {
                let _ = res.send(Ok(json!({"status": "success"}))).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to set column: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_remove_column(
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let (payload, requester_name) = match Self::validate_and_extract_payload::<APIRemoveColumnPayload>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::RemoveColumn,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let sheet_manager = match sheet_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "SheetManager is not available".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Perform the logic to remove the column using SheetManager
        match sheet_manager_guard
            .remove_column(&payload.sheet_id, payload.column_id)
            .await
        {
            Ok(_) => {
                let _ = res.send(Ok(json!({"status": "success"}))).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove column: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_user_sheets(
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let requester_name = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::UserSheets,
        )
        .await
        {
            Ok((_, requester_name)) => requester_name,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let sheet_manager = match sheet_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "SheetManager is not available".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Lock the sheet_manager before using it
        let sheet_manager_guard = sheet_manager.lock().await;

        // Get user sheets using SheetManager
        match sheet_manager_guard.get_user_sheets().await {
            Ok(sheets) => {
                let response = json!(sheets);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get user sheets: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_create_empty_sheet(
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let requester_name = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::CreateEmptySheet,
        )
        .await
        {
            Ok((_, requester_name)) => requester_name,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let sheet_manager = match sheet_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "SheetManager is not available".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Create an empty sheet using SheetManager
        match sheet_manager_guard.create_empty_sheet() {
            Ok(sheet_id) => {
                let response = json!({"status": "success", "sheet_id": sheet_id});
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create empty sheet: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_remove_sheet(
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let (sheet_id, requester_name) = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::RemoveSheet,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let sheet_manager = match sheet_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "SheetManager is not available".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Remove the sheet using SheetManager
        match sheet_manager_guard.remove_sheet(&sheet_id) {
            Ok(_) => {
                let _ = res.send(Ok(json!({"status": "success"}))).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove sheet: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }
}
