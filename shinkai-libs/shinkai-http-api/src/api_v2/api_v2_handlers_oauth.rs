use async_channel::Sender;
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use utoipa::{OpenApi, ToSchema};
use warp::Filter;

use super::api_v2_router::{create_success_response, with_sender};
use crate::{node_api_router::APIError, node_commands::NodeCommand};

#[derive(Deserialize, ToSchema)]
pub struct OAuthTokenRequest {
    pub state: String,
    pub code: String,
}

pub fn oauth_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let get_oauth_token = warp::path("get_oauth_token")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(get_oauth_token_handler);

    let set_oauth_token = warp::path("set_oauth_token")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_oauth_token_handler);

    get_oauth_token.or(set_oauth_token)
}

#[utoipa::path(
    get,
    path = "/v2/get_oauth_token",
    params(
        ("connection_name" = String, Query, description = "Name of the OAuth connection"),
        ("tool_key" = String, Query, description = "Key of the tool")
    ),
    responses(
        (status = 200, description = "Successfully retrieved OAuth token", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_oauth_token_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let connection_name = query_params
        .get("connection_name")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "connection_name is required".to_string(),
            })
        })?
        .to_string();

    let tool_key = query_params
        .get("tool_key")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "tool_key is required".to_string(),
            })
        })?
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetOAuthToken {
            bearer,
            connection_name,
            tool_key,
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
    path = "/v2/set_oauth_token",
    request_body = OAuthTokenRequest,
    responses(
        (status = 200, description = "Successfully set OAuth token", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_oauth_token_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: OAuthTokenRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetOAuthToken {
            bearer,
            code: payload.code,
            state: payload.state,
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
        get_oauth_token_handler,
        set_oauth_token_handler
    ),
    components(
        schemas(
            APIError,
            OAuthTokenRequest
        )
    ),
    tags(
        (name = "oauth", description = "OAuth API endpoints")
    )
)]
pub struct OAuthApiDoc;
