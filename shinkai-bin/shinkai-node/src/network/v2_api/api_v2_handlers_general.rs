use async_channel::Sender;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    Exo, Gemini, GenericAPI, Groq, LLMProviderInterface, LocalLLM, Ollama, OpenAI, ShinkaiBackend,
};
use shinkai_message_primitives::schemas::shinkai_name::{ShinkaiName, ShinkaiSubidentityType};
use shinkai_message_primitives::shinkai_message::shinkai_message::{
    EncryptedShinkaiBody, ExternalMetadata, MessageBody, ShinkaiBody, ShinkaiMessage, ShinkaiVersion,
};
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::{
    schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider,
    shinkai_message::shinkai_message_schemas::APIAddOllamaModels,
};
use utoipa::OpenApi;
use warp::Filter;

use crate::network::v1_api::api_v1_handlers::APIUseRegistrationCodeSuccessResponse;
use crate::network::{
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

    let get_local_processing_preference_route = warp::path("local_processing_preference")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_local_processing_preference_handler);

    let update_local_processing_preference_route = warp::path("local_processing_preference")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_local_processing_preference_handler);

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

    public_keys_route
        .or(health_check_route)
        .or(initial_registration_route)
        .or(get_local_processing_preference_route)
        .or(update_local_processing_preference_route)
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
    let version = env!("CARGO_PKG_VERSION");

    let (res_sender, res_receiver) = async_channel::bounded(1);

    sender
        .send(NodeCommand::APIIsPristine { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    let pristine_state = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    if let Err(error) = pristine_state {
        return Ok(warp::reply::json(&json!({ "status": "error", "error": error })));
    }

    Ok(warp::reply::json(
        &json!({ "status": "ok", "version": version, "node_name": node_name, "is_pristine": pristine_state.unwrap() }),
    ))
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
    path = "/v2/local_processing_preference",
    responses(
        (status = 200, description = "Successfully retrieved local processing preference", body = bool),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_local_processing_preference_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetLocalProcessingPreference {
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
    path = "/v2/local_processing_preference",
    request_body = bool,
    responses(
        (status = 200, description = "Successfully updated local processing preference", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_local_processing_preference_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    preference: bool,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUpdateLocalProcessingPreference {
            bearer,
            preference,
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
    request_body = String,
    responses(
        (status = 200, description = "Successfully removed LLM provider", body = String),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_llm_provider_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    llm_provider_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
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

#[derive(Deserialize)]
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

#[derive(OpenApi)]
#[openapi(
    paths(
        download_file_from_inbox_handler,
        list_files_in_inbox_handler,
        get_public_keys,
        health_check,
        initial_registration_handler,
        get_local_processing_preference_handler,
        update_local_processing_preference_handler,
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
    ),
    components(
        schemas(APIAddOllamaModels, SerializedLLMProvider, ShinkaiName, LLMProviderInterface,
            ShinkaiMessage, MessageBody, EncryptionMethod, ExternalMetadata, ShinkaiVersion,
            OpenAI, GenericAPI, Ollama, LocalLLM, Groq, Gemini, Exo, EncryptedShinkaiBody, ShinkaiBody, 
            ShinkaiSubidentityType, ShinkaiBackend,
            APIUseRegistrationCodeSuccessResponse, GetPublicKeysResponse, APIError)
    ),
    tags(
        (name = "general", description = "General API endpoints")
    )
)]
pub struct GeneralApiDoc;
