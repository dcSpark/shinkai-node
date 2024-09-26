use std::collections::HashMap;

use async_channel::Sender;
use reqwest::StatusCode;

use serde_json::Value;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{APISetWorkflow, APIWorkflowKeyname};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use utoipa::OpenApi;
use warp::Filter;

use crate::{node_api_router::APIError, node_commands::NodeCommand};

use super::api_v2_router::{create_success_response, with_sender};

pub fn workflows_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let search_workflows_route = warp::path("search_workflows")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(search_workflows_handler);

    let set_workflow_route = warp::path("set_workflow")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_workflow_handler);

    let remove_workflow_route = warp::path("remove_workflow")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(remove_workflow_handler);

    let get_workflow_info_route = warp::path("get_workflow_info")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<APIWorkflowKeyname>())
        .and_then(get_workflow_info_handler);

    let list_all_workflows_route = warp::path("list_all_workflows")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(list_all_workflows_handler);

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

    search_workflows_route
        .or(set_workflow_route)
        .or(remove_workflow_route)
        .or(get_workflow_info_route)
        .or(list_all_workflows_route)
        .or(list_all_shinkai_tools_route)
        .or(set_shinkai_tool_route)
        .or(get_shinkai_tool_route)
        .or(search_shinkai_tool_route)
        .or(add_shinkai_tool_route)
}

#[utoipa::path(
    get,
    path = "/v2/search_workflows",
    params(
        ("query" = String, Query, description = "Search query for workflows")
    ),
    responses(
        (status = 200, description = "Successfully searched workflows", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn search_workflows_handler(
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
        .send(NodeCommand::V2ApiSearchWorkflows {
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
    post,
    path = "/v2/set_workflow",
    request_body = APISetWorkflow,
    responses(
        (status = 200, description = "Successfully set workflow", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_workflow_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: APISetWorkflow,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetWorkflow {
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
    path = "/v2/remove_workflow",
    request_body = APIWorkflowKeyname,
    responses(
        (status = 200, description = "Successfully removed workflow", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_workflow_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIWorkflowKeyname,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiRemoveWorkflow {
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
    path = "/v2/get_workflow_info",
    params(
        ("tool_router_key" = String, Query, description = "Keyname of the workflow")
    ),
    responses(
        (status = 200, description = "Successfully retrieved workflow info", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_workflow_info_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    keyname: APIWorkflowKeyname,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetWorkflowInfo {
            bearer,
            payload: keyname,
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
    path = "/v2/list_all_workflows",
    responses(
        (status = 200, description = "Successfully listed all workflows", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_all_workflows_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListAllWorkflows {
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
        search_workflows_handler,
        set_workflow_handler,
        remove_workflow_handler,
        get_workflow_info_handler,
        list_all_workflows_handler,
        list_all_shinkai_tools_handler,
        set_shinkai_tool_handler,
        get_shinkai_tool_handler,
        search_shinkai_tool_handler,
        add_shinkai_tool_handler,
    ),
    components(
        schemas(APIError, APIWorkflowKeyname, APISetWorkflow)
    ),
    tags(
        (name = "workflows", description = "Workflow API endpoints")
    )
)]
pub struct WorkflowsApiDoc;
