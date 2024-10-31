use async_channel::Sender;
use bytes::Buf;
use futures::StreamExt;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use shinkai_message_primitives::{
    schemas::{
        job_config::JobConfig,
        llm_providers::serialized_llm_provider::{
            Exo, Gemini, Groq, LLMProviderInterface, LocalLLM, Ollama, OpenAI, SerializedLLMProvider, ShinkaiBackend,
        },
        shinkai_name::{ShinkaiName, ShinkaiSubidentityType},
        smart_inbox::{LLMProviderSubset, V2SmartInbox},
    },
    shinkai_message::{
        shinkai_message::NodeApiData,
        shinkai_message_schemas::{
            APIChangeJobAgentRequest, AssociatedUI, CallbackAction, JobCreationInfo, JobMessage, SheetJobAction,
            SheetManagerAction, V2ChatMessage,
        },
    },
    shinkai_utils::job_scope::{
        JobScope, LocalScopeVRKaiEntry, LocalScopeVRPackEntry, VectorFSFolderScopeEntry,
        VectorFSItemScopeEntry,
    },
};
use utoipa::{OpenApi, ToSchema};
use warp::multipart::FormData;
use warp::Filter;

use crate::{
    node_api_router::{APIError, SendResponseBody, SendResponseBodyData},
    node_commands::NodeCommand,
};

use super::api_v2_router::{create_success_response, with_sender};

pub fn job_routes(
    node_commands_sender: Sender<NodeCommand>,
    _node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let create_job_route = warp::path("create_job")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(create_job_handler);

    let job_message_route = warp::path("job_message")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(job_message_handler);

    let get_last_messages_route = warp::path("last_messages")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(get_last_messages_handler);

    let get_all_smart_inboxes_route = warp::path("all_inboxes")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_all_smart_inboxes_handler);

    let available_llm_providers_route = warp::path("available_models")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_available_llm_providers_handler);

    let update_smart_inbox_name_route = warp::path("update_smart_inbox_name")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_smart_inbox_name_handler);

    let create_files_inbox_route = warp::path("create_files_inbox")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(create_files_inbox_handler);

    let add_file_to_inbox_route = warp::path("add_file_to_inbox")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::multipart::form().max_length(500 * 1024 * 1024)) // Set max length to 500 MB
        .and_then(add_file_to_inbox_handler);

    let change_job_llm_provider_route = warp::path("change_job_llm_provider")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(change_job_llm_provider_handler);

    let get_last_messages_with_branches_route = warp::path("last_messages_with_branches")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(get_last_messages_with_branches_handler);

    let update_job_config_route = warp::path("update_job_config")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_job_config_handler);

    let get_job_config_route = warp::path("get_job_config")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<GetJobConfigRequest>())
        .and_then(get_job_config_handler);

    let retry_message_route = warp::path("retry_message")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(retry_message_handler);

    let update_job_scope_route = warp::path("update_job_scope")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(update_job_scope_handler);

    let get_job_scope_route = warp::path("get_job_scope")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<GetJobScopeRequest>())
        .and_then(get_job_scope_handler);

    let get_tooling_logs_route = warp::path("get_tooling_logs")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<GetToolingLogsRequest>())
        .and_then(get_tooling_logs_handler);

    let fork_job_messages_route = warp::path("fork_job_messages")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(fork_job_messages_handler);

    let remove_job_route = warp::path("remove_job")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(remove_job_handler);

    create_job_route
        .or(job_message_route)
        .or(get_last_messages_route)
        .or(get_all_smart_inboxes_route)
        .or(available_llm_providers_route)
        .or(update_smart_inbox_name_route)
        .or(create_files_inbox_route)
        .or(add_file_to_inbox_route)
        .or(change_job_llm_provider_route)
        .or(update_job_config_route)
        .or(get_last_messages_with_branches_route)
        .or(get_job_config_route)
        .or(retry_message_route)
        .or(update_job_scope_route)
        .or(get_job_scope_route)
        .or(get_tooling_logs_route)
        .or(fork_job_messages_route)
        .or(remove_job_route)
}

#[derive(Deserialize, ToSchema)]
pub struct CreateJobRequest {
    pub job_creation_info: JobCreationInfo,
    pub llm_provider: String,
}

#[derive(Deserialize, ToSchema)]
pub struct JobMessageRequest {
    pub job_message: JobMessage,
}

#[derive(Deserialize, ToSchema)]
pub struct GetLastMessagesRequest {
    pub inbox_name: String,
    pub limit: usize,
    pub offset_key: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateSmartInboxNameRequest {
    pub inbox_name: String,
    pub custom_name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AddFileToInboxRequest {
    pub file_inbox_name: String,
    pub filename: String,
    pub file: Vec<u8>,
}

#[derive(Deserialize, ToSchema)]
pub struct GetLastMessagesWithBranchesRequest {
    pub inbox_name: String,
    pub limit: usize,
    pub offset_key: Option<String>,
}

// Code

#[derive(Deserialize, ToSchema)]
pub struct RetryMessageRequest {
    pub message_id: String,
    pub inbox_name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ForkJobMessagesRequest {
    pub job_id: String,
    pub message_id: String,
}

#[derive(Deserialize, ToSchema)]
pub struct RemoveJobRequest {
    pub job_id: String,
}

#[utoipa::path(
    post,
    path = "/v2/retry_message",
    request_body = RetryMessageRequest,
    responses(
        (status = 200, description = "Successfully retried message", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn retry_message_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: RetryMessageRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiRetryMessage {
            bearer,
            inbox_name: payload.inbox_name,
            message_id: payload.message_id,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "result": response }));
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
    path = "/v2/create_job",
    request_body = CreateJobRequest,
    responses(
        (status = 200, description = "Successfully created job", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn create_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: CreateJobRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiCreateJob {
            bearer,
            job_creation_info: payload.job_creation_info,
            llm_provider: payload.llm_provider,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "job_id": response }));
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
    path = "/v2/job_message",
    request_body = JobMessageRequest,
    responses(
        (status = 200, description = "Successfully processed job message", body = SendResponseBody),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn job_message_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: JobMessageRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiJobMessage {
            bearer,
            job_message: payload.job_message,
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
    path = "/v2/last_messages",
    request_body = GetLastMessagesRequest,
    responses(
        (status = 200, description = "Successfully retrieved last messages", body = Vec<V2ChatMessage>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_last_messages_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetLastMessagesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetLastMessagesFromInbox {
            bearer,
            inbox_name: payload.inbox_name,
            limit: payload.limit,
            offset_key: payload.offset_key,
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
    path = "/v2/all_inboxes",
    responses(
        (status = 200, description = "Successfully retrieved all smart inboxes", body = Vec<V2SmartInbox>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_all_smart_inboxes_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetAllSmartInboxes {
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
    path = "/v2/available_llm_providers",
    responses(
        (status = 200, description = "Successfully retrieved available LLM providers", body = Vec<SerializedLLMProvider>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_available_llm_providers_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiAvailableLLMProviders {
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
    path = "/v2/update_smart_inbox_name",
    request_body = UpdateSmartInboxNameRequest,
    responses(
        (status = 200, description = "Successfully updated smart inbox name", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_smart_inbox_name_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: UpdateSmartInboxNameRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiUpdateSmartInboxName {
            bearer,
            inbox_name: payload.inbox_name,
            custom_name: payload.custom_name,
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
    path = "/v2/create_files_inbox",
    responses(
        (status = 200, description = "Successfully created files inbox", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn create_files_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiCreateFilesInbox {
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
    path = "/v2/add_file_to_inbox",
    request_body = AddFileToInboxRequest,
    responses(
        (status = 200, description = "Successfully added file to inbox", body = String),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_file_to_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    mut form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let mut file_inbox_name = String::new();
    let mut filename = String::new();
    let mut file_data = Vec::new();

    while let Some(part) = form.next().await {
        let mut part = part.map_err(|e| {
            eprintln!("Error collecting form data: {:?}", e);
            warp::reject::custom(APIError::new(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                format!("Failed to collect form data: {:?}", e).as_str(),
            ))
        })?;
        match part.name() {
            "file_inbox_name" => {
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing file_inbox_name",
                    ))
                })?;
                let mut content = content.map_err(|e| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        format!("Failed to read file_inbox_name: {:?}", e).as_str(),
                    ))
                })?;
                file_inbox_name =
                    String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                        warp::reject::custom(APIError::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            "Invalid UTF-8 in file_inbox_name",
                        ))
                    })?;
            }
            "filename" => {
                let content = part.data().await.ok_or_else(|| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Missing filename",
                    ))
                })?;
                let mut content = content.map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Failed to read filename",
                    ))
                })?;
                filename = String::from_utf8(content.copy_to_bytes(content.remaining()).to_vec()).map_err(|_| {
                    warp::reject::custom(APIError::new(
                        StatusCode::BAD_REQUEST,
                        "Bad Request",
                        "Invalid UTF-8 in filename",
                    ))
                })?;
            }
            "file_data" => {
                while let Some(content) = part.data().await {
                    let mut content = content.map_err(|_| {
                        warp::reject::custom(APIError::new(
                            StatusCode::BAD_REQUEST,
                            "Bad Request",
                            "Failed to read file data",
                        ))
                    })?;
                    file_data.extend_from_slice(&content.copy_to_bytes(content.remaining()));
                }
            }
            _ => {}
        }
    }

    if file_data.is_empty() {
        return Err(warp::reject::custom(APIError::new(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "No file data found. Check that the file is being uploaded correctly in the `field_data` field",
        )));
    }

    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiAddFileToInbox {
            bearer,
            file_inbox_name,
            filename,
            file: file_data,
            res: res_sender,
        })
        .await
        .map_err(|_| {
            warp::reject::custom(APIError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Failed to send command",
            ))
        })?;
    let result = res_receiver.recv().await.map_err(|_| {
        warp::reject::custom(APIError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "Failed to receive response",
        ))
    })?;

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
    path = "/v2/change_job_llm_provider",
    request_body = APIChangeJobAgentRequest,
    responses(
        (status = 200, description = "Successfully changed job LLM provider", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn change_job_llm_provider_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: APIChangeJobAgentRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiChangeJobLlmProvider {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "result": response }));
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
    path = "/v2/last_messages_with_branches",
    request_body = GetLastMessagesWithBranchesRequest,
    responses(
        (status = 200, description = "Successfully retrieved last messages with branches", body = Vec<Vec<V2ChatMessage>>),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_last_messages_with_branches_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: GetLastMessagesWithBranchesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetLastMessagesFromInboxWithBranches {
            bearer,
            inbox_name: payload.inbox_name,
            limit: payload.limit,
            offset_key: payload.offset_key,
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
pub struct UpdateJobConfigRequest {
    pub job_id: String,
    pub config: JobConfig,
}

#[utoipa::path(
    post,
    path = "/v2/update_job_config",
    request_body = UpdateJobConfigRequest,
    responses(
        (status = 200, description = "Successfully updated job configuration", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_job_config_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: UpdateJobConfigRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiUpdateJobConfig {
            bearer,
            job_id: payload.job_id,
            config: payload.config.clone(),
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "result": response }));
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[derive(Deserialize)]
pub struct GetJobConfigRequest {
    pub job_id: String,
}

#[utoipa::path(
    get,
    path = "/v2/get_job_config",
    params(
        ("job_id" = String, Query, description = "Job ID to retrieve configuration for")
    ),
    responses(
        (status = 200, description = "Successfully retrieved job configuration", body = JobConfig),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_job_config_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    query: GetJobConfigRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetJobConfig {
            bearer,
            job_id: query.job_id,
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
pub struct UpdateJobScopeRequest {
    pub job_id: String,
    pub job_scope: JobScope,
}

#[utoipa::path(
    post,
    path = "/v2/update_job_scope",
    request_body = UpdateJobScopeRequest,
    responses(
        (status = 200, description = "Successfully updated job scope", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn update_job_scope_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: UpdateJobScopeRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiUpdateJobScope {
            bearer,
            job_id: payload.job_id,
            job_scope: payload.job_scope,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(_) => {
            let response = create_success_response(json!({ "result": "Job scope updated successfully" }));
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

#[derive(Deserialize)]
pub struct GetJobScopeRequest {
    pub job_id: String,
}

#[utoipa::path(
    get,
    path = "/v2/get_job_scope",
    params(
        ("job_id" = String, Query, description = "Job ID to retrieve scope for")
    ),
    responses(
        (status = 200, description = "Successfully retrieved job scope", body = JobScope),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_job_scope_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    query: GetJobScopeRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetJobScope {
            bearer,
            job_id: query.job_id,
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
pub struct GetToolingLogsRequest {
    pub message_id: String,
}

#[utoipa::path(
    get,
    path = "/v2/get_tooling_logs",
    params(
        ("message_id" = String, Query, description = "Message ID to retrieve tooling logs for")
    ),
    responses(
        (status = 200, description = "Successfully retrieved tooling logs", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_tooling_logs_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    query: GetToolingLogsRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiGetToolingLogs {
            bearer,
            message_id: query.message_id,
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
    path = "/v2/fork_job_messages",
    request_body = CreateJobRequest,
    responses(
        (status = 200, description = "Successfully created job", body = Value),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn fork_job_messages_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: ForkJobMessagesRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiForkJobMessages {
            bearer,
            job_id: payload.job_id,
            message_id: payload.message_id,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => {
            let response = create_success_response(json!({ "job_id": response }));
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
    path = "/v2/remove_job",
    request_body = RemoveJobRequest,
    responses(
        (status = 200, description = "Successfully removed job", body = SendResponseBody),
        (status = 400, description = "Bad request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    authorization: String,
    payload: RemoveJobRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::V2ApiRemoveJob {
            bearer,
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

#[derive(OpenApi)]
#[openapi(
    paths(
        get_all_smart_inboxes_handler,
        get_available_llm_providers_handler,
        create_job_handler,
        job_message_handler,
        get_last_messages_handler,
        update_smart_inbox_name_handler,
        create_files_inbox_handler,
        add_file_to_inbox_handler,
        change_job_llm_provider_handler,
        get_last_messages_with_branches_handler,
        update_job_config_handler,
        get_job_config_handler,
        retry_message_handler,
        update_job_scope_handler,
        get_job_scope_handler,
        get_tooling_logs_handler,
        fork_job_messages_handler,
        remove_job_handler,
    ),
    components(
        schemas(AddFileToInboxRequest, V2SmartInbox, APIChangeJobAgentRequest, CreateJobRequest, JobConfig,
            JobMessageRequest, GetLastMessagesRequest, V2ChatMessage, GetLastMessagesWithBranchesRequest,
            UpdateJobConfigRequest, UpdateSmartInboxNameRequest, SerializedLLMProvider, JobCreationInfo,
            JobMessage, NodeApiData, LLMProviderSubset, AssociatedUI, JobScope, LocalScopeVRKaiEntry, LocalScopeVRPackEntry,
            VectorFSItemScopeEntry, VectorFSFolderScopeEntry, CallbackAction, ShinkaiName,
            LLMProviderInterface, RetryMessageRequest, UpdateJobScopeRequest,
            ShinkaiSubidentityType, OpenAI, Ollama, LocalLLM, Groq, Gemini, Exo, ShinkaiBackend, SheetManagerAction,
            SheetJobAction, SendResponseBody, SendResponseBodyData, APIError, GetToolingLogsRequest, ForkJobMessagesRequest, RemoveJobRequest)
    ),
    tags(
        (name = "jobs", description = "Job API endpoints")
    )
)]
pub struct JobsApiDoc;
