use async_channel::Sender;
use serde::Deserialize;
use serde_json::Value;
use shinkai_message_primitives::{shinkai_message::shinkai_message_schemas::JobCreationInfo, shinkai_utils::job_scope::JobScope};
use utoipa::{OpenApi, ToSchema};
use warp::Filter;
use reqwest::StatusCode;
use std::collections::HashMap;

use crate::{node_api_router::APIError, node_commands::NodeCommand};
use super::{api_v2_handlers_jobs::CreateJobRequest, api_v2_router::{create_success_response, with_sender}};

#[derive(Deserialize, ToSchema, Clone)]
pub enum Language {
    Typescript,
    Python,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Typescript => write!(f, "typescript"),
            Language::Python => write!(f, "python"),
        }
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    Local,
    JS,
    Deno,
    DenoDynamic,
    Python,
    PythonDynamic,
    Rust,
    Network,
    Internal,
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolType::JS => write!(f, "js"),
            ToolType::Local => write!(f, "local"),
            ToolType::Deno => write!(f, "deno"),
            ToolType::DenoDynamic => write!(f, "deno_dynamic"),
            ToolType::Python => write!(f, "python"),
            ToolType::PythonDynamic => write!(f, "python_dynamic"),
            ToolType::Rust => write!(f, "rust"), 
            ToolType::Network => write!(f, "network"),
            ToolType::Internal => write!(f, "internal"),
        }
    }
}

pub fn tool_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let tool_execution_route = warp::path("tool_execution")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
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

    tool_execution_route
        .or(tool_definitions_route)
        .or(tool_implementation_route)
        .or(tool_metadata_implementation_route)
}

#[utoipa::path(
    get,
    path = "/v2/tool_definitions",
    params(
        ("language" = Option<String>, Query, description = "Output language (typescript or python)")
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
            "typescript" => Some(Language::Typescript),
            "python" => Some(Language::Python),
            _ => None,
        })
        .unwrap_or(Language::Typescript); // Default to Typescript

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::GenerateToolDefinitions {
            bearer,
            language,
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
pub struct ToolExecutionRequest {
    pub tool_type: ToolType,
    pub tool_router_key: String,
    pub parameters: Value,
    #[serde(default)]
    pub extra_config: Option<String>,
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

    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::ExecuteCommand {
            bearer,
            tool_router_key: payload.tool_router_key.clone(),
            tool_type: payload.tool_type,
            parameters,
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
    pub metadata: ToolMetadata,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Deserialize, ToSchema)]
pub struct ToolImplementationRequest {
    pub language: Option<Language>,
    pub prompt: Option<String>,
    pub raw: Option<bool>,
    pub code: Option<String>,
    pub metadata: Option<String>,
    pub output: Option<String>,
    pub llm_provider: Option<String>,
    pub fetch_query: Option<bool>,
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
        .send(NodeCommand::GenerateToolImplementation {
            bearer: authorization.strip_prefix("Bearer ").unwrap_or("").to_string(),
            language: payload.language.unwrap_or(Language::Typescript),
            prompt: payload.prompt.unwrap_or("".to_string()),
            code: payload.code,
            metadata: payload.metadata,
            output: payload.output,
            job_creation_info: JobCreationInfo {
                scope: JobScope::new_default(),
                is_hidden: Some(false),
                associated_ui: None,
            },
            llm_provider: payload.llm_provider.unwrap_or("llama3_1_8b".to_string()),
            raw: payload.raw.unwrap_or(false),
            fetch_query: payload.fetch_query.unwrap_or(false),
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
    path = "/v2/tool_metadata_implementation",
    request_body = ToolImplementationRequest,
    responses(
        (status = 200, description = "Tool metadata implementation", body = ToolImplementationResponse),
        (status = 400, description = "Invalid parameters", body = APIError),
    )
)]
pub async fn tool_metadata_implementation_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: ToolImplementationRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::GenerateToolMetadataImplementation {
            bearer: authorization.strip_prefix("Bearer ").unwrap_or("").to_string(),
            language: payload.language.unwrap_or(Language::Typescript),
            code: payload.code,
            metadata: payload.metadata,
            output: payload.output,
            job_creation_info: JobCreationInfo {
                scope: JobScope::new_default(),
                is_hidden: Some(false),
                associated_ui: None,
            },
            llm_provider: payload.llm_provider.unwrap_or("llama3_1_8b".to_string()),
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
