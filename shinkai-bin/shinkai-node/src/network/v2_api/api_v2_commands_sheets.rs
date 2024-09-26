use crate::llm_provider::execution::chains::sheet_ui_chain::sheet_rust_functions::SheetRustFunctions;
use crate::managers::sheet_manager::SheetManager;
use crate::network::node_error::NodeError;
use crate::network::Node;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value as JsonValue};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIExportSheetPayload, APIImportSheetPayload, SheetFileFormat, SpreadSheetPayload,
};

impl Node {
    pub async fn v2_api_import_sheet(
        db: Arc<ShinkaiDB>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        input_payload: APIImportSheetPayload,
        bearer: String,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let sheet_id = sheet_manager.lock().await.create_empty_sheet().unwrap();

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
                let sheet_result = SheetRustFunctions::import_sheet_from_xlsx(sheet_manager.clone(), xlsx_data).await;

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
        db: Arc<ShinkaiDB>,
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
}
