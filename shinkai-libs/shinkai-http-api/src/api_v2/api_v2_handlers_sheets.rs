use async_channel::Sender;
use reqwest::StatusCode;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIExportSheetPayload, APIImportSheetPayload,
};

use crate::{node_api_router::APIError, node_commands::NodeCommand};
use utoipa::OpenApi;
use warp::Filter;

use super::api_v2_router::{create_success_response, with_sender};

pub fn sheets_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let import_sheet_route = warp::path("import_sheet")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(import_sheet_handler);

    let export_sheet_route = warp::path("export_sheet")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(export_sheet_handler);

    import_sheet_route.or(export_sheet_route)
}

#[utoipa::path(
  post,
  path = "/v2/import_sheet",
  request_body = APIImportSheetPayload,
  responses(
      (status = 200, description = "Successfully retrieved path", body = Value),
      (status = 400, description = "Bad request", body = APIError),
      (status = 500, description = "Internal server error", body = APIError)
  )
)]
pub async fn import_sheet_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIImportSheetPayload,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiImportSheet {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(response);
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
  post,
  path = "/v2/export_sheet",
  request_body = APIExportSheetPayload,
  responses(
      (status = 200, description = "Successfully retrieved path", body = Value),
      (status = 400, description = "Bad request", body = APIError),
      (status = 500, description = "Internal server error", body = APIError)
  )
)]
pub async fn export_sheet_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIExportSheetPayload,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiExportSheet {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(response);
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        import_sheet_handler,
        export_sheet_handler
    ),
    components(
        schemas(APIError, APIExportSheetPayload, APIImportSheetPayload)
    ),
    tags(
        (name = "sheets", description = "Sheets API endpoints")
    )
)]
pub struct SheetsApiDoc;
