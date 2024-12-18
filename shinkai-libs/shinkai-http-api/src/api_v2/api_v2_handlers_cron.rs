use crate::{node_api_router::APIError, node_commands::NodeCommand};
use async_channel::Sender;
use serde::Deserialize;
use shinkai_message_primitives::schemas::crontab::{CronTask, CronTaskAction};
use utoipa::OpenApi;
use warp::http::StatusCode;
use warp::Filter;
use std::collections::HashMap;

use super::api_v2_router::{create_success_response, with_sender};

pub fn cron_routes(
    node_commands_sender: Sender<NodeCommand>,
    _node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let add_cron_task_route = warp::path("add_cron_task")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_cron_task_handler);

    let list_all_cron_tasks_route = warp::path("list_all_cron_tasks")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(list_all_cron_tasks_handler);

    let get_specific_cron_task_route = warp::path("get_specific_cron_task")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(get_specific_cron_task_handler);

    let remove_cron_task_route = warp::path("remove_cron_task")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(remove_cron_task_handler);

    let get_cron_task_logs_route = warp::path("get_cron_task_logs")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(get_cron_task_logs_handler);

    let update_cron_task_route = warp::path("update_cron_task")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_cron_task_handler);

    let force_execute_cron_task_route = warp::path("force_execute_cron_task")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(force_execute_cron_task_handler);

    add_cron_task_route
        .or(list_all_cron_tasks_route)
        .or(get_specific_cron_task_route)
        .or(remove_cron_task_route)
        .or(get_cron_task_logs_route)
        .or(update_cron_task_route)
        .or(force_execute_cron_task_route)
}

#[derive(Deserialize)]
pub struct AddCronTaskRequest {
    cron: String,
    action: CronTaskAction,
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateCronTaskRequest {
    cron_task_id: String,
    cron: String,
    action: CronTaskAction,
    name: String,
    description: Option<String>,
}

#[utoipa::path(
    post,
    path = "/v2/add_cron_task",
    request_body = AddCronTaskRequest,
    responses(
        (status = 200, description = "Successfully added cron task", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_cron_task_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: AddCronTaskRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiAddCronTask {
            bearer,
            name: payload.name,
            description: payload.description,
            cron: payload.cron,
            action: payload.action,
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
    path = "/v2/list_all_cron_tasks",
    responses(
        (status = 200, description = "Successfully listed all cron tasks", body = Vec<CronTask>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_all_cron_tasks_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiListAllCronTasks {
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
    path = "/v2/get_specific_cron_task",
    params(
        ("cron_task_id" = String, Query, description = "Cron task ID to retrieve")
    ),
    responses(
        (status = 200, description = "Successfully retrieved specific cron task", body = Option<CronTask>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_specific_cron_task_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    // Extract cron_task_id from query parameters
    let cron_task_id_str = query_params
        .get("cron_task_id")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "The request query string is invalid.".to_string(),
            })
        })?;

    // Parse cron_task_id to i64
    let cron_task_id: i64 = cron_task_id_str.parse().map_err(|_| {
        warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Query".to_string(),
            message: "The cron_task_id must be a valid integer.".to_string(),
        })
    })?;

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetSpecificCronTask {
            bearer,
            cron_task_id,
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
    path = "/v2/remove_cron_task",
    params(
        ("cron_task_id" = String, Query, description = "Cron task ID to remove")
    ),
    responses(
        (status = 200, description = "Successfully removed cron task", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_cron_task_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    // Extract cron_task_id from query parameters
    let cron_task_id_str = query_params
        .get("cron_task_id")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "The request query string is invalid.".to_string(),
            })
        })?;

    // Parse cron_task_id to i64
    let cron_task_id: i64 = cron_task_id_str.parse().map_err(|_| {
        warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Query".to_string(),
            message: "The cron_task_id must be a valid integer.".to_string(),
        })
    })?;

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiRemoveCronTask {
            bearer,
            cron_task_id,
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
    path = "/v2/get_cron_task_logs",
    params(
        ("cron_task_id" = String, Query, description = "Cron task ID to retrieve logs for")
    ),
    responses(
        (status = 200, description = "Successfully retrieved cron task logs", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_cron_task_logs_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    // Extract cron_task_id from query parameters
    let cron_task_id_str = query_params
        .get("cron_task_id")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "The request query string is invalid.".to_string(),
            })
        })?;

    // Parse cron_task_id to i64
    let cron_task_id: i64 = cron_task_id_str.parse().map_err(|_| {
        warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Query".to_string(),
            message: "The cron_task_id must be a valid integer.".to_string(),
        })
    })?;

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetCronTaskLogs {
            bearer,
            cron_task_id,
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
    put,
    path = "/v2/update_cron_task",
    params(
        ("cron_task_id" = String, Query, description = "Cron task ID to update")
    ),
    request_body = UpdateCronTaskRequest,
    responses(
        (status = 200, description = "Successfully updated cron task", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_cron_task_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: UpdateCronTaskRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let cron_task_id: i64 = payload.cron_task_id.parse().map_err(|_| warp::reject::reject())?;
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiUpdateCronTask {
            bearer,
            cron_task_id,
            cron: payload.cron,
            action: payload.action,
            name: payload.name,
            description: payload.description,
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
    path = "/v2/force_execute_cron_task",
    params(
        ("cron_task_id" = String, Query, description = "Cron task ID to force execute")
    ),
    responses(
        (status = 200, description = "Successfully forced execution of cron task", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn force_execute_cron_task_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    // Extract cron_task_id from query parameters
    let cron_task_id_str = query_params
        .get("cron_task_id")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid Query".to_string(),
                message: "The request query string is invalid.".to_string(),
            })
        })?;

    // Parse cron_task_id to i64
    let cron_task_id: i64 = cron_task_id_str.parse().map_err(|_| {
        warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Query".to_string(),
            message: "The cron_task_id must be a valid integer.".to_string(),
        })
    })?;

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiForceExecuteCronTask {
            bearer,
            cron_task_id,
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
        add_cron_task_handler,
        list_all_cron_tasks_handler,
        get_specific_cron_task_handler,
        remove_cron_task_handler,
        get_cron_task_logs_handler,
        update_cron_task_handler,
        force_execute_cron_task_handler,
    ),
    components(
        schemas(CronTask, CronTaskAction, APIError)
    ),
    tags(
        (name = "cron", description = "Cron Task API endpoints")
    )
)]
pub struct CronApiDoc;
