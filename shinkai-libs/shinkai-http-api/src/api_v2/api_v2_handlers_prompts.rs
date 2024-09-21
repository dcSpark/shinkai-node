use async_channel::Sender;
use serde::Deserialize;
use shinkai_message_primitives::schemas::custom_prompt::CustomPrompt;
use warp::Filter;

use crate::node_commands::NodeCommand;

use super::api_v2_router::with_sender;

pub fn prompt_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let add_custom_prompt_route = warp::path("add_custom_prompt")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_custom_prompt_handler);

    let delete_custom_prompt_route = warp::path("delete_custom_prompt")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(delete_custom_prompt_handler);

    let get_all_custom_prompts_route = warp::path("get_all_custom_prompts")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_all_custom_prompts_handler);

    let get_custom_prompt_route = warp::path("get_custom_prompt")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<GetCustomPromptRequest>())
        .and_then(get_custom_prompt_handler);

    let search_custom_prompts_route = warp::path("search_custom_prompts")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<SearchCustomPromptsRequest>())
        .and_then(search_custom_prompts_handler);

    let update_custom_prompt_route = warp::path("update_custom_prompt")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_custom_prompt_handler);

    add_custom_prompt_route
        .or(delete_custom_prompt_route)
        .or(get_all_custom_prompts_route)
        .or(get_custom_prompt_route)
        .or(search_custom_prompts_route)
        .or(update_custom_prompt_route)
}

#[derive(Deserialize)]
pub struct GetCustomPromptRequest {
    pub prompt_name: String,
}

#[derive(Deserialize)]
pub struct SearchCustomPromptsRequest {
    pub query: String,
}

#[utoipa::path(
    post,
    path = "/v2/add_custom_prompt",
    request_body = CustomPrompt,
    responses(
        (status = 200, description = "Successfully added custom prompt", body = CustomPrompt),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_custom_prompt_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: CustomPrompt,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiAddCustomPrompt {
            bearer,
            prompt: payload,
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
    path = "/v2/delete_custom_prompt",
    request_body = GetCustomPromptRequest,
    responses(
        (status = 200, description = "Successfully deleted custom prompt", body = CustomPrompt),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn delete_custom_prompt_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetCustomPromptRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiDeleteCustomPrompt {
            bearer,
            prompt_name: payload.prompt_name,
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
    path = "/v2/get_all_custom_prompts",
    responses(
        (status = 200, description = "Successfully retrieved all custom prompts", body = Vec<CustomPrompt>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_all_custom_prompts_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetAllCustomPrompts {
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
    get,
    path = "/v2/get_custom_prompt",
    params(
        ("prompt_name" = String, Query, description = "Name of the custom prompt to retrieve")
    ),
    responses(
        (status = 200, description = "Successfully retrieved custom prompt", body = CustomPrompt),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_custom_prompt_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query: GetCustomPromptRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetCustomPrompt {
            bearer,
            prompt_name: query.prompt_name,
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
    path = "/v2/search_custom_prompts",
    params(
        ("query" = String, Query, description = "Search query for custom prompts")
    ),
    responses(
        (status = 200, description = "Successfully searched custom prompts", body = Vec<CustomPrompt>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn search_custom_prompts_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query: SearchCustomPromptsRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSearchCustomPrompts {
            bearer,
            query: query.query,
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
    path = "/v2/update_custom_prompt",
    request_body = CustomPrompt,
    responses(
        (status = 200, description = "Successfully updated custom prompt", body = CustomPrompt),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_custom_prompt_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: CustomPrompt,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateCustomPrompt {
            bearer,
            prompt: payload,
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
