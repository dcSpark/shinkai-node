use async_channel::Sender;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::OpenApi;
use warp::Filter;

use crate::network::{node_api_router::{APIError, GetPublicKeysResponse}, node_commands::NodeCommand};

use super::api_v2_router::{create_success_response, with_node_name, with_sender};

pub fn general_routes(
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

    let initial_registration_route = warp::path("initial_registration")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::body::json())
        .and_then(initial_registration_handler);

    let get_local_processing_preference_route = warp::path("local_processing_preference")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_local_processing_preference_handler);

    let update_local_processing_preference_route = warp::path("local_processing_preference")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_local_processing_preference_handler);

    let get_default_embedding_model_route = warp::path("default_embedding_model")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_default_embedding_model_handler);

    let get_supported_embedding_models_route = warp::path("supported_embedding_models")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_supported_embedding_models_handler);

    let update_default_embedding_model_route = warp::path("default_embedding_model")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_default_embedding_model_handler);

    let update_supported_embedding_models_route = warp::path("supported_embedding_models")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_supported_embedding_models_handler);

    public_keys_route
        .or(health_check_route)
        .or(initial_registration_route)
        .or(get_local_processing_preference_route)
        .or(update_local_processing_preference_route)
        .or(get_default_embedding_model_route)
        .or(get_supported_embedding_models_route)
        .or(update_default_embedding_model_route)
        .or(update_supported_embedding_models_route)
}

#[derive(Deserialize)]
pub struct InitialRegistrationRequest {
    pub profile_encryption_pk: String,
    pub profile_identity_pk: String,
}

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

    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::APIIsPristine { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let pristine_state = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    if let Err(error) = pristine_state {
        return Ok(warp::reply::json(&json!({ "status": "error", "error": error })));
    }

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
    get,
    path = "/v2/local_processing_preference",
    responses(
        (status = 200, description = "Successfully retrieved local processing preference", body = bool),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_local_processing_preference_handler(
    sender: Sender<NodeCommand>,
    bearer: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetLocalProcessingPreference { bearer, res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    post,
    path = "/v2/local_processing_preference",
    request_body = bool,
    responses(
        (status = 200, description = "Successfully updated local processing preference", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_local_processing_preference_handler(
    sender: Sender<NodeCommand>,
    bearer: String,
    preference: bool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateLocalProcessingPreference { bearer, preference, res: res_sender })
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
    path = "/v2/default_embedding_model",
    responses(
        (status = 200, description = "Successfully retrieved default embedding model", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_default_embedding_model_handler(
    sender: Sender<NodeCommand>,
    bearer: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetDefaultEmbeddingModel { bearer, res: res_sender })
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
    path = "/v2/supported_embedding_models",
    responses(
        (status = 200, description = "Successfully retrieved supported embedding models", body = Vec<String>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_supported_embedding_models_handler(
    sender: Sender<NodeCommand>,
    bearer: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetSupportedEmbeddingModels { bearer, res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    post,
    path = "/v2/default_embedding_model",
    request_body = String,
    responses(
        (status = 200, description = "Successfully updated default embedding model", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_default_embedding_model_handler(
    sender: Sender<NodeCommand>,
    bearer: String,
    model_name: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateDefaultEmbeddingModel { bearer, model_name, res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    post,
    path = "/v2/supported_embedding_models",
    request_body = Vec<String>,
    responses(
        (status = 200, description = "Successfully updated supported embedding models", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_supported_embedding_models_handler(
    sender: Sender<NodeCommand>,
    bearer: String,
    models: Vec<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateSupportedEmbeddingModels { bearer, models, res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_public_keys,
        health_check,
        initial_registration_handler,
        get_local_processing_preference_handler,
        update_local_processing_preference_handler,
        get_default_embedding_model_handler,
        get_supported_embedding_models_handler,
        update_default_embedding_model_handler,
        update_supported_embedding_models_handler,
    ),
    components(
        schemas(GetPublicKeysResponse, APIError)
    ),
    tags(
        (name = "general", description = "General API endpoints")
    )
)]
pub struct GeneralApiDoc;