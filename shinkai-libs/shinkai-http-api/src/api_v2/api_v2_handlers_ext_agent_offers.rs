use async_channel::Sender;
use serde::Deserialize;
use serde_json::json;
use utoipa::{OpenApi, ToSchema};
use warp::http::StatusCode;
use warp::Filter;

use shinkai_message_primitives::schemas::{
    shinkai_tool_offering::{ShinkaiToolOffering, ToolPrice, UsageType},
    wallet_mixed::{Asset, NetworkIdentifier},
    x402_types::PaymentRequirements,
};

use crate::{node_api_router::APIError, node_commands::NodeCommand};

use super::api_v2_router::{create_success_response, with_sender};

pub fn ext_agent_offers_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let set_tool_offering_route = warp::path("set_tool_offering")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_tool_offering_handler);

    let get_tool_offering_route = warp::path("get_tool_offering")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(get_tool_offering_handler);

    let get_tool_with_offering_route = warp::path("get_tool_with_offering")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(get_tool_with_offering_handler);

    let get_tools_with_offerings_route = warp::path("get_tools_with_offerings")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_tools_with_offerings_handler);

    let remove_tool_offering_route = warp::path("remove_tool_offering")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(remove_tool_offering_handler);

    let get_all_tool_offerings_route = warp::path("get_all_tool_offerings")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_all_tool_offerings_handler);

    let get_agent_network_offering_route = warp::path("get_agent_network_offering")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(get_agent_network_offering_handler);

    set_tool_offering_route
        .or(get_tool_offering_route)
        .or(remove_tool_offering_route)
        .or(get_all_tool_offerings_route)
        .or(get_tool_with_offering_route)
        .or(get_tools_with_offerings_route)
        .or(get_agent_network_offering_route)
}

#[derive(Deserialize, ToSchema)]
pub struct SetToolOfferingRequest {
    pub tool_offering: ShinkaiToolOffering,
}

#[derive(Deserialize, ToSchema)]
pub struct GetToolOfferingRequest {
    pub tool_key_name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RemoveToolOfferingRequest {
    pub tool_key_name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct GetToolWithOfferingRequest {
    pub tool_key_name: String,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize, ToSchema)]
pub struct GetAgentNetworkOfferingRequest {
    pub node_name: String,
    #[serde(default = "default_true")]
    pub auto_check: bool,
}

#[utoipa::path(
    post,
    path = "/v2/set_tool_offering",
    request_body = SetToolOfferingRequest,
    responses(
        (status = 200, description = "Successfully set tool offering", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_tool_offering_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: SetToolOfferingRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiSetToolOffering {
            bearer,
            tool_offering: payload.tool_offering,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "result": response }));
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
    path = "/v2/get_tool_offering",
    request_body = GetToolOfferingRequest,
    responses(
        (status = 200, description = "Successfully retrieved tool offering", body = ShinkaiToolOffering),
        (status = 400, description = "Bad request", body = APIError),
        (status = 404, description = "Tool offering not found", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_tool_offering_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetToolOfferingRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetToolOffering {
            bearer,
            tool_key_name: payload.tool_key_name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(tool_offering) => Ok(warp::reply::with_status(
            warp::reply::json(&tool_offering),
            StatusCode::OK,
        )),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/remove_tool_offering",
    request_body = RemoveToolOfferingRequest,
    responses(
        (status = 200, description = "Successfully removed tool offering", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 404, description = "Tool offering not found", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_tool_offering_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: RemoveToolOfferingRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiRemoveToolOffering {
            bearer,
            tool_key_name: payload.tool_key_name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "result": response }));
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
    path = "/v2/get_all_tool_offerings",
    responses(
        (status = 200, description = "Successfully retrieved all tool offerings", body = Vec<ShinkaiToolOffering>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_all_tool_offerings_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetAllToolOfferings {
            bearer,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(tool_offerings) => Ok(warp::reply::with_status(
            warp::reply::json(&tool_offerings),
            StatusCode::OK,
        )),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/get_tool_with_offering",
    request_body = GetToolWithOfferingRequest,
    responses(
        (status = 200, description = "Successfully retrieved network tool and offering", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 404, description = "Not found", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_tool_with_offering_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetToolWithOfferingRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetToolWithOffering {
            bearer,
            tool_key_name: payload.tool_key_name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(info) => Ok(warp::reply::with_status(warp::reply::json(&info), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    get,
    path = "/v2/get_tools_with_offerings",
    responses(
        (status = 200, description = "Successfully retrieved all network tools and offerings", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_tools_with_offerings_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetToolsWithOfferings {
            bearer,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(info) => Ok(warp::reply::with_status(warp::reply::json(&info), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/v2/get_agent_network_offering",
    request_body = GetAgentNetworkOfferingRequest,
    responses(
        (status = 200, description = "Successfully retrieved agent network offering", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_agent_network_offering_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetAgentNetworkOfferingRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetAgentNetworkOffering {
            bearer,
            node_name: payload.node_name,
            auto_check: payload.auto_check,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;
    match result {
        Ok(info) => Ok(warp::reply::with_status(warp::reply::json(&info), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        set_tool_offering_handler,
        get_tool_offering_handler,
        remove_tool_offering_handler,
        get_all_tool_offerings_handler,
        get_tool_with_offering_handler,
        get_tools_with_offerings_handler,
        get_agent_network_offering_handler
    ),
    components(
        schemas(ShinkaiToolOffering, APIError, GetToolOfferingRequest, UsageType, ToolPrice, PaymentRequirements, Asset, NetworkIdentifier,
            RemoveToolOfferingRequest, SetToolOfferingRequest, GetToolWithOfferingRequest, GetAgentNetworkOfferingRequest)
    ),
    tags(
        (name = "tool_offerings", description = "Tool Offering API endpoints")
    )
)]
pub struct ToolOfferingsApiDoc;
