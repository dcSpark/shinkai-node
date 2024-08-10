use async_channel::Sender;
use reqwest::StatusCode;
use serde::Deserialize;
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

    public_keys_route.or(health_check_route).or(initial_registration_route)
}

#[derive(Deserialize)]
pub struct InitialRegistrationRequest {
    pub profile_encryption_pk: String,
    pub profile_identity_pk: String,
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

#[derive(OpenApi)]
#[openapi(
    paths(
        get_public_keys,
        health_check,
        initial_registration_handler,
    ),
    components(
        schemas(GetPublicKeysResponse, APIError)
    ),
    tags(
        (name = "general", description = "General API endpoints")
    )
)]
pub struct GeneralApiDoc;
