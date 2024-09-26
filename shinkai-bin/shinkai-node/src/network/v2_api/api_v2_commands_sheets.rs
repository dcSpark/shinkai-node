use crate::managers::{sheet_manager::SheetManager, IdentityManager};
use crate::network::node_error::NodeError;
use crate::network::Node;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value as JsonValue};
use shinkai_message_primitives::schemas::identity::Identity;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APISetSheetUploadedFilesPayload;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::vector_resource::VRPath;
use std::sync::Arc;
use tokio::sync::Mutex;

use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;

impl Node {
    pub async fn v2_set_sheet_uploaded_files(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        input_payload: APISetSheetUploadedFilesPayload,
        bearer: String,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        for ((row, col), files) in input_payload.files.into_iter() {
            // Validate file paths
            for file in files.iter() {
                let vr_path = match VRPath::from_string(file) {
                    Ok(path) => path,
                    Err(e) => {
                        let api_error = APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to convert path to VRPath: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };

                match vector_fs.validate_path_points_to_entry(vr_path, &requester_name).await {
                    Ok(_) => {}
                    Err(e) => {
                        let api_error = APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to validate path points to entry: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };
            }

            // Add uploaded files to the sheet
            let mut sheet_manager_guard = sheet_manager.lock().await;
            match sheet_manager_guard.set_uploaded_files(&input_payload.sheet_id, row, col, files) {
                Ok(_) => {}
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to set uploaded files: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        }

        let _ = res.send(Ok(json!("Ok"))).await;
        Ok(())
    }
}
