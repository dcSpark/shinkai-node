use async_channel::Sender;
use reqwest::StatusCode;
use serde::Deserialize;
use utoipa::ToSchema;
use warp::Filter;

use super::api_v2_router::{create_success_response, with_sender};
use crate::node_commands::NodeCommand;

#[derive(Deserialize, ToSchema)]
pub struct SetNgrokAuthTokenRequest {
    pub auth_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SetNgrokEnabledRequest {
    pub enabled: bool,
}

pub fn ngrok_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let set_auth_token_route = warp::path("set_ngrok_auth_token")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_ngrok_auth_token_handler);

    let clear_auth_token_route = warp::path("clear_ngrok_auth_token")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(clear_ngrok_auth_token_handler);

    let set_enabled_route = warp::path("set_ngrok_enabled")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_ngrok_enabled_handler);

    let get_status_route = warp::path("get_ngrok_status")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_ngrok_status_handler);

    set_auth_token_route
        .or(clear_auth_token_route)
        .or(set_enabled_route)
        .or(get_status_route)
}

#[utoipa::path(
    post,
    path = "/v2/set_ngrok_auth_token",
    request_body = SetNgrokAuthTokenRequest,
    responses(
        (status = 200, description = "Successfully set ngrok auth token", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_ngrok_auth_token_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: SetNgrokAuthTokenRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Api,
        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Info,
        &format!("Setting ngrok auth token: {}", payload.auth_token),
    );
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetNgrokAuthToken {
            bearer,
            auth_token: payload.auth_token,
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
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    post,
    path = "/v2/clear_ngrok_auth_token",
    responses(
        (status = 200, description = "Successfully cleared ngrok auth token", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn clear_ngrok_auth_token_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiClearNgrokAuthToken {
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
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    post,
    path = "/v2/set_ngrok_enabled",
    request_body = SetNgrokEnabledRequest,
    responses(
        (status = 200, description = "Successfully set ngrok enabled state", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_ngrok_enabled_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: SetNgrokEnabledRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetNgrokEnabled {
            bearer,
            enabled: payload.enabled,
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
    path = "/v2/get_ngrok_status",
    responses(
        (status = 200, description = "Successfully got ngrok status", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_ngrok_status_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetNgrokStatus { bearer, res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(response);
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Err(warp::reject::custom(error)),
    }
}