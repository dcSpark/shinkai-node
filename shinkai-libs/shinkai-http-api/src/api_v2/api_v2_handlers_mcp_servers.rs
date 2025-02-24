use async_channel::Sender;
use serde::Deserialize;
use shinkai_message_primitives::schemas::mcp_server::{MCPServer, MCPServerType};
use utoipa::{openapi, OpenApi, ToSchema};
use warp::Filter;

use crate::{node_api_router::APIError, node_commands::NodeCommand};

use super::api_v2_router::with_sender;

#[derive(Deserialize, ToSchema, Debug)]
pub struct AddMCPServerRequest {
    pub name: String,
    pub r#type: MCPServerType,
    pub url: Option<String>,
    pub command: Option<String>,
    pub is_enabled: bool,
}

pub fn mcp_server_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let list_mcp_servers_route = warp::path("mcp_servers")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(list_mcp_servers_handler);

    let add_mcp_server_route = warp::path("add_mcp_server")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_mcp_server_handler);

    list_mcp_servers_route.or(add_mcp_server_route)
}

#[utoipa::path(
    get,
    path = "/v2/list_mcp_servers",
    responses(
        (status = 200, description = "Successfully retrieved MCP servers", body = Vec<MCPServer>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_mcp_servers_handler(
    sender: Sender<NodeCommand>,
    bearer: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListMCPServers {
            bearer,
            res: res_sender,
        })
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
    path = "/v2/add_mcp_server",
    request_body = AddMCPServerRequest,
    responses(
        (status = 200, description = "Successfully added MCP server", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_mcp_server_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: AddMCPServerRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiAddMCPServer {
            bearer,
            mcp_server: payload,
            res: res_sender,
        })
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
        list_mcp_servers_handler,
        add_mcp_server_handler,
    ),
    components(
        schemas(AddMCPServerRequest, MCPServer, APIError)
    ),
    tags(
        (name = "mcp_servers", description = "MCP Server API endpoints")
    )
)]
pub struct MCPServerApiDoc;
