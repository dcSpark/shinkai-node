use async_channel::Sender;
use serde::Deserialize;
use serde_json::{Map, Value};
use shinkai_message_primitives::{schemas::shinkai_tools::{CodeLanguage, DynamicToolType}, shinkai_message::shinkai_message_schemas::JobMessage};
use shinkai_tools_primitives::tools::{tool_playground::ToolPlayground, shinkai_tool::ShinkaiTool};
use utoipa::{OpenApi, ToSchema};
use warp::Filter;
use reqwest::StatusCode;
use std::collections::HashMap;

use crate::{node_api_router::APIError, node_commands::NodeCommand};
use super::api_v2_router::{create_success_response, with_sender};

pub fn tool_routes(
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


    let tool_execution_route = warp::path("tool_execution")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::body::json())
        .and_then(tool_execution_handler);
    
    let tool_definitions_route = warp::path("tool_definitions")
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(tool_definitions_handler);

    let tool_implementation_route = warp::path("tool_implementation")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(tool_implementation_handler);

    let tool_metadata_implementation_route = warp::path("tool_metadata_implementation")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(tool_metadata_implementation_handler);

    let set_playground_tool_route = warp::path("set_playground_tool")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_playground_tool_handler);

    let list_playground_tools_route = warp::path("list_playground_tools")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(list_playground_tools_handler);

    let remove_playground_tool_route = warp::path("remove_playground_tool")
        .and(warp::delete())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(remove_playground_tool_handler);

    let get_playground_tool_route = warp::path("get_playground_tool")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(get_playground_tool_handler);

    let get_tool_implementation_prompt_route = warp::path("get_tool_implementation_prompt")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(get_tool_implementation_prompt_handler);

    let code_execution_route = warp::path("code_execution")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::body::json())
        .and_then(code_execution_handler);

    let undo_to_route = warp::path("tool_implementation_undo_to")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(undo_to_handler);

    let tool_implementation_code_update_route = warp::path("tool_implementation_code_update")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(tool_implementation_code_update_handler);

    let resolve_shinkai_file_protocol_route = warp::path("resolve_shinkai_file_protocol")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(resolve_shinkai_file_protocol_handler);

    let export_tool_route = warp::path("export_tool")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(export_tool_handler);

    let import_tool_route = warp::path("import_tool")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(import_tool_handler);

    tool_execution_route
        .or(code_execution_route)
        .or(tool_definitions_route)
        .or(tool_implementation_route)
        .or(tool_metadata_implementation_route)
        .or(list_all_shinkai_tools_route)
        .or(set_shinkai_tool_route)
        .or(get_shinkai_tool_route)
        .or(search_shinkai_tool_route)
        .or(add_shinkai_tool_route)
        .or(set_playground_tool_route)
        .or(list_playground_tools_route)
        .or(remove_playground_tool_route)
        .or(get_playground_tool_route)
        .or(get_tool_implementation_prompt_route)
        .or(undo_to_route)
        .or(tool_implementation_code_update_route)
        .or(resolve_shinkai_file_protocol_route)
        .or(export_tool_route)
        .or(import_tool_route)
}

#[utoipa::path(
    get,
    path = "/v2/tool_definitions",
    params(
        ("language" = String, Query, description = "Output language (typescript or python)")
    ),
    responses(
        (status = 200, description = "tool definitions", body = String),
        (status = 400, description = "Invalid language parameter", body = APIError),
    )
)]
pub async fn tool_definitions_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    // Get language from query params, default to Language::Typescript if not provided
    let language = query_params
        .get("language")
        .and_then(|s| match s.as_str() {
            "typescript" => Some(CodeLanguage::Typescript),
            "python" => Some(CodeLanguage::Python),
            _ => None,
        });

    if language.is_none() {
        return Err(warp::reject::custom(APIError {
            code: 400,
            error: "Invalid language".to_string(),
            message: "Invalid language parameter".to_string(),
        }));
    }

    let tools: Vec<String> = query_params
        .get("tools")
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect::<Vec<String>>())
        .unwrap_or_default();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiGenerateToolDefinitions {
            bearer,
            language: language.unwrap(),
            tools,
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


#[derive(Deserialize, ToSchema, Debug)]
pub struct ToolExecutionRequest {
    pub tool_router_key: String,
    pub llm_provider: String,
    pub parameters: Value,
    #[serde(default = "default_map")]
    pub extra_config: Value
}

#[utoipa::path(
    post,
    path = "/v2/tool_execution",
    request_body = ToolExecutionRequest,
    responses(
        (status = 200, description = "Successfully executed tool", body = Value),
        (status = 400, description = "Invalid request parameters", body = APIError),
        (status = 500, description = "Tool execution failed", body = APIError)
    )
)]
pub async fn tool_execution_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    payload: ToolExecutionRequest,
) -> Result<impl warp::Reply, warp::Rejection> {    
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    // Convert parameters to a Map if it isn't already
    let parameters = match payload.parameters {
        Value::Object(map) => map,
        _ => return Err(warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Parameters".to_string(),
            message: "Parameters must be an object".to_string(),
        })),
    };

    let extra_config = match payload.extra_config {
        Value::Object(map) => map,
        _ => return Err(warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Extra Config".to_string(),
            message: "Extra Config must be an object".to_string(),
        })),
    };

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiExecuteTool {
            bearer,
            tool_router_key: payload.tool_router_key.clone(),
            parameters,
            tool_id,
            app_id,
            llm_provider: payload.llm_provider.clone(),
            extra_config,
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

#[derive(serde::Serialize, ToSchema)]
pub struct ToolImplementationResponse {
    pub code: String,
    pub metadata: ToolMetadata, // TODO: is this actually being returned?
}

#[derive(serde::Serialize, ToSchema)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Deserialize, ToSchema)]
pub struct ToolImplementationRequest {
    pub message: JobMessage,
    pub language: CodeLanguage,
    pub tools: Vec<String>,
    pub raw: Option<bool>,
    #[serde(default)]
    // Field to run a check after the tool implementation is generated
    // Default is false
    pub post_check: bool,
}

#[utoipa::path(
    post,
    path = "/v2/tool_implementation",
    request_body = ToolImplementationRequest,
    responses(
        (status = 200, description = "Tool implementation code and metadata", body = ToolImplementationResponse),
        (status = 400, description = "Invalid parameters", body = APIError),
    )
)]
pub async fn tool_implementation_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ToolImplementationRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiGenerateToolImplementation {
            bearer: authorization.strip_prefix("Bearer ").unwrap_or("").to_string(),
            message: payload.message,
            language: payload.language,
            tools: payload.tools,
            post_check: payload.post_check,
            raw: payload.raw.unwrap_or(false),
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

#[derive(Deserialize, ToSchema)]
pub struct ToolMetadataImplementationRequest {
    pub language: CodeLanguage,
    pub job_id: String,
    pub tools: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/v2/tool_metadata_implementation",
    request_body = ToolMetadataImplementationRequest,
    responses(
        (status = 200, description = "Tool metadata implementation", body = ToolImplementationResponse),
        (status = 400, description = "Invalid parameters", body = APIError),
    )
)]
pub async fn tool_metadata_implementation_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ToolMetadataImplementationRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiGenerateToolMetadataImplementation {
            bearer: authorization.strip_prefix("Bearer ").unwrap_or("").to_string(),
            language: payload.language,
            job_id: payload.job_id,
            tools: payload.tools,
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

#[utoipa::path(
    post,
    path = "/v2/set_playground_tool",
    request_body = PlaygroundTool,
    responses(
        (status = 200, description = "Successfully set playground tool", body = bool),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_playground_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ToolPlayground,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetPlaygroundTool {
            bearer,
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
    path = "/v2/list_playground_tools",
    responses(
        (status = 200, description = "Successfully listed all playground tools", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_playground_tools_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListPlaygroundTools {
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
    delete,
    path = "/v2/remove_playground_tool",
    params(
        ("tool_key" = String, Query, description = "Key of the playground tool to remove")
    ),
    responses(
        (status = 200, description = "Successfully removed playground tool", body = bool),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_playground_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let tool_key = query_params
        .get("tool_key")
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
        .send(NodeCommand::V2ApiRemovePlaygroundTool {
            bearer,
            tool_key,
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
    path = "/v2/get_playground_tool",
    params(
        ("tool_key" = String, Query, description = "Key of the playground tool to retrieve")
    ),
    responses(
        (status = 200, description = "Successfully retrieved playground tool", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_playground_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let tool_key = query_params
        .get("tool_key")
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
        .send(NodeCommand::V2ApiGetPlaygroundTool {
            bearer,
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
    get,
    path = "/v2/get_tool_implementation_prompt",
    responses(
        (status = 200, description = "Successfully retrieved tool implementation prompt", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_tool_implementation_prompt_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,

) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    
        // Get language from query params, default to Language::Typescript if not provided
        let language = query_params
        .get("language")
        .and_then(|s| match s.as_str() {
            "typescript" => Some(CodeLanguage::Typescript),
            "python" => Some(CodeLanguage::Python),
            _ => None,
        });
        
    if language.is_none() {
        return Err(warp::reject::custom(APIError {
            code: 400,
            error: "Invalid language".to_string(),
            message: "Invalid language parameter".to_string(),
        }));
    }

    let tools: Vec<String> = query_params
        .get("tools")
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect::<Vec<String>>())
        .unwrap_or_default();

    let code = query_params
        .get("code")
        .map_or("", |v| v)  
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGenerateToolFetchQuery {
            bearer,
            language: language.unwrap(),
            tools,
            code,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(prompt) => {
            let response = create_success_response(prompt);
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[derive(Deserialize, ToSchema, Debug)]
pub struct CodeExecutionRequest {
    pub tool_type: DynamicToolType,
    pub code: String,
    pub parameters: Value,
    #[serde(default = "default_map")]
    pub extra_config: Value,
    #[serde(default = "default_map")]
    pub oauth: Value,
    pub llm_provider: String,
    pub tools: Vec<String>,
}

// Define a custom default function for oauth
fn default_map() -> Value {
    Value::Object(Map::new())
}

#[utoipa::path(
    post,
    path = "/v2/code_execution",
    request_body = CodeExecutionRequest,
    responses(
        (status = 200, description = "Successfully executed code", body = Value),
        (status = 400, description = "Invalid request parameters", body = APIError),
        (status = 500, description = "Code execution failed", body = APIError)
    )
)]
pub async fn code_execution_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    payload: CodeExecutionRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    eprintln!("payload: {:?}", payload);

    // Convert parameters to a Map if it isn't already
    let parameters = match payload.parameters {
        Value::Object(map) => map,
        _ => return Err(warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Parameters".to_string(),
            message: "Parameters must be an object".to_string(),
        })),
    };

    let oauth = match payload.oauth {
        Value::Object(map) => map,
        _ => return Err(warp::reject::custom(APIError {
            code: 400,
            error: "Invalid OAuth".to_string(),
            message: "OAuth must be an object".to_string(),
        })),
    };

    let extra_config = match payload.extra_config {
        Value::Object(map) => map,
        _ => return Err(warp::reject::custom(APIError {
            code: 400,
            error: "Invalid Extra Config".to_string(),
            message: "Extra Config must be an object".to_string(),
        })),
    };

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiExecuteCode {
            bearer,
            tool_type: payload.tool_type,
            code: payload.code,
            tools: payload.tools,
            parameters,
            extra_config,
            oauth,
            tool_id: tool_id,
            app_id: app_id,
            llm_provider: payload.llm_provider,
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

#[derive(Deserialize, ToSchema)]
pub struct UndoToRequest {
    pub message_hash: String,
    pub job_id: String,
}

#[utoipa::path(
    post,
    path = "/v2/tool_implementation_undo_to",
    request_body = UndoToRequest,
    responses(
        (status = 200, description = "Successfully undone to specified state", body = Value),
        (status = 400, description = "Invalid request parameters", body = APIError),
        (status = 500, description = "Undo operation failed", body = APIError)
    )
)]
pub async fn undo_to_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: UndoToRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiToolImplementationUndoTo {
            bearer,
            message_hash: payload.message_hash,
            job_id: payload.job_id,
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

#[derive(Deserialize, ToSchema)]
pub struct ToolImplementationCodeUpdateRequest {
    pub job_id: String,
    pub code: String,
}

#[utoipa::path(
    post,
    path = "/v2/tool_implementation_code_update",
    request_body = ToolImplementationCodeUpdateRequest,
    responses(
        (status = 200, description = "Successfully updated tool implementation code", body = Value),
        (status = 400, description = "Invalid request parameters", body = APIError),
        (status = 500, description = "Code update failed", body = APIError)
    )
)]
pub async fn tool_implementation_code_update_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ToolImplementationCodeUpdateRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiToolImplementationCodeUpdate {
            bearer,
            job_id: payload.job_id,
            code: payload.code,
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
    path = "/v2/export_tool",
    params(
        ("tool_key_path" = String, Query, description = "Tool key path")
    ),
    responses(
        (status = 200, description = "Exported tool", body = Vec<u8>),
        (status = 400, description = "Invalid tool key path", body = APIError),
    )
)]
pub async fn export_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let tool_key_path = query_params
        .get("tool_key_path")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid tool key path".to_string(),
                message: "Tool key path is required".to_string(),
            })
        })?
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiExportTool {
            bearer,
            tool_key_path,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(file_bytes) => {
            // Return the raw bytes with appropriate headers
            Ok(warp::reply::with_header(
                warp::reply::with_status(file_bytes, StatusCode::OK),
                "Content-Type",
                "application/octet-stream",
            ))
        }
        Err(error) => Ok(warp::reply::with_header(
            warp::reply::with_status(
                error.message.as_bytes().to_vec(),
                StatusCode::from_u16(error.code).unwrap()
            ),
            "Content-Type",
            "text/plain",
        ))
    }
}

#[derive(Deserialize, ToSchema)]
pub struct ImportToolRequest {
    pub url: String,
}

#[utoipa::path(
    post,
    path = "/v2/import_tool",
    request_body = ImportToolRequest,
    responses(
        (status = 200, description = "Imported tool", body = Value),
        (status = 400, description = "Invalid URL", body = APIError),
    )
)]
pub async fn import_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ImportToolRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let url = payload.url;

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiImportTool {
            bearer,
            url,
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
    path = "/v2/resolve_shinkai_file_protocol",
    params(
        ("file" = String, Query, description = "Shinkai file protocol")
    ),
    responses(
        (status = 200, description = "Resolved shinkai file protocol", body = Vec<u8>),
        (status = 400, description = "Invalid shinkai file protocol", body = APIError),
    )
)]
pub async fn resolve_shinkai_file_protocol_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let shinkai_file_protocol = query_params
        .get("file")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid shinkai file protocol".to_string(),
                message: "Shinkai file protocol is required".to_string(),
            })
        })?
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiResolveShinkaiFileProtocol {
            bearer,
            shinkai_file_protocol,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(file_bytes) => {
            // Return the raw bytes with appropriate headers
            Ok(warp::reply::with_header(
                warp::reply::with_status(file_bytes, StatusCode::OK),
                "Content-Type",
                "application/octet-stream",
            ))
        }
        Err(error) => Ok(warp::reply::with_header(
            warp::reply::with_status(
                error.message.as_bytes().to_vec(),
                StatusCode::from_u16(error.code).unwrap()
            ),
            "Content-Type",
            "text/plain",
        ))
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        tool_execution_handler,
        tool_definitions_handler,
        tool_implementation_handler,
        tool_metadata_implementation_handler,
        list_all_shinkai_tools_handler,
        set_shinkai_tool_handler,
        get_shinkai_tool_handler,
        search_shinkai_tool_handler,
        add_shinkai_tool_handler,
        set_playground_tool_handler,
        list_playground_tools_handler,
        remove_playground_tool_handler,
        get_playground_tool_handler,
        get_tool_implementation_prompt_handler,
        code_execution_handler,
        undo_to_handler,
    ),
    components(
        schemas(
            APIError, 
            ToolExecutionRequest,
        )
    ),
    tags(
        (name = "tools", description = "Tool API endpoints")
    )
)]
pub struct ToolsApiDoc;
