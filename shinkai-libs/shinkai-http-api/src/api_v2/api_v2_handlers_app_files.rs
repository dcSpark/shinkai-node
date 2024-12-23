use async_channel::Sender;
use bytes::Buf;
use futures::TryStreamExt;
use reqwest::StatusCode;
use std::collections::HashMap;
use utoipa::{OpenApi, ToSchema};
use warp::multipart::FormData;
use warp::Filter;

use super::api_v2_router::{create_success_response, with_sender};
use crate::{node_api_router::APIError, node_commands::NodeCommand};

pub fn app_files_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let upload_file_route = warp::path("app_file")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::multipart::form())
        .and_then(upload_file_handler);

    let get_file_route = warp::path("app_file")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(get_file_handler);

    let update_file_route = warp::path("patch_app_file")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::multipart::form())
        .and_then(update_file_handler);

    let list_files_route = warp::path("list_app_files")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and_then(list_files_handler);

    let delete_file_route = warp::path("app_file")
        .and(warp::delete())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(delete_file_handler);

    upload_file_route
        .or(get_file_route)
        .or(update_file_route)
        .or(list_files_route)
        .or(delete_file_route)
}

#[utoipa::path(
    post,
    path = "/v2/app_file",
    responses(
        (status = 200, description = "Successfully uploaded file", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn upload_file_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let mut file_name = String::new();
    let mut file_data: Option<Vec<u8>> = Some(vec![]);

    while let Ok(Some(part)) = form.try_next().await {
        match part.name() {
            "file_name" => {
                // Convert the part to bytes then to string
                let mut bytes = Vec::new();
                let mut stream = part.stream();
                while let Ok(Some(chunk)) = stream.try_next().await {
                    bytes.extend_from_slice(chunk.chunk());
                }
                file_name = String::from_utf8_lossy(&bytes).into_owned();
            }
            "file" => {
                // Read file data
                let mut bytes = Vec::new();
                let mut stream = part.stream();
                while let Ok(Some(chunk)) = stream.try_next().await {
                    bytes.extend_from_slice(chunk.chunk());
                }
                file_data = Some(bytes);
            }
            _ => {}
        }
    }

    if file_name.is_empty() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&APIError {
                code: 400,
                error: "Missing file name".to_string(),
                message: "File name is required".to_string(),
            }),
            StatusCode::BAD_REQUEST,
        ));
    }

    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::V2ApiUploadAppFile {
            bearer,
            tool_id,
            app_id,
            file_name,
            file_data: file_data.unwrap(),
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

// #[derive(Deserialize, ToSchema)]
// pub struct UpdateFileRequest {
//     pub file_name: String,
//     pub new_name: String,
//     pub file_data: Vec<u8>,
// }

#[utoipa::path(
    post,
    path = "/v2/patch_app_file",
    request_body = UpdateFileRequest,
    responses(
        (status = 200, description = "Successfully updated file", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_file_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);

    let mut file_name: String = String::new();
    let mut new_name: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;

    while let Ok(Some(part)) = form.try_next().await {
        match part.name() {
            "file_name" => {
                // Convert the part to bytes then to string
                let mut bytes = Vec::new();
                let mut stream = part.stream();
                while let Ok(Some(chunk)) = stream.try_next().await {
                    bytes.extend_from_slice(chunk.chunk());
                }
                file_name = String::from_utf8_lossy(&bytes).into_owned();
            }
            "file" => {
                // Read file data
                let mut bytes = Vec::new();
                let mut stream = part.stream();
                while let Ok(Some(chunk)) = stream.try_next().await {
                    bytes.extend_from_slice(chunk.chunk());
                }
                file_data = Some(bytes);
            }
            "new_name" => {
                // Convert the part to bytes then to string
                let mut bytes = Vec::new();
                let mut stream = part.stream();
                while let Ok(Some(chunk)) = stream.try_next().await {
                    bytes.extend_from_slice(chunk.chunk());
                }
                new_name = Some(String::from_utf8_lossy(&bytes).into_owned());
            }
            _ => {}
        }
    }
    if file_name.is_empty() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&APIError {
                code: 400,
                error: "Missing file name".to_string(),
                message: "File name is required".to_string(),
            }),
            StatusCode::BAD_REQUEST,
        ));
    }

    sender
        .send(NodeCommand::V2ApiUpdateAppFile {
            bearer,
            tool_id,
            app_id,
            file_name,
            new_name,
            file_data,
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
    get,
    path = "/v2/app_file",
    params(
        ("file_name" = String, Query, description = "Name of the file to retrieve")
    ),
    responses(
        (status = 200, description = "Successfully retrieved file", body = Vec<u8>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_file_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let file_name = query_params
        .get("file_name")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Missing file name".to_string(),
                message: "File name is required".to_string(),
            })
        })?
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::V2ApiGetAppFile {
            bearer,
            tool_id,
            app_id,
            file_name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(file_bytes) => Ok(warp::reply::with_header(
            warp::reply::with_status(file_bytes, StatusCode::OK),
            "Content-Type",
            "application/octet-stream",
        )),
        Err(error) => Ok(warp::reply::with_header(
            warp::reply::with_status(
                error.message.as_bytes().to_vec(),
                StatusCode::from_u16(error.code).unwrap(),
            ),
            "Content-Type",
            "text/plain",
        )),
    }
}

#[utoipa::path(
    get,
    path = "/v2/list_app_files",
    responses(
        (status = 200, description = "Successfully listed files", body = Vec<String>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_files_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::V2ApiListAppFiles {
            bearer,
            tool_id,
            app_id,
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
    delete,
    path = "/v2/app_file",
    params(
        ("file_name" = String, Query, description = "Name of the file to delete")
    ),
    responses(
        (status = 200, description = "Successfully deleted file", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn delete_file_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let file_name = query_params
        .get("file_name")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Missing file name".to_string(),
                message: "File name is required".to_string(),
            })
        })?
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::V2ApiDeleteAppFile {
            bearer,
            tool_id,
            app_id,
            file_name,
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
        upload_file_handler,
        get_file_handler,
        update_file_handler,
        list_files_handler,
        delete_file_handler,
    ),
    components(
        schemas(
            APIError,
        )
    ),
    tags(
        (name = "files", description = "File API endpoints")
    )
)]
pub struct FilesApiDoc;
