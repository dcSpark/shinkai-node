use async_channel::Sender;
use serde::Deserialize;
use serde_json::{Map, Value};
use shinkai_message_primitives::{schemas::{shinkai_tools::{CodeLanguage, DynamicToolType}, tool_router_key::ToolRouterKey}, shinkai_message::shinkai_message_schemas::JobMessage};
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolWithAssets, tool_config::OAuth, tool_playground::ToolPlayground, tool_types::{OperatingSystem, RunnerType}};
use utoipa::{OpenApi, ToSchema};
use warp::Filter;
use reqwest::StatusCode;
use std::collections::HashMap;
use futures::TryStreamExt;
use warp::multipart::FormData;
use bytes::Buf;

use crate::{node_api_router::APIError, node_commands::NodeCommand};
use super::api_v2_router::{create_success_response, with_sender};

pub fn tool_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let list_all_shinkai_tools_route = warp::path("list_all_shinkai_tools")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(list_all_shinkai_tools_handler);

    let set_shinkai_tool_route = warp::path("set_shinkai_tool")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::body::json())
        .and_then(set_shinkai_tool_handler);

    let enable_all_tools_route = warp::path("enable_all_tools")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(enable_all_tools_handler);

    let disable_all_tools_route = warp::path("disable_all_tools")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(disable_all_tools_handler);

    let duplicate_tool_route = warp::path("duplicate_tool")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(duplicate_tool_handler);

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
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::header::optional::<String>("x-shinkai-original-tool-router-key"))
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

    // Resolves shinkai://file URLs to actual file bytes, providing secure access to files in the node's storage
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

    let publish_tool_route = warp::path("publish_tool")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(publish_tool_handler);

    let import_tool_route = warp::path("import_tool")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(import_tool_handler);

    let import_tool_zip_route = warp::path("import_tool_zip")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::multipart::form().max_length(50 * 1024 * 1024))
        .and_then(import_tool_zip_handler);

    let tool_asset_route = warp::path("tool_asset")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::multipart::form().max_length(50 * 1024 * 1024))
        .and_then(tool_asset_handler);

    let list_tool_asset_route = warp::path("list_tool_asset")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and_then(list_tool_asset_handler);

    let delete_tool_asset_route = warp::path("tool_asset")
        .and(warp::delete())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(delete_tool_asset_handler);

    let remove_tool_route = warp::path("remove_tool")
        .and(warp::delete())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(remove_tool_handler);

    let tool_store_proxy_route = warp::path("tool_store_proxy")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::path::param::<String>())
        .and_then(tool_store_proxy_handler);

    let standalone_playground_route = warp::path("tools_standalone_playground")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::header::<String>("x-shinkai-tool-id"))
        .and(warp::header::<String>("x-shinkai-app-id"))
        .and(warp::header::<String>("x-shinkai-llm-provider"))
        .and(warp::body::json())
        .and_then(standalone_playground_handler);

    let list_all_shinkai_tools_versions_route = warp::path("list_all_shinkai_tools_versions")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(list_all_shinkai_tools_versions_handler);

    let set_tool_enabled_route = warp::path("set_tool_enabled")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(set_tool_enabled_handler);

    let copy_tool_assets = warp::path!("copy_tool_assets")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(copy_tool_assets_handler);

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
        .or(duplicate_tool_route)
        .or(set_playground_tool_route)
        .or(list_playground_tools_route)
        .or(remove_playground_tool_route)
        .or(get_playground_tool_route)
        .or(get_tool_implementation_prompt_route)
        .or(undo_to_route)
        .or(tool_implementation_code_update_route)
        .or(resolve_shinkai_file_protocol_route)
        .or(export_tool_route)
        .or(publish_tool_route)
        .or(import_tool_route)
        .or(import_tool_zip_route)
        .or(tool_asset_route)
        .or(list_tool_asset_route)
        .or(delete_tool_asset_route)
        .or(remove_tool_route)
        .or(enable_all_tools_route)
        .or(disable_all_tools_route)
        .or(tool_store_proxy_route)
        .or(standalone_playground_route)
        .or(list_all_shinkai_tools_versions_route)
        .or(set_tool_enabled_route)
        .or(copy_tool_assets)
}

pub fn safe_folder_name(tool_router_key: &str) -> String {
    tool_router_key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
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
            tools: tools.iter().filter_map(|t| ToolRouterKey::from_string(t).ok ()).collect(),
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
    pub extra_config: Value,
    pub mounts: Option<Vec<String>>,
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
            tool_id: safe_folder_name(&tool_id),
            app_id: safe_folder_name(&app_id),
            llm_provider: payload.llm_provider.clone(),
            extra_config,
            mounts: payload.mounts,
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

#[derive(Deserialize, ToSchema, Debug)]
pub struct ToolImplementationRequest {
    pub message: JobMessage,
    pub language: CodeLanguage,
    pub tools: Vec<ToolRouterKey>,
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
    #[serde(deserialize_with = "deserialize_tool_router_keys")]
    pub tools: Vec<ToolRouterKey>,
}

fn deserialize_tool_router_keys<'de, D>(deserializer: D) -> Result<Vec<ToolRouterKey>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let string_vec: Vec<String> = Vec::deserialize(deserializer)?;
    string_vec
        .iter()
        .map(|s| ToolRouterKey::from_string(s).map_err(serde::de::Error::custom))
        .collect()
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
        ("query" = String, Query, description = "Search query for Shinkai tools"),
        ("agent_or_llm" = Option<String>, Query, description = "Optional agent or LLM identifier")
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
    
    // Get the optional agent_or_llm parameter
    let agent_or_llm = query_params.get("agent_or_llm").cloned();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSearchShinkaiTool {
            bearer,
            query,
            agent_or_llm,
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
    params(
        ("category" = Option<String>, Query, description = "Optional category filter for tools. Use 'download' to only list tools from external sources.")
    ),
    responses(
        (status = 200, description = "Successfully listed all Shinkai tools", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_all_shinkai_tools_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let category = query_params.get("category").cloned();
    
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListAllShinkaiTools {
            bearer,
            category,
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
    payload: ShinkaiToolWithAssets,
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
    tool_id: String,
    app_id: String,
    original_tool_key_path: Option<String>,
    payload: ToolPlayground,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    if let Some(original_tool_key_path) = original_tool_key_path.clone() {
        if original_tool_key_path.split(":::").collect::<Vec<&str>>().len() != 4 {
            println!("Invalid original_tool_key_path: {}", original_tool_key_path);
        }
    }
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiSetPlaygroundTool {
            bearer,
            payload, 
            tool_id: safe_folder_name(&tool_id),
            app_id: safe_folder_name(&app_id),
            original_tool_key_path,
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

    let tools: Vec<ToolRouterKey> = query_params
        .get("tools")
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect::<Vec<String>>())
        .unwrap_or_default()
        .iter()
        .filter_map(|t| ToolRouterKey::from_string(t).ok())
        .collect();

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
    pub oauth: Option<Vec<OAuth>>,
    pub llm_provider: String,
    #[serde(deserialize_with = "deserialize_tool_router_keys")]
    pub tools: Vec<ToolRouterKey>,
    pub mounts: Option<Vec<String>>,
    pub runner: Option<RunnerType>,
    pub operating_system: Option<Vec<OperatingSystem>>,
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
        .send(NodeCommand::V2ApiExecuteCode {
            bearer,
            tool_type: payload.tool_type,
            code: payload.code,
            tools: payload.tools,
            parameters,
            extra_config,
            oauth: payload.oauth,
            tool_id: safe_folder_name(&tool_id),
            app_id: safe_folder_name(&app_id),
            llm_provider: payload.llm_provider,
            mounts: payload.mounts,
            runner: payload.runner,
            operating_system: payload.operating_system,
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

#[utoipa::path(
    get,
    path = "/v2/publish_tool",
    params(
        ("tool_key_path" = String, Query, description = "Tool key path"),
    ),
    responses(
        (status = 200, description = "Exported tool", body = Vec<u8>),
        (status = 400, description = "Invalid tool key path", body = APIError),
    )
)]
pub async fn publish_tool_handler(
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
        .send(NodeCommand::V2ApiPublishTool {
            bearer,
            tool_key_path,
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
    post,
    path = "/v2/import_tool_zip",
    responses(
        (status = 200, description = "Successfully imported tool from zip", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn import_tool_zip_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let mut file_data: Option<Vec<u8>> = None;

    // Add error handling for form parsing
    while let Ok(Some(part)) = form.try_next().await {
        if part.name() == "file" {
            // Read file data with error handling
            let mut bytes = Vec::new();
            let mut stream = part.stream();
            
            while let Ok(Some(chunk)) = stream.try_next().await {
                if bytes.len() + chunk.chunk().len() > 50 * 1024 * 1024 {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&APIError {
                            code: 400,
                            error: "File too large".to_string(),
                            message: "File size exceeds 50MB limit".to_string(),
                        }),
                        StatusCode::BAD_REQUEST,
                    ));
                }
                bytes.extend_from_slice(chunk.chunk());
            }
            
            if bytes.is_empty() {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&APIError {
                        code: 400,
                        error: "Empty file".to_string(),
                        message: "The uploaded file is empty".to_string(),
                    }),
                    StatusCode::BAD_REQUEST,
                ));
            }
            
            file_data = Some(bytes);
        }
    }

    // Validate we have the file data
    let file_data = match file_data {
        Some(data) => data,
        None => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&APIError {
                    code: 400,
                    error: "Missing file".to_string(),
                    message: "Zip file data is required".to_string(),
                }),
                StatusCode::BAD_REQUEST,
            ))
        }
    };

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    match sender
        .send(NodeCommand::V2ApiImportToolZip {
            bearer,
            file_data,
            res: res_sender,
        })
        .await {
            Ok(_) => (),
            Err(_) => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&APIError {
                        code: 500,
                        error: "Internal server error".to_string(),
                        message: "Failed to process the request".to_string(),
                    }),
                    StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        };

    let result = match res_receiver.recv().await {
        Ok(result) => result,
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&APIError {
                    code: 500,
                    error: "Internal server error".to_string(),
                    message: "Failed to receive response from server".to_string(),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    };

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

#[utoipa::path(
    delete,
    path = "/v2/remove_tool",
    params(
        ("tool_key" = String, Query, description = "Key of the tool to remove")
    ),
    responses(
        (status = 200, description = "Successfully removed tool", body = bool),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_tool_handler(
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
        .send(NodeCommand::V2ApiRemoveTool {
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
    post,
    path = "/v2/tool_asset",
    responses(
        (status = 200, description = "Successfully uploaded tool asset", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn tool_asset_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let mut file_name = String::new();
    let mut file_data: Option<Vec<u8>> = None;

    while let Ok(Some(part)) = form.try_next().await {
        match part.name() {
            "file_name" => {
                // Convert the part to bytes then to string
                let mut bytes = Vec::new();
                let mut stream = part.stream();
                while let Ok(Some(chunk)) = stream.try_next().await {
                    bytes.extend_from_slice(chunk.chunk());
                }
                file_name = String::from_utf8_lossy(&bytes).into_owned();
            }
            "file" => {
                // Read file data
                let mut bytes = Vec::new();
                let mut stream = part.stream();
                while let Ok(Some(chunk)) = stream.try_next().await {
                    bytes.extend_from_slice(chunk.chunk());
                }
                file_data = Some(bytes);
            }
            _ => {}
        }
    }

    // Validate we have both file name and data
    let file_data = match file_data {
        Some(data) => data,
        None => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&APIError {
                    code: 400,
                    error: "Missing file".to_string(),
                    message: "File data is required".to_string(),
                }),
                StatusCode::BAD_REQUEST,
            ))
        }
    };

    if file_name.is_empty() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&APIError {
                code: 400,
                error: "Missing file name".to_string(),
                message: "File name is required".to_string(),
            }),
            StatusCode::BAD_REQUEST,
        ));
    }

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiUploadToolAsset {
            bearer,
            tool_id: safe_folder_name(&tool_id),
            app_id: safe_folder_name(&app_id),
            file_name,
            file_data,
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
    path = "/v2/list_tool_asset",
    responses(
        (status = 200, description = "Successfully listed tool assets", body = Vec<String>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_tool_asset_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiListToolAssets {
            bearer,
            tool_id: safe_folder_name(&tool_id),
            app_id: safe_folder_name(&app_id),
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
    path = "/v2/tool_asset",
    params(
        ("file_name" = String, Query, description = "Name of the file to delete")
    ),
    responses(
        (status = 200, description = "Successfully deleted tool asset", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn delete_tool_asset_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let file_name = query_params
        .get("file_name")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Missing file name".to_string(),
                message: "File name is required".to_string(),
            })
        })?
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiDeleteToolAsset {
            bearer,
            tool_id: safe_folder_name(&tool_id),
            app_id: safe_folder_name(&app_id),
            file_name,
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
    path = "/v2/enable_all_tools",
    responses(
        (status = 200, description = "Successfully enabled all available tools", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn enable_all_tools_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::V2ApiEnableAllTools {
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
    path = "/v2/disable_all_tools",
    responses(
        (status = 200, description = "Successfully disabled all tools", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn disable_all_tools_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::V2ApiDisableAllTools {
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
    path = "/v2/duplicate_tool",
    responses(
        (status = 200, description = "Successfully duplicated tool", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]

pub async fn duplicate_tool_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    let tool_key_path = query_params.get("tool_key_path").unwrap_or(&String::new()).to_string();
    if tool_key_path.is_empty() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&APIError {
                code: 400,
                error: "Missing tool key path".to_string(),
                message: "Tool key path is required".to_string(),
            }),
            StatusCode::BAD_REQUEST,
        ));
    }
    sender
        .send(NodeCommand::V2ApiDuplicateTool { bearer, tool_key_path, res: res_sender })
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
    path = "/v2/tool_store_proxy/{tool_router_key}",
    params(
        ("tool_router_key" = String, Path, description = "Tool router key")
    ),
    responses(
        (status = 200, description = "Successfully retrieved store data", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]

pub async fn tool_store_proxy_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_router_key: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiStoreProxy {
            bearer,
            tool_router_key,
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

#[derive(Deserialize, ToSchema)]
pub struct StandAlonePlaygroundRequest {
    pub language: CodeLanguage,
    pub code: Option<String>,
    pub metadata: Option<Value>,
    pub assets: Option<Vec<String>>,
    pub tools: Option<Vec<ToolRouterKey>>,
    pub parameters: Option<Value>,
    pub config: Option<Value>,
    pub oauth: Option<Vec<OAuth>>,
}

#[utoipa::path(
    post,
    path = "/v2/tools_standalone_playground",
    request_body = StandAlonePlaygroundRequest,
    responses(
        (status = 200, description = "Successfully created standalone playground", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]

pub async fn standalone_playground_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    payload: StandAlonePlaygroundRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiStandAlonePlayground {
            bearer,
            code: payload.code,
            metadata: payload.metadata,
            assets: payload.assets,
            language: payload.language,
            tools: payload.tools,
            parameters: payload.parameters,
            config: payload.config,
            oauth: payload.oauth,
            tool_id: safe_folder_name(&tool_id),
            app_id: safe_folder_name(&app_id),
            llm_provider: llm_provider,
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
    path = "/v2/list_all_shinkai_tools_versions",
    responses(
        (status = 200, description = "Successfully listed all Shinkai tools with versions", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_all_shinkai_tools_versions_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListAllShinkaiToolsVersions {
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

#[derive(Deserialize, ToSchema)]
pub struct SetToolEnabledRequest {
    pub tool_router_key: String,
    pub enabled: bool,
}

#[utoipa::path(
    post,
    path = "/v2/set_tool_enabled",
    request_body = SetToolEnabledRequest,
    responses(
        (status = 200, description = "Successfully enabled/disabled tool", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn set_tool_enabled_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: SetToolEnabledRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiSetToolEnabled {
            bearer,
            tool_router_key: payload.tool_router_key,
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


#[derive(Debug, Deserialize)]
pub struct CopyToolAssetsRequest {
    pub is_first_playground: bool,
    pub first_path: String,  // app_id for playground or tool_key_path for tool
    pub is_second_playground: bool,
    pub second_path: String, // app_id for playground or tool_key_path for tool
}

#[utoipa::path(
    post,
    path = "/v2/copy_tool_assets",
    request_body = CopyToolAssetsRequest,
    responses(
        (status = 200, description = "Successfully copied tool assets", body = bool),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn copy_tool_assets_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: CopyToolAssetsRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiCopyToolAssets {
            bearer,
            is_first_playground: payload.is_first_playground,
            first_path: payload.first_path,
            is_second_playground: payload.is_second_playground,
            second_path: payload.second_path,
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
        remove_tool_handler,
        export_tool_handler,
        import_tool_handler,
        import_tool_zip_handler,
        resolve_shinkai_file_protocol_handler,
        tool_asset_handler,
        list_tool_asset_handler,
        delete_tool_asset_handler,
        enable_all_tools_handler,
        disable_all_tools_handler,
        tool_store_proxy_handler,
        standalone_playground_handler,
        set_tool_enabled_handler,
    ),
    components(
        schemas(
            APIError, 
            ToolExecutionRequest,
            SetToolEnabledRequest,
        )
    ),
    tags(
        (name = "tools", description = "Tool API endpoints")
    )
)]

pub struct ToolsApiDoc;