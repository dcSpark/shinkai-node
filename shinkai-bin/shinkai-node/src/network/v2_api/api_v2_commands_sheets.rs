use crate::managers::sheet_manager::SheetManager;
use crate::network::node_error::NodeError;
use crate::network::Node;
use crate::{
    llm_provider::execution::chains::sheet_ui_chain::sheet_rust_functions::SheetRustFunctions,
    managers::IdentityManager,
};

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value as JsonValue};
use shinkai_message_primitives::schemas::identity::Identity;
use shinkai_sqlite::SqliteManager;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::vector_resource::VRPath;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIExportSheetPayload, APIImportSheetPayload, APISetSheetUploadedFilesPayload, SheetFileFormat, SpreadSheetPayload,
};

impl Node {
    pub async fn v2_api_import_sheet(
        db: Arc<RwLock<SqliteManager>>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        input_payload: APIImportSheetPayload,
        bearer: String,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let sheet_id = sheet_manager.lock().await.create_empty_sheet().await.unwrap();

        if let Some(sheet_name) = &input_payload.sheet_name {
            let mut sheet_manager = sheet_manager.lock().await;
            sheet_manager.update_sheet_name(&sheet_id, sheet_name.clone()).await?;
        }

        match input_payload.sheet_data {
            SpreadSheetPayload::CSV(csv_data) => {
                let mut args = HashMap::new();
                args.insert("csv_data".to_string(), Box::new(csv_data) as Box<dyn Any + Send>);

                let sheet_result =
                    SheetRustFunctions::create_new_columns_with_csv(sheet_manager.clone(), sheet_id.clone(), args)
                        .await;

                match sheet_result {
                    Ok(_) => {
                        let response = json!({ "sheet_id": sheet_id });
                        let _ = res.send(Ok(response)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to import sheet: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            SpreadSheetPayload::XLSX(xlsx_data) => {
                let sheet_result = SheetRustFunctions::import_sheet_from_xlsx(
                    sheet_manager.clone(),
                    xlsx_data,
                    input_payload.sheet_name,
                )
                .await;

                match sheet_result {
                    Ok(sheet_id) => {
                        let response = json!({ "sheet_id": sheet_id });
                        let _ = res.send(Ok(response)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to import sheet: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
        }
    }

    pub async fn v2_api_export_sheet(
        db: Arc<RwLock<SqliteManager>>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        input_payload: APIExportSheetPayload,
        bearer: String,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match input_payload.file_format {
            SheetFileFormat::CSV => {
                let csv_result =
                    SheetRustFunctions::export_sheet_to_csv(sheet_manager.clone(), input_payload.sheet_id.clone())
                        .await;

                match csv_result {
                    Ok(csv_data) => {
                        let response = json!(SpreadSheetPayload::CSV(csv_data));
                        let _ = res.send(Ok(response)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to export sheet: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            SheetFileFormat::XLSX => {
                let xlsx_result =
                    SheetRustFunctions::export_sheet_to_xlsx(sheet_manager.clone(), input_payload.sheet_id.clone())
                        .await;

                match xlsx_result {
                    Ok(xlsx_data) => {
                        let response = json!(SpreadSheetPayload::XLSX(xlsx_data));
                        let _ = res.send(Ok(response)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to export sheet: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
        }
    }

    pub async fn v2_set_sheet_uploaded_files(
        db: Arc<RwLock<SqliteManager>>,
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
            match sheet_manager_guard
                .set_uploaded_files(&input_payload.sheet_id, row, col, files)
                .await
            {
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
