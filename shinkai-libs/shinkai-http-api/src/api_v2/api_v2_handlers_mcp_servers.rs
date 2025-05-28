use async_channel::Sender;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::mcp_server::{MCPServer, MCPServerEnv, MCPServerType};
use utoipa::{OpenApi, ToSchema};
use warp::Filter;

use crate::{node_api_router::APIError, node_commands::NodeCommand};

use super::api_v2_router::with_sender;

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct AddMCPServerRequest {
    pub name: String,
    pub r#type: MCPServerType,
    pub url: Option<String>,
    pub command: Option<String>,
    pub env: Option<MCPServerEnv>,
    pub is_enabled: bool,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct UpdateMCPServerRequest {
    pub id: i64,
    pub name: Option<String>,
    pub r#type: MCPServerType,
    pub url: Option<String>,
    pub command: Option<String>,
    pub env: Option<MCPServerEnv>,
    pub is_enabled: Option<bool>,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct ImportMCPServerFromGitHubRequest {
    pub github_url: String,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct GetAllMCPServerToolsRequest {
    pub mcp_server_id: i64,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct DeleteMCPServerRequest {
    pub mcp_server_id: i64,
}

#[derive(Deserialize, Serialize, ToSchema, Debug)]
pub struct DeleteMCPServerResponse {
    pub tools_deleted: i64,
    pub deleted_mcp_server: MCPServer,
    pub message: Option<String>,
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct SetEnableMCPServerRequest {
    pub mcp_server_id: i64,
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

    let update_mcp_server_route = warp::path("update_mcp_server")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_mcp_server_handler);

    let import_mcp_server_from_github_url_route = warp::path("import_mcp_server_from_github_url")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(import_mcp_server_from_github_url_handler);

    let get_all_mcp_server_tools_route = warp::path("mcp_server_tools")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<GetAllMCPServerToolsRequest>())
        .and_then(get_all_mcp_server_tools_handler);

    let delete_mcp_server_route = warp::path("delete_mcp_server")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(delete_mcp_server_handler);

    let set_enable_mcp_server_route = warp::path("set_enable_mcp_server")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_enable_mcp_server_handler);

    list_mcp_servers_route
        .or(add_mcp_server_route)
        .or(get_all_mcp_server_tools_route)
        .or(delete_mcp_server_route)
        .or(import_mcp_server_from_github_url_route)
        .or(set_enable_mcp_server_route)
        .or(update_mcp_server_route)
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
    let bearer = bearer.strip_prefix("Bearer ").unwrap_or("").to_string();
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

#[utoipa::path(
    post,
    path = "/v2/delete_mcp_server",
    request_body = DeleteMCPServerRequest,
    responses(
        (status = 200, description = "Successfully deleted MCP server", body = DeleteMCPServerResponse),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn delete_mcp_server_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: DeleteMCPServerRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiDeleteMCPServer {
            bearer,
            mcp_server_id: payload.mcp_server_id,
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
    get,
    path = "/v2/get_all_mcp_server_tools",
    responses(
        (status = 200, description = "Successfully retrieved MCP server tools", body = Vec<ShinkaiTool>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_all_mcp_server_tools_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetAllMCPServerToolsRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetAllMCPServerTools {
            bearer,
            mcp_server_id: payload.mcp_server_id,
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
    path = "/v2/import_mcp_server_from_github_url",
    request_body = ImportMCPServerFromGitHubRequest,
    responses(
        (status = 200, description = "Successfully imported MCP server", body = AddMCPServerRequest),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn import_mcp_server_from_github_url_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ImportMCPServerFromGitHubRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiImportMCPServerFromGitHubURL {
            bearer,
            github_url: payload.github_url,
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
    path = "/v2/set_enable_mcp_server",
    request_body = SetEnableMCPServerRequest,
    responses(
        (status = 200, description = "Successfully set enable MCP server", body = MCPServer),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_enable_mcp_server_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: SetEnableMCPServerRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetEnableMCPServer {
            bearer,
            mcp_server_id: payload.mcp_server_id,
            is_enabled: payload.is_enabled,
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
    path = "/v2/update_mcp_server",
    request_body = UpdateMCPServerRequest,
    responses(
        (status = 200, description = "Successfully updated MCP server", body = MCPServer),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_mcp_server_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: UpdateMCPServerRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateMCPServer {
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
        get_all_mcp_server_tools_handler,
        import_mcp_server_from_github_url_handler,
        delete_mcp_server_handler,
        set_enable_mcp_server_handler,
        update_mcp_server_handler,
    ),
    components(
        schemas(AddMCPServerRequest, MCPServer, APIError)
    ),
    tags(
        (name = "mcp_servers", description = "MCP Server API endpoints")
    )
)]
pub struct MCPServerApiDoc;
