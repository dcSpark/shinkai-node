use crate::managers::sheet_manager::SheetManager;
use crate::managers::IdentityManager;
use crate::network::node_error::NodeError;
use crate::network::Node;

use async_channel::Sender;

use reqwest::StatusCode;
use serde_json::{json, Value as JsonValue};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIAddRowsPayload, APIRemoveColumnPayload, APIRemoveRowsPayload, APISetCellValuePayload,
            APISetColumnPayload, MessageSchemaType,
        },
    },
};

use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn api_set_column(
        sheet_manager: Arc<Mutex<SheetManager>>,
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

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;
        let column_result = sheet_manager_guard
            .from_api_column_to_new_column(&payload.sheet_id, payload.column)
            .await;

        // Handle the Result<ColumnDefinition, String>
        let column = match column_result {
            Ok(col) => col,
            Err(err_msg) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert column: {}", err_msg),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Perform the logic to set the column using SheetManager
        match sheet_manager_guard.set_column(&payload.sheet_id, column.clone()).await {
            Ok(_) => {
                let _ = res.send(Ok(json!(column))).await;
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
        sheet_manager: Arc<Mutex<SheetManager>>,
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

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Perform the logic to remove the column using SheetManager
        match sheet_manager_guard
            .remove_column(&payload.sheet_id, payload.column_id)
            .await
        {
            Ok(_) => {
                let _ = res.send(Ok(json!(null))).await;
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
        sheet_manager: Arc<Mutex<SheetManager>>,
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
        sheet_manager: Arc<Mutex<SheetManager>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let (sheet_name, requester_name) = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::CreateEmptySheet,
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

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Create an empty sheet using SheetManager
        match sheet_manager_guard.create_empty_sheet() {
            Ok(sheet_id) => {
                // Update the sheet name
                match sheet_manager_guard.update_sheet_name(&sheet_id, sheet_name).await {
                    Ok(_) => {
                        let response = json!({"sheet_id": sheet_id});
                        let _ = res.send(Ok(response)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to update sheet name: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
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
        sheet_manager: Arc<Mutex<SheetManager>>,
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

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Remove the sheet using SheetManager
        match sheet_manager_guard.remove_sheet(&sheet_id) {
            Ok(_) => {
                let _ = res.send(Ok(json!(null))).await;
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

    pub async fn api_set_cell_value(
        sheet_manager: Arc<Mutex<SheetManager>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let (payload, requester_name) = match Self::validate_and_extract_payload::<APISetCellValuePayload>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::SetCellValue,
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

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;
        let payload_clone = payload.clone();

        // Perform the logic to set the cell value using SheetManager
        match sheet_manager_guard
            .set_cell_value(&payload.sheet_id, payload.row, payload.col, payload.value)
            .await
        {
            Ok(_) => {
                let _ = res.send(Ok(json!(payload_clone))).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to set cell value: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_get_sheet(
        sheet_manager: Arc<Mutex<SheetManager>>,
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
            MessageSchemaType::GetSheet,
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

        // Lock the sheet_manager before using it
        let sheet_manager_guard = sheet_manager.lock().await;

        // Get the sheet using SheetManager
        match sheet_manager_guard.get_sheet(&sheet_id) {
            Ok(sheet) => {
                let response = json!(sheet);
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Failed to get sheet: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_remove_rows(
        sheet_manager: Arc<Mutex<SheetManager>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let (payload, requester_name) = match Self::validate_and_extract_payload::<APIRemoveRowsPayload>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::RemoveRows,
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

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Perform the logic to remove the rows using SheetManager
        match sheet_manager_guard
            .remove_rows(&payload.sheet_id, payload.row_indices)
            .await
        {
            Ok(_) => {
                let _ = res.send(Ok(json!(null))).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove rows: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_add_rows(
        sheet_manager: Arc<Mutex<SheetManager>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        let (payload, requester_name) = match Self::validate_and_extract_payload::<APIAddRowsPayload>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::AddRows,
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

        // Lock the sheet_manager before using it
        let mut sheet_manager_guard = sheet_manager.lock().await;

        // Perform the logic to add rows using SheetManager
        let mut row_ids = Vec::new();
        for _ in 0..payload.number_of_rows {
            match sheet_manager_guard
                .add_row(&payload.sheet_id, payload.starting_row)
                .await
            {
                Ok(row_id) => row_ids.push(row_id),
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to add row: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        }

        let response = json!({ "row_ids": row_ids });
        let _ = res.send(Ok(response)).await;
        Ok(())
    }
}
