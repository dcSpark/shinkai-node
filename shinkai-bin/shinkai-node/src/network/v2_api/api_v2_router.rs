use crate::network::{node_commands::NodeCommand, v1_api::api_v1_handlers::APIUseRegistrationCodeSuccessResponse};

use super::super::node_api_router::{APIError, GetPublicKeysResponse, SendResponseBody, SendResponseBodyData};
use async_channel::Sender;
use bytes::Buf;
use chrono::DateTime;
use chrono::Utc;
use futures::StreamExt;
use futures::TryStreamExt;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIConvertFilesAndSaveToFolder, APIVecFsCreateFolder, APIVecFsRetrievePathSimplifiedJson, JobCreationInfo,
    JobMessage,
};
use utoipa::OpenApi;
use warp::multipart::{FormData, Part};
use warp::Filter;

pub fn v2_routes(
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let public_keys_route = warp::path("public_keys")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and_then(get_public_keys);

    let health_check_route = warp::path("health_check")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(with_node_name(node_name.clone()))
        .and_then(health_check);

    let create_job_route = warp::path("create_job")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(create_job_handler);

    let job_message_route = warp::path("job_message")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(job_message_handler);

    let initial_registration_route = warp::path("initial_registration")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::body::json())
        .and_then(initial_registration_handler);

    let get_last_messages_route = warp::path("last_messages")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(get_last_messages_handler);

    let get_all_smart_inboxes_route = warp::path("all_inboxes")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_all_smart_inboxes_handler);

    let available_llm_providers_route = warp::path("available_models")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_available_llm_providers_handler);

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

    let convert_files_and_save_route = warp::path("convert_files_and_save")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(convert_files_and_save_handler);

    let create_folder_route = warp::path("create_folder")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(create_folder_handler);

    let update_smart_inbox_name_route = warp::path("update_smart_inbox_name")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_smart_inbox_name_handler);

    let create_files_inbox_route = warp::path("create_files_inbox")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(create_files_inbox_handler);

    let add_file_to_inbox_route = warp::path("add_file_to_inbox")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::multipart::form())
        .and_then(add_file_to_inbox_handler);

    let upload_file_to_folder_route = warp::path("upload_file_to_folder")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::multipart::form())
        .and_then(upload_file_to_folder_handler);

    public_keys_route
        .or(health_check_route)
        .or(initial_registration_route)
        .or(create_job_route)
        .or(job_message_route)
        .or(get_last_messages_route)
        .or(get_all_smart_inboxes_route)
        .or(available_llm_providers_route)
        .or(retrieve_path_simplified_route)
        .or(retrieve_vector_resource_route)
        .or(convert_files_and_save_route)
        .or(create_folder_route)
        .or(update_smart_inbox_name_route)
        .or(create_files_inbox_route)
        .or(add_file_to_inbox_route)
        .or(upload_file_to_folder_route)
}

fn with_sender(
    sender: Sender<NodeCommand>,
) -> impl Filter<Extract = (Sender<NodeCommand>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || sender.clone())
}

fn with_node_name(node_name: String) -> impl Filter<Extract = (String,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || node_name.clone())
}

fn create_success_response<T: Serialize>(data: T) -> Value {
    json!(data)
}

// Structs

#[derive(Deserialize)]
pub struct InitialRegistrationRequest {
    pub profile_encryption_pk: String,
    pub profile_identity_pk: String,
}

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub job_creation_info: JobCreationInfo,
    pub llm_provider: String,
}

#[derive(Deserialize)]
pub struct JobMessageRequest {
    pub job_message: JobMessage,
}

#[derive(Deserialize)]
pub struct GetLastMessagesRequest {
    pub inbox_name: String,
    pub limit: usize,
    pub offset_key: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateSmartInboxNameRequest {
    pub inbox_name: String,
    pub custom_name: String,
}

#[derive(Deserialize)]
pub struct AddFileToInboxRequest {
    pub file_inbox_name: String,
    pub filename: String,
    pub file: Vec<u8>,
}

// Code

#[utoipa::path(
    get,
    path = "/v2/public_keys",
    responses(
        (status = 200, description = "Successfully retrieved public keys", body = GetPublicKeysResponse),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_public_keys(sender: Sender<NodeCommand>) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetPublicKeys { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/health_check",
    responses(
        (status = 200, description = "Health check successful", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn health_check(sender: Sender<NodeCommand>, node_name: String) -> Result<impl warp::Reply, warp::Rejection> {
    let version = env!("CARGO_PKG_VERSION");

    // Create a channel to receive the result
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command to the node
    sender
        .send(NodeCommand::APIIsPristine { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    // Receive the result
    let pristine_state = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    // Check if there was an error
    if let Err(error) = pristine_state {
        return Ok(warp::reply::json(&json!({ "status": "error", "error": error })));
    }

    // If there was no error, proceed as usual
    Ok(warp::reply::json(
        &json!({ "status": "ok", "version": version, "node_name": node_name, "is_pristine": pristine_state.unwrap() }),
    ))
}

#[utoipa::path(
    post,
    path = "/v2/initial_registration",
    request_body = ShinkaiMessage,
    responses(
        (status = 200, description = "Successfully used registration code", body = APIUseRegistrationCodeSuccessResponse),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn initial_registration_handler(
    node_commands_sender: Sender<NodeCommand>,
    payload: InitialRegistrationRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiInitialRegistration {
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
    path = "/v2/create_job",
    request_body = CreateJobRequest,
    responses(
        (status = 200, description = "Successfully created job", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn create_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: CreateJobRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiCreateJob {
            bearer,
            job_creation_info: payload.job_creation_info,
            llm_provider: payload.llm_provider,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "job_id": response }));
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
    path = "/v2/job_message",
    request_body = JobMessageRequest,
    responses(
        (status = 200, description = "Successfully processed job message", body = SendResponseBody),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn job_message_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: JobMessageRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiJobMessage {
            bearer,
            job_message: payload.job_message,
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
    path = "/v2/last_messages",
    request_body = GetLastMessagesRequest,
    responses(
        (status = 200, description = "Successfully retrieved last messages", body = Vec<V2ChatMessage>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_last_messages_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetLastMessagesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetLastMessagesFromInbox {
            bearer,
            inbox_name: payload.inbox_name,
            limit: payload.limit,
            offset_key: payload.offset_key,
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
    path = "/v2/all_smart_inboxes",
    responses(
        (status = 200, description = "Successfully retrieved all smart inboxes", body = Vec<V2SmartInbox>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_all_smart_inboxes_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetAllSmartInboxes {
            bearer,
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
    path = "/v2/available_llm_providers",
    responses(
        (status = 200, description = "Successfully retrieved available LLM providers", body = Vec<SerializedLLMProvider>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_available_llm_providers_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiAvailableLLMProviders {
            bearer,
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
    path = "/v2/convert_files_and_save",
    request_body = APIConvertFilesAndSaveToFolder,
    responses(
        (status = 200, description = "Successfully converted files and saved to folder", body = Vec<Value>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn convert_files_and_save_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIConvertFilesAndSaveToFolder,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiConvertFilesAndSaveToFolder {
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
    path = "/v2/update_smart_inbox_name",
    request_body = UpdateSmartInboxNameRequest,
    responses(
        (status = 200, description = "Successfully updated smart inbox name", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_smart_inbox_name_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: UpdateSmartInboxNameRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiUpdateSmartInboxName {
            bearer,
            inbox_name: payload.inbox_name,
            custom_name: payload.custom_name,
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
    path = "/v2/create_files_inbox",
    responses(
        (status = 200, description = "Successfully created files inbox", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn create_files_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiCreateFilesInbox {
            bearer,
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
    path = "/v2/add_file_to_inbox",
    request_body = AddFileToInboxRequest,
    responses(
        (status = 200, description = "Successfully added file to inbox", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_file_to_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let mut file_inbox_name = String::new();
    let mut filename = String::new();
    let mut file_data = Vec::new();

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
            "file_inbox_name" => {
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing file_inbox_name",
                    ))
                })?;
                let mut content = content.map_err(|e| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        format!("Failed to read file_inbox_name: {:?}", e).as_str(),
                    ))
                })?;
                file_inbox_name =
                    String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                        warp::reject::custom(APIError::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            "Invalid UTF-8 in file_inbox_name",
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
            _ => {}
        }
    }

    if file_data.is_empty() {
        return Err(warp::reject::custom(APIError::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "No file data found. Check that the file is being uploaded correctly in the `field_data` field",
        )));
    }

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiAddFileToInbox {
            bearer,
            file_inbox_name,
            filename,
            file: file_data,
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
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing path",
                    ))
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
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing file_datetime",
                    ))
                })?;
                let mut content = content.map_err(|e| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        format!("Failed to read file_datetime: {:?}", e).as_str(),
                    ))
                })?;
                let datetime_str = String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Invalid UTF-8 in file_datetime",
                    ))
                })?;
                file_datetime = Some(DateTime::parse_from_rfc3339(&datetime_str).map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Invalid datetime format",
                    ))
                })?.with_timezone(&Utc));
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

#[derive(OpenApi)]
#[openapi(
    paths(
        get_public_keys,
        health_check,
        initial_registration_handler,
        create_job_handler,
        job_message_handler,
        get_all_smart_inboxes_handler,
        get_available_llm_providers_handler,
        retrieve_path_simplified_handler,
        retrieve_vector_resource_handler,
        convert_files_and_save_handler,
        create_folder_handler,
        update_smart_inbox_name_handler,
        create_files_inbox_handler,
        add_file_to_inbox_handler,
    ),
    components(
        schemas(GetPublicKeysResponse, SendResponseBody, SendResponseBodyData, APIError, APIUseRegistrationCodeSuccessResponse)
    ),
    tags(
        (name = "v2", description = "V2 API endpoints")
    )
)]
pub struct ApiDoc;
