use std::collections::HashMap;

use async_channel::Sender;
use reqwest::StatusCode;

use serde_json::Value;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use utoipa::OpenApi;
use warp::Filter;

use crate::{node_api_router::APIError, node_commands::NodeCommand};

use super::api_v2_router::{create_success_response, with_sender};

pub fn workflows_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let list_all_shinkai_tools_route = warp::path("list_all_shinkai_tools")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(list_all_shinkai_tools_handler);

    let set_shinkai_tool_route = warp::path("set_shinkai_tool")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::json())
        .and_then(set_shinkai_tool_handler);

    let get_shinkai_tool_route = warp::path("get_shinkai_tool")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(get_shinkai_tool_handler);

    let search_shinkai_tool_route = warp::path("search_shinkai_tool")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(search_shinkai_tool_handler);

    let add_shinkai_tool_route = warp::path("add_shinkai_tool")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_shinkai_tool_handler);

    list_all_shinkai_tools_route
        .or(set_shinkai_tool_route)
        .or(get_shinkai_tool_route)
        .or(search_shinkai_tool_route)
        .or(add_shinkai_tool_route)
}

#[utoipa::path(
    get,
    path = "/v2/search_shinkai_tool",
    params(
        ("query" = String, Query, description = "Search query for Shinkai tools")
    ),
    responses(
        (status = 200, description = "Successfully searched Shinkai tools", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn search_shinkai_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let query = query_params
        .get("query")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "The request query string is invalid.".to_string(),
            })
        })?
        .to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSearchShinkaiTool {
            bearer,
            query,
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
    path = "/v2/list_all_shinkai_tools",
    responses(
        (status = 200, description = "Successfully listed all Shinkai tools", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_all_shinkai_tools_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListAllShinkaiTools {
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
    path = "/v2/set_shinkai_tool",
    request_body = Value,
    params(
        ("tool_name" = String, Query, description = "Key name of the Shinkai tool")
    ),
    responses(
        (status = 200, description = "Successfully set Shinkai tool", body = bool),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_shinkai_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
    payload: Value,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let tool_key = query_params
        .get("tool_name")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "The request query string is invalid.".to_string(),
            })
        })?
        .to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetShinkaiTool {
            bearer,
            tool_key,
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
    path = "/v2/get_shinkai_tool",
    params(
        ("tool_name" = String, Query, description = "Name of the Shinkai tool")
    ),
    responses(
        (status = 200, description = "Successfully retrieved Shinkai tool", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_shinkai_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let tool_name = query_params
        .get("tool_name")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "The request query string is invalid.".to_string(),
            })
        })?
        .to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetShinkaiTool {
            bearer,
            payload: tool_name,
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
    path = "/v2/add_shinkai_tool",
    request_body = ShinkaiTool,
    responses(
        (status = 200, description = "Successfully added Shinkai tool", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_shinkai_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ShinkaiTool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiAddShinkaiTool {
            bearer,
            shinkai_tool: payload,
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
        list_all_shinkai_tools_handler,
        set_shinkai_tool_handler,
        get_shinkai_tool_handler,
        search_shinkai_tool_handler,
        add_shinkai_tool_handler,
    ),
    components(
        schemas(APIError)
    ),
    tags(
        (name = "workflows", description = "Workflow API endpoints")
    )
)]
pub struct WorkflowsApiDoc;
