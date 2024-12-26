use async_channel::Sender;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder,
    APIVecFsDeleteItem, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
    APIVecFsRetrieveSourceFile, APIVecFsSearchItems,
};

use crate::api_v2::api_v2_handlers_jobs::AddFileToJob;
use crate::node_commands::NodeCommand;
use crate::{api_v2::api_v2_handlers_jobs::AddFileToFolder, node_api_router::APIError};
use bytes::Buf;
use futures::StreamExt;
use utoipa::OpenApi;
use warp::multipart::FormData;
use warp::Filter;

use super::api_v2_router::{create_success_response, with_sender};

pub fn vecfs_routes(
    node_commands_sender: Sender<NodeCommand>,
    _node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let retrieve_path_simplified_route = warp::path("retrieve_path_simplified")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<APIVecFsRetrievePathSimplifiedJson>())
        .and_then(retrieve_path_simplified_handler);

    let retrieve_vector_resource_route = warp::path("retrieve_vector_resource")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<String>())
        .and_then(retrieve_vector_resource_handler);

    let create_folder_route = warp::path("create_folder")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(create_folder_handler);

    let move_item_route = warp::path("move_item")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move_item_handler);

    let copy_item_route = warp::path("copy_item")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(copy_item_handler);

    let move_folder_route = warp::path("move_folder")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move_folder_handler);

    let copy_folder_route = warp::path("copy_folder")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(copy_folder_handler);

    let delete_folder_route = warp::path("delete_folder")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(delete_folder_handler);

    let delete_item_route = warp::path("delete_item")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(delete_item_handler);

    let search_items_route = warp::path("search_items")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(search_items_handler);

    let upload_file_to_folder_route = warp::path("upload_file_to_folder")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::multipart::form())
        .and_then(upload_file_to_folder_handler);

    let retrieve_source_file_route = warp::path("download_file")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<APIVecFsRetrieveSourceFile>())
        .and_then(retrieve_source_file_handler);

    let retrieve_files_for_job_route = warp::path("retrieve_files_for_job")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<String>())
        .and_then(retrieve_files_for_job_handler);

    let get_folder_name_for_job_route = warp::path("get_folder_name_for_job")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<String>())
        .and_then(get_folder_name_for_job_handler);

    let upload_file_to_job_route = warp::path("upload_file_to_job")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::multipart::form())
        .and_then(upload_file_to_job_handler);

    move_item_route
        .or(copy_item_route)
        .or(move_folder_route)
        .or(copy_folder_route)
        .or(delete_folder_route)
        .or(delete_item_route)
        .or(search_items_route)
        .or(retrieve_path_simplified_route)
        .or(retrieve_vector_resource_route)
        .or(create_folder_route)
        .or(upload_file_to_folder_route)
        .or(retrieve_source_file_route)
        .or(retrieve_files_for_job_route)
        .or(get_folder_name_for_job_route)
        .or(upload_file_to_job_route)
}

#[utoipa::path(
    post,
    path = "/v2/retrieve_path_simplified",
    request_body = APIVecFsRetrievePathSimplifiedJson,
    responses(
        (status = 200, description = "Successfully retrieved path", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn retrieve_path_simplified_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsRetrievePathSimplifiedJson,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiVecFSRetrievePathSimplifiedJson {
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
    get,
    path = "/v2/retrieve_vector_resource",
    responses(
        (status = 200, description = "Successfully retrieved vector resource", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn retrieve_vector_resource_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    path: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiVecFSRetrieveVectorResource {
            bearer,
            path,
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
    path = "/v2/create_folder",
    request_body = APIVecFsCreateFolder,
    responses(
        (status = 200, description = "Successfully created folder", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn create_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsCreateFolder,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiVecFSCreateFolder {
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
    path = "/v2/move_item",
    request_body = APIVecFsMoveItem,
    responses(
        (status = 200, description = "Successfully moved item", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn move_item_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsMoveItem,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiMoveItem {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/copy_item",
    request_body = APIVecFsCopyItem,
    responses(
        (status = 200, description = "Successfully copied item", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn copy_item_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsCopyItem,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiCopyItem {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/move_folder",
    request_body = APIVecFsMoveFolder,
    responses(
        (status = 200, description = "Successfully moved folder", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn move_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsMoveFolder,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiMoveFolder {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/copy_folder",
    request_body = APIVecFsCopyFolder,
    responses(
        (status = 200, description = "Successfully copied folder", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn copy_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsCopyFolder,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiCopyFolder {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/delete_folder",
    request_body = APIVecFsDeleteFolder,
    responses(
        (status = 200, description = "Successfully deleted folder", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn delete_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsDeleteFolder,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiDeleteFolder {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/delete_item",
    request_body = APIVecFsDeleteItem,
    responses(
        (status = 200, description = "Successfully deleted item", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn delete_item_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsDeleteItem,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiDeleteItem {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/search_items",
    request_body = APIVecFsSearchItems,
    responses(
        (status = 200, description = "Successfully searched items", body = Vec<String>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn search_items_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsSearchItems,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiSearchItems {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/upload_file_to_folder",
    request_body = AddFileToInboxRequest,
    responses(
        (status = 200, description = "Successfully uploaded file to folder", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn upload_file_to_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let mut filename = String::new();
    let mut file_data = Vec::new();
    let mut path = String::new();
    let mut file_datetime: Option<DateTime<Utc>> = None;

    while let Some(part) = form.next().await {
        let mut part = part.map_err(|e| {
            eprintln!("Error collecting form data: {:?}", e);
            warp::reject::custom(APIError::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                format!("Failed to collect form data: {:?}", e).as_str(),
            ))
        })?;
        match part.name() {
            "filename" => {
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing filename",
                    ))
                })?;
                let mut content = content.map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Failed to read filename",
                    ))
                })?;
                filename = String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Invalid UTF-8 in filename",
                    ))
                })?;
            }
            "file_data" => {
                while let Some(content) = part.data().await {
                    let mut content = content.map_err(|_| {
                        warp::reject::custom(APIError::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            "Failed to read file data",
                        ))
                    })?;
                    file_data.extend_from_slice(&content.copy_to_bytes(content.remaining()));
                }
            }
            "path" => {
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(StatusCode::BAD_REQUEST, "Bad Request", "Missing path"))
                })?;
                let mut content = content.map_err(|e| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        format!("Failed to read path: {:?}", e).as_str(),
                    ))
                })?;
                path = String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Invalid UTF-8 in path",
                    ))
                })?;
            }
            "file_datetime" => {
                if let Some(content) = part.data().await {
                    let mut content = content.map_err(|e| {
                        warp::reject::custom(APIError::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            format!("Failed to read file_datetime: {:?}", e).as_str(),
                        ))
                    })?;
                    let datetime_str =
                        String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                            warp::reject::custom(APIError::new(
                                StatusCode::BAD_REQUEST,
                                "Bad Request",
                                "Invalid UTF-8 in file_datetime",
                            ))
                        })?;
                    file_datetime = Some(
                        DateTime::parse_from_rfc3339(&datetime_str)
                            .map_err(|_| {
                                warp::reject::custom(APIError::new(
                                    StatusCode::BAD_REQUEST,
                                    "Bad Request",
                                    "Invalid datetime format",
                                ))
                            })?
                            .with_timezone(&Utc),
                    );
                }
            }
            _ => {}
        }
    }

    if file_data.is_empty() {
        return Err(warp::reject::custom(APIError::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "No file data found. Check that the file is being uploaded correctly in the `file_data` field",
        )));
    }

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiUploadFileToFolder {
            bearer,
            filename,
            file: file_data,
            path,
            file_datetime,
            res: res_sender,
        })
        .await
        .map_err(|_| {
            warp::reject::custom(APIError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Failed to send command",
            ))
        })?;
    let result = res_receiver.recv().await.map_err(|_| {
        warp::reject::custom(APIError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "Failed to receive response",
        ))
    })?;

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
    path = "/v2/download_file",
    request_body = APIVecFsSearchItems,
    responses(
        (status = 200, description = "Successfully searched items", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn retrieve_source_file_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIVecFsRetrieveSourceFile,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiRetrieveFile {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    get,
    path = "/v2/retrieve_files_for_job",
    responses(
        (status = 200, description = "Successfully retrieved files for job", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn retrieve_files_for_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    job_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiVecFSRetrieveFilesForJob {
            bearer,
            job_id,
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
    path = "/v2/get_folder_name_for_job",
    responses(
        (status = 200, description = "Successfully retrieved folder name for job", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_folder_name_for_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    job_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiVecFSGetFolderNameForJob {
            bearer,
            job_id,
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
    path = "/v2/upload_file_to_job",
    request_body = AddFileToJob,
    responses(
        (status = 200, description = "Successfully uploaded file to job", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn upload_file_to_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let mut job_id = String::new();
    let mut filename = String::new();
    let mut file_data = Vec::new();
    let mut file_datetime: Option<DateTime<Utc>> = None;

    while let Some(part) = form.next().await {
        let mut part = part.map_err(|e| {
            eprintln!("Error collecting form data: {:?}", e);
            warp::reject::custom(APIError::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                format!("Failed to collect form data: {:?}", e).as_str(),
            ))
        })?;
        match part.name() {
            "job_id" => {
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing job_id",
                    ))
                })?;
                let mut content = content.map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Failed to read job_id",
                    ))
                })?;
                job_id = String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Invalid UTF-8 in job_id",
                    ))
                })?;
            }
            "filename" => {
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing filename",
                    ))
                })?;
                let mut content = content.map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Failed to read filename",
                    ))
                })?;
                filename = String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Invalid UTF-8 in filename",
                    ))
                })?;
            }
            "file_data" => {
                while let Some(content) = part.data().await {
                    let mut content = content.map_err(|_| {
                        warp::reject::custom(APIError::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            "Failed to read file data",
                        ))
                    })?;
                    file_data.extend_from_slice(&content.copy_to_bytes(content.remaining()));
                }
            }
            "file_datetime" => {
                if let Some(content) = part.data().await {
                    let mut content = content.map_err(|e| {
                        warp::reject::custom(APIError::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            format!("Failed to read file_datetime: {:?}", e).as_str(),
                        ))
                    })?;
                    let datetime_str =
                        String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                            warp::reject::custom(APIError::new(
                                StatusCode::BAD_REQUEST,
                                "Bad Request",
                                "Invalid UTF-8 in file_datetime",
                            ))
                        })?;
                    file_datetime = Some(
                        DateTime::parse_from_rfc3339(&datetime_str)
                            .map_err(|_| {
                                warp::reject::custom(APIError::new(
                                    StatusCode::BAD_REQUEST,
                                    "Bad Request",
                                    "Invalid datetime format",
                                ))
                            })?
                            .with_timezone(&Utc),
                    );
                }
            }
            _ => {}
        }
    }

    if file_data.is_empty() {
        return Err(warp::reject::custom(APIError::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "No file data found. Check that the file is being uploaded correctly in the `file_data` field",
        )));
    }

    // Generate current UTC time if file_datetime is not provided
    let file_datetime = file_datetime.unwrap_or_else(Utc::now);

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiUploadFileToJob {
            bearer,
            job_id,
            filename,
            file: file_data,
            file_datetime: Some(file_datetime),
            res: res_sender,
        })
        .await
        .map_err(|_| {
            warp::reject::custom(APIError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Failed to send command",
            ))
        })?;
    let result = res_receiver.recv().await.map_err(|_| {
        warp::reject::custom(APIError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "Failed to receive response",
        ))
    })?;

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
        retrieve_path_simplified_handler,
        retrieve_vector_resource_handler,
        create_folder_handler,
        move_item_handler,
        copy_item_handler,
        move_folder_handler,
        copy_folder_handler,
        delete_folder_handler,
        delete_item_handler,
        search_items_handler,
        upload_file_to_folder_handler,
        retrieve_source_file_handler,
        retrieve_files_for_job_handler,
        get_folder_name_for_job_handler,
        upload_file_to_job_handler,
    ),
    components(
        schemas(APIError, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem,
            APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsSearchItems, AddFileToFolder, AddFileToJob)
    ),
    tags(
        (name = "vecfs", description = "VecFS API endpoints")
    )
)]
pub struct VecFsApiDoc;
