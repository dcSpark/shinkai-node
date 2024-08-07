use crate::network::{node_commands::NodeCommand, v1_api::api_v1_handlers::APIUseRegistrationCodeSuccessResponse};

use super::super::node_api_router::{APIError, GetPublicKeysResponse, SendResponseBody, SendResponseBodyData};
use async_channel::Sender;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIVecFsRetrievePathSimplifiedJson, JobCreationInfo, JobMessage,
};
use utoipa::OpenApi;
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
        .and(warp::query::<Option<APIVecFsRetrievePathSimplifiedJson>>())
        .and_then(retrieve_path_simplified_handler);

    public_keys_route
        .or(health_check_route)
        .or(initial_registration_route)
        .or(create_job_route)
        .or(job_message_route)
        .or(get_last_messages_route)
        .or(get_all_smart_inboxes_route)
        .or(available_llm_providers_route)
        .or(retrieve_path_simplified_route)
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
    json!({
        "status": "success",
        "data": data,
    })
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
    get,
    path = "/v2/retrieve_path_simplified",
    params(
        ("authorization" = String, Header, description = "Bearer token"),
        ("payload" = Option<APIVecFsRetrievePathSimplifiedJson>, Query, description = "Path retrieval parameters")
    ),
    responses(
        (status = 200, description = "Successfully retrieved path", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn retrieve_path_simplified_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: Option<APIVecFsRetrievePathSimplifiedJson>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);

    let payload = payload.unwrap_or(APIVecFsRetrievePathSimplifiedJson {
        path: "/".to_string(),
    });

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
    ),
    components(
        schemas(GetPublicKeysResponse, SendResponseBody, SendResponseBodyData, APIError, APIUseRegistrationCodeSuccessResponse)
    ),
    tags(
        (name = "v2", description = "V2 API endpoints")
    )
)]
pub struct ApiDoc;
