use async_channel::Sender;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use shinkai_message_primitives::schemas::llm_providers::agent::Agent;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    Exo, Gemini, Groq, LLMProviderInterface, LocalLLM, Ollama, OpenAI, ShinkaiBackend,
};
use shinkai_message_primitives::schemas::shinkai_name::{ShinkaiName, ShinkaiSubidentityType};
use shinkai_message_primitives::shinkai_message::shinkai_message::{
    EncryptedShinkaiBody, EncryptedShinkaiData, ExternalMetadata, InternalMetadata, MessageBody, MessageData, NodeApiData, ShinkaiBody, ShinkaiData, ShinkaiMessage, ShinkaiVersion
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::{
    schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider,
    shinkai_message::shinkai_message_schemas::APIAddOllamaModels,
};
use utoipa::{OpenApi, ToSchema};
use warp::Filter;
use std::collections::HashMap;

use crate::api_v1::api_v1_handlers::APIUseRegistrationCodeSuccessResponse;
use crate::{
    node_api_router::{APIError, GetPublicKeysResponse},
    node_commands::NodeCommand,
};

use super::api_v2_router::{create_success_response, with_node_name, with_sender};

pub fn general_routes(
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let public_keys_route = warp::path("public_keys")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and_then(get_public_keys);

    let health_check_route = warp::path("health_check")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(with_node_name(node_name.clone()))
        .and_then(health_check);

    let initial_registration_route = warp::path("initial_registration")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::body::json())
        .and_then(initial_registration_handler);

    let get_default_embedding_model_route = warp::path("default_embedding_model")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_default_embedding_model_handler);

    let get_supported_embedding_models_route = warp::path("supported_embedding_models")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_supported_embedding_models_handler);

    let update_default_embedding_model_route = warp::path("default_embedding_model")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_default_embedding_model_handler);

    let update_supported_embedding_models_route = warp::path("supported_embedding_models")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_supported_embedding_models_handler);

    let add_llm_provider_route = warp::path("add_llm_provider")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_llm_provider_handler);

    let remove_llm_provider_route = warp::path("remove_llm_provider")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(remove_llm_provider_handler);

    let modify_llm_provider_route = warp::path("modify_llm_provider")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(modify_llm_provider_handler);

    let change_node_name_route = warp::path("change_node_name")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(change_node_name_handler);

    let is_pristine_route = warp::path("is_pristine")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(is_pristine_handler);

    let scan_ollama_models_route = warp::path("scan_ollama_models")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(scan_ollama_models_handler);

    let add_ollama_models_route = warp::path("add_ollama_models")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_ollama_models_handler);

    let download_file_from_inbox_route = warp::path("download_file_from_inbox")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and_then(download_file_from_inbox_handler);

    let list_files_in_inbox_route = warp::path("list_files_in_inbox")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::path::param::<String>())
        .and_then(list_files_in_inbox_handler);

    let stop_llm_route = warp::path("stop_llm")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(stop_llm_handler);

    let add_agent_route = warp::path("add_agent")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_agent_handler);

    let remove_agent_route = warp::path("remove_agent")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(remove_agent_handler);

    let update_agent_route = warp::path("update_agent")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_agent_handler);

    let get_agent_route = warp::path("get_agent")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::path::param::<String>())
        .and_then(get_agent_handler);

    let get_all_agents_route = warp::path("get_all_agents")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_all_agents_handler);

    let export_agent_route = warp::path("export_agent")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(export_agent_handler);

    let import_agent_route = warp::path("import_agent")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(import_agent_handler);

    let test_llm_provider_route = warp::path("test_llm_provider")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(test_llm_provider_handler);

    public_keys_route
        .or(health_check_route)
        .or(initial_registration_route)
        .or(get_default_embedding_model_route)
        .or(get_supported_embedding_models_route)
        .or(update_default_embedding_model_route)
        .or(update_supported_embedding_models_route)
        .or(add_llm_provider_route)
        .or(remove_llm_provider_route)
        .or(modify_llm_provider_route)
        .or(change_node_name_route)
        .or(is_pristine_route)
        .or(scan_ollama_models_route)
        .or(add_ollama_models_route)
        .or(download_file_from_inbox_route)
        .or(list_files_in_inbox_route)
        .or(stop_llm_route)
        .or(add_agent_route)
        .or(remove_agent_route)
        .or(update_agent_route)
        .or(get_agent_route)
        .or(get_all_agents_route)
        .or(export_agent_route)
        .or(import_agent_route)        
        .or(test_llm_provider_route)
}

#[derive(Deserialize)]
pub struct InitialRegistrationRequest {
    pub profile_encryption_pk: String,
    pub profile_identity_pk: String,
}

#[utoipa::path(
    get,
    path = "/v2/download_file_from_inbox/{inbox_name}/{filename}",
    responses(
        (status = 200, description = "Successfully downloaded file", body = Vec<u8>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn download_file_from_inbox_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    inbox_name: String,
    filename: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiDownloadFileFromInbox {
            bearer,
            inbox_name,
            filename,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(file_data) => Ok(warp::reply::with_header(
            file_data,
            "Content-Type",
            "application/octet-stream",
        )),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/list_files_in_inbox/{inbox_name}",
    responses(
        (status = 200, description = "Successfully listed files in inbox", body = Vec<String>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_files_in_inbox_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    inbox_name: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListFilesInInbox {
            bearer,
            inbox_name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(file_list) => Ok(warp::reply::json(&file_list)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/public_keys",
    responses(
        (status = 200, description = "Successfully retrieved public keys", body = GetPublicKeysResponse),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_public_keys(sender: Sender<NodeCommand>) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetPublicKeys { res: res_sender })
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
    path = "/v2/health_check",
    responses(
        (status = 200, description = "Health check successful", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn health_check(sender: Sender<NodeCommand>, node_name: String) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);

     // Send the APIHealthCheck command to retrieve the pristine state and public HTTPS certificate
    sender
        .send(NodeCommand::V2ApiHealthCheck { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let health_check_state = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    if let Err(error) = health_check_state {
        return Ok(warp::reply::json(&json!({ "status": "error", "error": error })));
    }

    let mut response = json!({ "status": "ok", "node_name": node_name });
    if let Ok(state) = health_check_state {
        if let serde_json::Value::Object(ref mut map) = response {
            if let serde_json::Value::Object(state_map) = state {
                map.extend(state_map);
            }
        }
    }

    Ok(warp::reply::json(&response))
}

#[utoipa::path(
    post,
    path = "/v2/initial_registration",
    request_body = ShinkaiMessage,
    responses(
        (status = 200, description = "Successfully used registration code", body = APIUseRegistrationCodeSuccessResponse),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn initial_registration_handler(
    node_commands_sender: Sender<NodeCommand>,
    payload: InitialRegistrationRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiInitialRegistration {
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
    path = "/v2/default_embedding_model",
    responses(
        (status = 200, description = "Successfully retrieved default embedding model", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_default_embedding_model_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetDefaultEmbeddingModel {
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
    path = "/v2/supported_embedding_models",
    responses(
        (status = 200, description = "Successfully retrieved supported embedding models", body = Vec<String>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_supported_embedding_models_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetSupportedEmbeddingModels {
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
    path = "/v2/default_embedding_model",
    request_body = String,
    responses(
        (status = 200, description = "Successfully updated default embedding model", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_default_embedding_model_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    model_name: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateDefaultEmbeddingModel {
            bearer,
            model_name,
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
    path = "/v2/supported_embedding_models",
    request_body = Vec<String>,
    responses(
        (status = 200, description = "Successfully updated supported embedding models", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_supported_embedding_models_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    models: Vec<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateSupportedEmbeddingModels {
            bearer,
            models,
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
    path = "/v2/add_llm_provider",
    request_body = SerializedLLMProvider,
    responses(
        (status = 200, description = "Successfully added LLM provider", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_llm_provider_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    agent: SerializedLLMProvider,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiAddLlmProvider {
            bearer,
            agent,
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
    path = "/v2/remove_llm_provider",
    request_body = HashMap<String, String>,
    responses(
        (status = 200, description = "Successfully removed LLM provider", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_llm_provider_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let llm_provider_id = payload.get("llm_provider_id").cloned().unwrap_or_default();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiRemoveLlmProvider {
            bearer,
            llm_provider_id,
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
    path = "/v2/modify_llm_provider",
    request_body = SerializedLLMProvider,
    responses(
        (status = 200, description = "Successfully modified LLM provider", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn modify_llm_provider_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    agent: SerializedLLMProvider,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiModifyLlmProvider {
            bearer,
            agent,
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
    path = "/v2/change_node_name",
    request_body = String,
    responses(
        (status = 200, description = "Successfully changed node name", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn change_node_name_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    new_name: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiChangeNodesName {
            bearer,
            new_name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(_) => Ok(warp::reply::json(&json!({"status": "success"}))),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/is_pristine",
    responses(
        (status = 200, description = "Successfully checked pristine state", body = bool),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn is_pristine_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiIsPristine {
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
    path = "/v2/scan_ollama_models",
    responses(
        (status = 200, description = "Successfully scanned Ollama models", body = Vec<serde_json::Value>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn scan_ollama_models_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiScanOllamaModels {
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
    path = "/v2/add_ollama_models",
    request_body = APIAddOllamaModels,
    responses(
        (status = 200, description = "Successfully added Ollama models", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_ollama_models_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIAddOllamaModels,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiAddOllamaModels {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(_) => Ok(warp::reply::json(&json!({"status": "success"}))),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[derive(Deserialize, ToSchema)]
pub struct StopLLMRequest {
    pub inbox_name: String,
}

#[utoipa::path(
    post,
    path = "/v2/stop_llm",
    request_body = StopLLMRequest,
    responses(
        (status = 200, description = "Successfully stopped LLM", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn stop_llm_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: StopLLMRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiStopLLM {
            bearer,
            inbox_name: payload.inbox_name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(_) => Ok(warp::reply::json(&json!({"status": "success"}))),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    post,
    path = "/v2/add_agent",
    request_body = Agent,
    responses(
        (status = 200, description = "Successfully added agent", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_agent_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    agent: Agent,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiAddAgent {
            bearer,
            agent,
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
    path = "/v2/remove_agent",
    request_body = HashMap<String, String>,
    responses(
        (status = 200, description = "Successfully removed agent", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_agent_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let agent_id = payload.get("agent_id").cloned().unwrap_or_default();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiRemoveAgent {
            bearer,
            agent_id,
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
    path = "/v2/update_agent",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Successfully updated agent", body = Agent),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_agent_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    update_data: serde_json::Value,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateAgent {
            bearer,
            partial_agent: update_data,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(updated_agent) => Ok(warp::reply::json(&updated_agent)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/get_agent/{agent_id}",
    responses(
        (status = 200, description = "Successfully retrieved agent", body = Agent),
        (status = 404, description = "Agent not found", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_agent_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    agent_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetAgent {
            bearer,
            agent_id,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(agent) => Ok(warp::reply::json(&agent)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/get_all_agents",
    responses(
        (status = 200, description = "Successfully retrieved all agents", body = Vec<Agent>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_all_agents_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetAllAgents {
            bearer,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(agents) => Ok(warp::reply::json(&agents)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/export_agent",
    params(
        ("agent_id" = String, Query, description = "Agent identifier")
    ),
    responses(
        (status = 200, description = "Exported agent", body = Vec<u8>),
        (status = 400, description = "Invalid agent identifier", body = APIError),
    )
)]
pub async fn export_agent_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query_params: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();

    let agent_id = query_params
        .get("agent_id")
        .ok_or_else(|| {
            warp::reject::custom(APIError {
                code: 400,
                error: "Invalid agent identifier".to_string(),
                message: "Agent identifier is required".to_string(),
            })
        })?
        .to_string();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiExportAgent {
            bearer,
            agent_id,
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
    post,
    path = "/v2/import_agent",
    request_body = HashMap<String, String>,
    responses(
        (status = 200, description = "Successfully imported agent", body = Value),
        (status = 400, description = "Invalid URL or agent data", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn import_agent_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: HashMap<String, String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let url = payload.get("url").cloned().unwrap_or_default();
    
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiImportAgent {
            bearer,
            url,
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
    path = "/v2/test_llm_provider",
    request_body = SerializedLLMProvider,
    responses(
        (status = 200, description = "Successfully tested LLM provider", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn test_llm_provider_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    provider: SerializedLLMProvider,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiTestLlmProvider {
            bearer,
            provider,
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
        download_file_from_inbox_handler,
        list_files_in_inbox_handler,
        get_public_keys,
        health_check,
        initial_registration_handler,
        get_default_embedding_model_handler,
        get_supported_embedding_models_handler,
        update_default_embedding_model_handler,
        update_supported_embedding_models_handler,
        add_llm_provider_handler,
        remove_llm_provider_handler,
        modify_llm_provider_handler,
        change_node_name_handler,
        is_pristine_handler,
        scan_ollama_models_handler,
        add_ollama_models_handler,
        stop_llm_handler,
        add_agent_handler,
        remove_agent_handler,
        update_agent_handler,
        import_agent_handler,
        export_agent_handler,        
        get_agent_handler,
        get_all_agents_handler,
        test_llm_provider_handler,
    ),
    components(
        schemas(APIAddOllamaModels, SerializedLLMProvider, ShinkaiName, LLMProviderInterface,
            ShinkaiMessage, MessageBody, EncryptionMethod, ExternalMetadata, ShinkaiVersion,
            OpenAI, Ollama, LocalLLM, Groq, Gemini, Exo, EncryptedShinkaiBody, ShinkaiBody, 
            ShinkaiSubidentityType, ShinkaiBackend, InternalMetadata, MessageData, StopLLMRequest,
            NodeApiData, EncryptedShinkaiData, ShinkaiData, MessageSchemaType,
            APIUseRegistrationCodeSuccessResponse, GetPublicKeysResponse, APIError, Agent)
    ),
    tags(
        (name = "general", description = "General API endpoints")
    )
)]
pub struct GeneralApiDoc;
