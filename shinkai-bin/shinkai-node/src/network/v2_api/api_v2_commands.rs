use crate::llm_provider::providers::shinkai_backend::check_quota;
use crate::managers::galxe_quests::{compute_quests, generate_proof};
use crate::managers::tool_router::ToolRouter;
use crate::network::node_shareable_logic::download_zip_from_url;
use crate::network::zip_export_import::zip_export_import::{
    generate_agent_zip, get_agent_from_zip, import_agent, import_dependencies_tools,
};
use crate::utils::environment::NodeEnvironment;
use crate::{
    llm_provider::{job_manager::JobManager, llm_stopper::LLMStopper},
    managers::{identity_manager::IdentityManagerTrait, IdentityManager},
    network::{node_error::NodeError, Node},
    tools::tool_generation,
    utils::update_global_identity::update_global_identity_name,
};
use async_channel::Sender;
use ed25519_dalek::ed25519::signature::SignerMut;
use ed25519_dalek::{SigningKey, VerifyingKey};
use reqwest::StatusCode;
use rusqlite::params;
use serde_json::{json, Value};
use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_embedding::{embedding_generator::RemoteEmbeddingGenerator, model_type::EmbeddingModelType};
use shinkai_http_api::api_v2::api_v2_handlers_mcp_servers::{
    AddMCPServerRequest, DeleteMCPServerResponse, UpdateMCPServerRequest,
};
use shinkai_http_api::node_api_router::APIUseRegistrationCodeSuccessResponse;
use shinkai_http_api::{
    api_v2::api_v2_handlers_general::InitialRegistrationRequest,
    node_api_router::{APIError, GetPublicKeysResponse},
};
use shinkai_mcp::mcp_methods::{list_tools_via_command, list_tools_via_http, list_tools_via_sse};
use shinkai_message_primitives::schemas::llm_providers::shinkai_backend::QuotaResponse;
use shinkai_message_primitives::schemas::mcp_server::{MCPServer, MCPServerType};
use shinkai_message_primitives::schemas::shinkai_preferences::ShinkaiInternalComms;
use shinkai_message_primitives::{
    schemas::ws_types::WSUpdateHandler,
    schemas::{
        identity::{Identity, IdentityType, RegistrationCode},
        inbox_name::InboxName,
        llm_providers::{agent::Agent, serialized_llm_provider::SerializedLLMProvider},
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message_schemas::JobCreationInfo,
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{
            APIAddOllamaModels, IdentityPermissions, JobMessage, MessageSchemaType, V2ChatMessage,
        },
    },
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, EncryptionMethod},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::signature_public_key_to_string,
    },
    shinkai_utils::{job_scope::MinimalJobScope, shinkai_time::ShinkaiStringTime},
};
use shinkai_sqlite::regex_pattern_manager::RegexPattern;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::mcp_server_tool::MCPServerTool;
use shinkai_tools_primitives::tools::{
    agent_tool_wrapper::AgentToolWrapper,
    parameters::Parameters,
    shinkai_tool::ShinkaiTool,
    tool_config::{BasicConfig, ToolConfig},
    tool_output_arg::ToolOutputArg,
    tool_types::ToolResult,
};
use std::collections::HashMap;
use std::process::Command;
use std::time::Instant;
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use tokio::time::Duration;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::network::mcp_manager;

#[cfg(debug_assertions)]
fn check_bearer_token(api_key: &str, bearer: &str) -> Result<(), ()> {
    if api_key == bearer || bearer == "debug" {
        return Ok(());
    } else {
        return Err(());
    }
}

#[cfg(not(debug_assertions))]
fn check_bearer_token(api_key: &str, bearer: &str) -> Result<(), ()> {
    if api_key == bearer {
        return Ok(());
    } else {
        return Err(());
    }
}

impl Node {
    pub async fn validate_bearer_token<T>(
        bearer: &str,
        db: Arc<SqliteManager>,
        res: &Sender<Result<T, APIError>>,
    ) -> Result<(), ()> {
        // Compare bearer token to the environment variable API_V2_KEY
        let api_key = match env::var("API_V2_KEY") {
            Ok(api_key) => api_key,
            Err(_) => {
                // If the environment variable is not set, read from the database
                match db.read_api_v2_key() {
                    Ok(Some(api_key)) => api_key,
                    Ok(None) | Err(_) => {
                        let api_error = APIError {
                            code: StatusCode::UNAUTHORIZED.as_u16(),
                            error: "Unauthorized".to_string(),
                            message: "Invalid bearer token".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Err(());
                    }
                }
            }
        };

        let result = check_bearer_token(&api_key, bearer);
        match result {
            Ok(_) => Ok(()),
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::UNAUTHORIZED.as_u16(),
                    error: "Unauthorized".to_string(),
                    message: "Invalid bearer token".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                Err(())
            }
        }
    }

    pub async fn get_bearer_token<T>(
        db: Arc<SqliteManager>,
        res: &Sender<Result<T, APIError>>,
    ) -> Result<String, NodeError> {
        let api_key = match env::var("API_V2_KEY") {
            Ok(api_key) => api_key,
            Err(_) => {
                // If the environment variable is not set, read from the database
                match db.read_api_v2_key() {
                    Ok(Some(api_key)) => api_key,
                    Ok(None) | Err(_) => {
                        let api_error = APIError {
                            code: StatusCode::UNAUTHORIZED.as_u16(),
                            error: "Unauthorized".to_string(),
                            message: "Invalid bearer token".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Err(NodeError {
                            message: "Invalid bearer token".to_string(),
                        });
                    }
                }
            }
        };
        Ok(api_key)
    }

    pub fn convert_shinkai_message_to_v2_chat_message(
        shinkai_message: ShinkaiMessage,
    ) -> Result<V2ChatMessage, NodeError> {
        let internal_metadata = match &shinkai_message.body {
            MessageBody::Unencrypted(body) => Ok(&body.internal_metadata),
            _ => Err(NodeError {
                message: "Missing internal metadata".to_string(),
            }),
        }?;

        let message_data = match &shinkai_message.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => Ok(data),
                _ => Err(NodeError {
                    message: "Missing message data".to_string(),
                }),
            },
            _ => Err(NodeError {
                message: "Missing message data".to_string(),
            }),
        }?;

        let external_metadata = shinkai_message.external_metadata;

        let job_message: JobMessage =
            serde_json::from_str(&message_data.message_raw_content).map_err(|e| NodeError {
                message: format!("Failed to parse job message content: {}", e),
            })?;

        let node_api_data = internal_metadata.node_api_data.clone().ok_or(NodeError {
            message: "Missing node API data".to_string(),
        })?;

        Ok(V2ChatMessage {
            job_message,
            sender: external_metadata.sender,
            sender_subidentity: internal_metadata.sender_subidentity.clone(),
            receiver: external_metadata.recipient,
            receiver_subidentity: internal_metadata.recipient_subidentity.clone(),
            node_api_data,
            inbox: internal_metadata.inbox.clone(),
        })
    }

    pub fn convert_shinkai_messages_to_v2_chat_messages(
        shinkai_messages: Vec<Vec<ShinkaiMessage>>,
    ) -> Result<Vec<Vec<V2ChatMessage>>, NodeError> {
        shinkai_messages
            .into_iter()
            .map(|message_group| {
                message_group
                    .into_iter()
                    .map(Self::convert_shinkai_message_to_v2_chat_message)
                    .collect::<Result<Vec<V2ChatMessage>, NodeError>>()
            })
            .collect::<Result<Vec<Vec<V2ChatMessage>>, NodeError>>()
    }

    pub fn api_v2_create_shinkai_message(
        sender: ShinkaiName,
        receiver: ShinkaiName,
        payload: &str,
        schema: MessageSchemaType,
        node_encryption_sk: EncryptionStaticKey,
        node_signing_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        job: Option<String>,
    ) -> Result<ShinkaiMessage, &'static str> {
        let timestamp = ShinkaiStringTime::generate_time_now();
        let sender_subidentity = sender.get_fullname_string_without_node_name().unwrap_or_default();
        let receiver_subidentity = receiver.get_fullname_string_without_node_name().unwrap_or_default();

        let inbox_name = job
            .map(|job_id| {
                InboxName::get_job_inbox_name_from_params(job_id)
                    .map(|inbox| inbox.to_string())
                    .unwrap_or_else(|_| "".to_string())
            })
            .unwrap_or_else(|| "".to_string());

        ShinkaiMessageBuilder::new(node_encryption_sk, node_signing_sk, receiver_public_key)
            .message_raw_content(payload.to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type(schema)
            .internal_metadata_with_inbox(
                sender_subidentity.to_string(),
                receiver_subidentity.to_string(),
                inbox_name,
                EncryptionMethod::None,
                None,
            )
            .external_metadata_with_schedule(receiver.node_name.to_string(), sender.node_name.to_string(), timestamp)
            .build()
    }

    pub async fn v2_send_public_keys(
        identity_public_key: VerifyingKey,
        encryption_public_key: EncryptionPublicKey,
        res: Sender<Result<GetPublicKeysResponse, APIError>>,
    ) -> Result<(), NodeError> {
        let public_keys_response = GetPublicKeysResponse {
            signature_public_key: signature_public_key_to_string(identity_public_key),
            encryption_public_key: encryption_public_key_to_string(encryption_public_key),
        };

        if let Err(_) = res.send(Ok(public_keys_response)).await {
            let api_error = APIError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Failed to send public keys",
            );
            let _ = res.send(Err(api_error)).await;
        }

        Ok(())
    }

    pub async fn v2_handle_initial_registration(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        payload: InitialRegistrationRequest,
        public_https_certificate: Option<String>,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,

        first_device_needs_registration_code: bool,
        embedding_generator: Arc<RemoteEmbeddingGenerator>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        identity_secret_key: SigningKey,
        initial_llm_providers: Vec<SerializedLLMProvider>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        supported_embedding_models: Arc<Mutex<Vec<EmbeddingModelType>>>,
        libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<crate::network::libp2p_manager::NetworkEvent>>,
    ) {
        let registration_code = RegistrationCode {
            code: "".to_string(),
            registration_name: "main".to_string(),
            profile_identity_pk: payload.profile_identity_pk.clone(),
            profile_encryption_pk: payload.profile_encryption_pk.clone(),
            device_identity_pk: payload.profile_identity_pk,
            device_encryption_pk: payload.profile_encryption_pk,
            identity_type: IdentityType::Device,
            permission_type: IdentityPermissions::Admin,
        };

        match Self::handle_registration_code_usage(
            db,
            node_name,
            first_device_needs_registration_code,
            embedding_generator,
            identity_manager,
            job_manager,
            encryption_public_key,
            identity_public_key,
            identity_secret_key,
            initial_llm_providers,
            registration_code,
            ws_manager,
            supported_embedding_models,
            public_https_certificate,
            libp2p_event_sender,
            res.clone(),
        )
        .await
        {
            Ok(_) => {}
            Err(err) => {
                let error = APIError {
                    code: 500,
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to handle registration code usage: {}", err),
                };
                let _ = res.send(Err(error)).await;
            }
        }
    }

    pub async fn v2_api_get_storage_location(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let node_storage_path: Option<String> = match env::var("NODE_STORAGE_PATH").ok() {
            Some(val) => Some(val),
            None => Some("storage".to_string()),
        };
        let base_path = tokio::fs::canonicalize(node_storage_path.as_ref().unwrap())
            .await
            .unwrap();
        let _ = res.send(Ok(base_path.to_string_lossy().to_string())).await;

        Ok(())
    }

    pub async fn v2_api_get_default_embedding_model(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the default embedding model from the database
        match db.get_default_embedding_model() {
            Ok(model) => {
                let _ = res.send(Ok(model.to_string())).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get default embedding model: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_get_supported_embedding_models(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the supported embedding models from the database
        match db.get_supported_embedding_models() {
            Ok(models) => {
                let model_names: Vec<String> = models.into_iter().map(|model| model.to_string()).collect();
                let _ = res.send(Ok(model_names)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get supported embedding models: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_update_default_embedding_model(
        db: Arc<SqliteManager>,
        bearer: String,
        model_name: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert the string to EmbeddingModelType
        let new_default_model = match EmbeddingModelType::from_string(&model_name) {
            Ok(model) => model,
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Invalid embedding model provided".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Update the default embedding model in the database
        match db.update_default_embedding_model(new_default_model) {
            Ok(_) => {
                let _ = res
                    .send(Ok("Default embedding model updated successfully".to_string()))
                    .await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update default embedding model: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_add_llm_provider(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Option<Arc<Mutex<JobManager>>>,
        identity_secret_key: SigningKey,
        bearer: String,
        agent: SerializedLLMProvider,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let job_manager = match job_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "JobManager is required".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match Self::internal_add_llm_provider(
            db.clone(),
            identity_manager.clone(),
            job_manager,
            identity_secret_key.clone(),
            agent,
            &profile,
            ws_manager,
        )
        .await
        {
            Ok(_) => {
                let _ = res.send(Ok("Agent added successfully".to_string())).await;
                Ok(())
            }
            Err(err) => {
                // Check if the error message indicates a unique constraint violation
                let api_error = if err.to_string().contains("UNIQUE constraint failed") {
                    APIError {
                        code: StatusCode::CONFLICT.as_u16(),
                        error: "Conflict".to_string(),
                        message: "An LLM provider with this ID already exists".to_string(),
                    }
                } else {
                    APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("{}", err),
                    }
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_remove_llm_provider(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        llm_provider_id: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut identity_manager = identity_manager.lock().await;
        match db.remove_llm_provider(&llm_provider_id, &requester_name) {
            Ok(_) => match identity_manager.remove_agent_subidentity(&llm_provider_id).await {
                Ok(_) => {
                    let _ = res.send(Ok("Agent removed successfully".to_string())).await;
                    Ok(())
                }
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to remove agent from identity manager: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    Ok(())
                }
            },
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove agent: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_modify_llm_provider(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        agent: SerializedLLMProvider,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match db.update_llm_provider(agent.clone(), &requester_name) {
            Ok(_) => {
                let mut identity_manager = identity_manager.lock().await;
                match identity_manager.modify_llm_provider_subidentity(agent).await {
                    Ok(_) => {
                        let _ = res.send(Ok("Agent modified successfully".to_string())).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to update agent in identity manager: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update agent: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_check_shinkai_backend_quota(
        db: Arc<SqliteManager>,
        model_type: String,
        bearer: String,
        res: Sender<Result<QuotaResponse, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match check_quota(db, model_type).await {
            Ok(quota_response) => {
                let _ = res.send(Ok(quota_response)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to fetch quota: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_change_nodes_name(
        bearer: String,
        db: Arc<SqliteManager>,
        secret_file_path: &str,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        new_name: String,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Validate the new node name
        let new_node_name = match ShinkaiName::from_node_name(new_name) {
            Ok(name) => name,
            Err(_) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Invalid node name".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        };

        {
            // Check if the new node name exists in the blockchain and the keys match
            let identity_manager = identity_manager.lock().await;
            match identity_manager
                .external_profile_to_global_identity(new_node_name.get_node_name_string().as_str(), Some(true))
                .await
            {
                Ok(standard_identity) => {
                    if standard_identity.node_encryption_public_key != encryption_public_key
                        || standard_identity.node_signature_public_key != identity_public_key
                    {
                        let _ = res
                            .send(Err(APIError {
                                code: StatusCode::FORBIDDEN.as_u16(),
                                error: "Forbidden".to_string(),
                                message: "The keys do not match with the current node".to_string(),
                            }))
                            .await;
                        return Ok(());
                    }
                }
                Err(_) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::NOT_FOUND.as_u16(),
                            error: "Not Found".to_string(),
                            message: "The new node name does not exist in the blockchain".to_string(),
                        }))
                        .await;
                    return Ok(());
                }
            }
        }

        // Write to .secret file
        match update_global_identity_name(secret_file_path, new_node_name.get_node_name_string().as_str()) {
            Ok(_) => {
                eprintln!("Node name changed successfully. Restarting server...");
                let _ = res.send(Ok(())).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                panic!("Node name changed successfully. Restarting server...");
            }
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("{}", err),
                    }))
                    .await;
            }
        };
        Ok(())
    }

    pub async fn v2_api_is_pristine(
        bearer: String,
        db: Arc<SqliteManager>,
        res: Sender<Result<bool, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let has_any_profile = db.has_any_profile().unwrap_or(false);
        let _ = res.send(Ok(!has_any_profile)).await;
        Ok(())
    }

    pub async fn v2_api_health_check(
        db: Arc<SqliteManager>,
        public_https_certificate: Option<String>,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        let public_https_certificate = match public_https_certificate {
            Some(cert) => cert,
            None => "".to_string(),
        };

        let version = env!("CARGO_PKG_VERSION");

        let (_current_version, needs_global_reset) = match db.get_version() {
            Ok(version) => version,
            Err(_err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to get version in table"),
                    }))
                    .await;
                return Ok(());
            }
        };

        let _ = res
            .send(Ok(serde_json::json!({
                "is_pristine": !db.has_any_profile().unwrap_or(false),
                "public_https_certificate": public_https_certificate,
                "version": version,
                "update_requires_reset": needs_global_reset,
                "docker_status": "not-installed",
            })))
            .await;
        Ok(())
    }

    pub async fn v2_api_scan_ollama_models(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Vec<serde_json::Value>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match Self::internal_scan_ollama_models().await {
            Ok(response) => {
                let _ = res.send(Ok(response)).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_add_ollama_models(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Option<Arc<Mutex<JobManager>>>,
        identity_secret_key: SigningKey,
        bearer: String,
        payload: APIAddOllamaModels,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let job_manager = match job_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "JobManager is required".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match Node::internal_add_ollama_models(
            db,
            identity_manager,
            job_manager,
            identity_secret_key,
            payload.models,
            requester_name,
            ws_manager,
        )
        .await
        {
            Ok(_) => {
                let _ = res.send(Ok::<(), APIError>(())).await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add model: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_stop_llm(
        db: Arc<SqliteManager>,
        stopper: Arc<LLMStopper>,
        bearer: String,
        inbox_name: String,
        job_manager: Option<Arc<Mutex<JobManager>>>,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Check if the inbox_name starts with "jobid_"
        let inbox_name = if inbox_name.starts_with("jobid_") {
            match InboxName::get_job_inbox_name_from_params(inbox_name.clone()) {
                Ok(name) => name,
                Err(_) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: "Invalid job ID format".to_string(),
                        }))
                        .await;
                    return Ok(());
                }
            }
        } else {
            match InboxName::new(inbox_name.clone()) {
                Ok(name) => name,
                Err(_) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: "Invalid inbox name format".to_string(),
                        }))
                        .await;
                    return Ok(());
                }
            }
        };

        // Search in job manager and fill the job as well
        if let Some(job_manager) = job_manager {
            if let Some(job_id) = inbox_name.get_job_id() {
                // Get both queue managers
                let job_queue_manager_normal = job_manager.lock().await.job_queue_manager_normal.clone();
                let job_queue_manager_immediate = job_manager.lock().await.job_queue_manager_immediate.clone();

                // First try to dequeue from immediate queue
                let dequeue_result_immediate = job_queue_manager_immediate.lock().await.dequeue(&job_id).await;
                if let Ok(Some(_)) = dequeue_result_immediate {
                    // Job was successfully dequeued from immediate queue
                } else {
                    // If not found in immediate queue, try normal queue
                    let dequeue_result_normal = job_queue_manager_normal.lock().await.dequeue(&job_id).await;
                    if let Ok(Some(_)) = dequeue_result_normal {
                        // Job was successfully dequeued from normal queue
                    } else {
                        eprintln!("Job {} not found in either queue", job_id);
                    }
                }
            }
        }

        // Stop the LLM
        stopper.stop(&inbox_name.get_value());

        let _ = res.send(Ok(())).await;
        Ok(())
    }

    pub async fn v2_api_add_agent(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        mut agent: Agent,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the profile name from the identity manager
        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Construct the expected full identity name
        let expected_full_identity_name = ShinkaiName::new(format!(
            "{}/main/agent/{}",
            requester_name.get_node_name_string(),
            agent.agent_id.to_lowercase()
        ))
        .unwrap();

        // Check if the full identity name matches
        if agent.full_identity_name != expected_full_identity_name {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid full identity name.".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        // TODO: validate tools
        // TODO: validate knowledge

        // My created agents are always marked as edited
        agent.edited = true;

        // Check if the llm_provider_id exists
        let llm_provider_exists = {
            let exists = match db.get_llm_provider(&agent.llm_provider_id, &requester_name) {
                Ok(Some(_)) => true,
                _ => false,
            };
            exists
        };

        if llm_provider_exists {
            // Check if the agent_id already exists
            let agent_exists = {
                let exists = match db.get_agent(&agent.agent_id) {
                    Ok(Some(_)) => true,
                    _ => false,
                };
                exists
            };

            if agent_exists {
                let api_error = APIError {
                    code: StatusCode::CONFLICT.as_u16(),
                    error: "Conflict".to_string(),
                    message: "agent_id already exists".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
            } else {
                // Add the agent to the database
                match db.add_agent(agent.clone(), &requester_name) {
                    Ok(_) => {
                        // Create and add Agent tool wrapper
                        let node_name = requester_name.get_node_name_string();
                        let agent_tool_wrapper = AgentToolWrapper::new(
                            agent.agent_id.clone(),
                            agent.name.clone(),
                            agent.ui_description.clone(),
                            node_name,
                            None,
                        );

                        let shinkai_tool = ShinkaiTool::Agent(agent_tool_wrapper, true);

                        // Add agent tool to database
                        if let Err(err) = db.add_tool(shinkai_tool).await {
                            eprintln!("Warning: Failed to add agent tool: {}", err);
                        }

                        let _ = res.send(Ok("Agent added successfully".to_string())).await;
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to add agent: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
            }
        } else {
            let api_error = APIError {
                code: StatusCode::NOT_FOUND.as_u16(),
                error: "Not Found".to_string(),
                message: "llm_provider_id does not exist".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
        }

        Ok(())
    }

    pub async fn v2_api_remove_agent(
        db: Arc<SqliteManager>,
        bearer: String,
        agent_id: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the agent before removal so that we can inspect its tools
        let agent_opt = match db.get_agent(&agent_id) {
            Ok(agent) => agent,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to fetch agent: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Remove the agent from the database
        match db.remove_agent(&agent_id) {
            Ok(_) => {
                // Remove the agent wrapper tool
                let tool = match db.get_tool_by_agent_id(&agent_id) {
                    Ok(tool) => tool,
                    Err(err) => {
                        eprintln!("Internal inconsistency: Failed to get tool: {}", err);
                        return Ok(());
                    }
                };
                if let Err(err) = db.remove_tool(&tool.tool_router_key().to_string_without_version(), tool.tool_router_key().version().map(|v| v.to_string())) {
                    eprintln!("Warning: Failed to remove agent tool: {}", err);
                }

                // If the agent only had a single tool and that tool is a Network tool,
                // also remove that network tool from the database. This mirrors the
                // logic when creating a network agent via `v2_api_add_network_agent`.
                if let Some(agent) = agent_opt {
                    if agent.tools.len() == 1 {
                        let tk = &agent.tools[0];
                        match db.get_tool_by_key_and_version(&tk.to_string_without_version(), tk.version()) {
                            Ok(ShinkaiTool::Network(_, _)) => {
                                if let Err(err) =
                                    db.remove_tool(&tk.to_string_without_version(), tk.version().map(|v| v.to_string()))
                                {
                                    eprintln!("Warning: Failed to remove network tool: {}", err);
                                }
                            }
                            _ => {}
                        }
                    }
                }

                let _ = res.send(Ok("Agent removed successfully".to_string())).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove agent: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_update_agent(
        db: Arc<SqliteManager>,
        bearer: String,
        full_identity: ShinkaiName,
        partial_agent: serde_json::Value,
        res: Sender<Result<Agent, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Extract agent_id from partial_agent
        let agent_id = match partial_agent.get("agent_id").and_then(|id| id.as_str()) {
            Some(id) => id.to_string(),
            None => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "agent_id is missing in the request".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Construct the Agent's full identity name, in the local node.
        let local_full_identity_name = ShinkaiName::new(format!(
            "{}/main/agent/{}",
            full_identity.get_node_name_string(),
            agent_id.to_lowercase()
        ))
        .unwrap();

        // Retrieve the existing agent from the database
        let existing_agent = match db.get_agent(&agent_id) {
            Ok(Some(agent)) => agent,
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Agent not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Database error: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Manually merge fields from partial_agent with existing_agent
        let updated_agent = Agent {
            name: partial_agent
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&existing_agent.name)
                .to_string(),
            agent_id: existing_agent.agent_id.clone(), // Keep the original agent_id
            llm_provider_id: partial_agent
                .get("llm_provider_id")
                .and_then(|v| v.as_str())
                .unwrap_or(&existing_agent.llm_provider_id)
                .to_string(),
            // TODO: decide if we keep this
            // instructions: partial_agent
            //     .get("instructions")
            //     .and_then(|v| v.as_str())
            //     .unwrap_or(&existing_agent.instructions)
            //     .to_string(),
            ui_description: partial_agent
                .get("ui_description")
                .and_then(|v| v.as_str())
                .unwrap_or(&existing_agent.ui_description)
                .to_string(),
            knowledge: partial_agent
                .get("knowledge")
                .and_then(|v| v.as_array())
                .map_or(existing_agent.knowledge.clone(), |v| {
                    v.iter().filter_map(|s| s.as_str().map(String::from)).collect()
                }),
            scope: partial_agent
                .get("scope")
                .map(|v| serde_json::from_value::<MinimalJobScope>(v.clone()).unwrap_or(existing_agent.scope.clone()))
                .unwrap_or(existing_agent.scope.clone()),
            storage_path: partial_agent
                .get("storage_path")
                .and_then(|v| v.as_str())
                .unwrap_or(&existing_agent.storage_path)
                .to_string(),
            tools: partial_agent
                .get("tools")
                .and_then(|v| v.as_array())
                .map_or(existing_agent.tools.clone(), |v| {
                    v.iter()
                        .filter_map(|s| serde_json::from_value(s.clone()).ok())
                        .collect()
                }),
            debug_mode: partial_agent
                .get("debug_mode")
                .and_then(|v| v.as_bool())
                .unwrap_or(existing_agent.debug_mode),
            config: partial_agent.get("config").map_or(existing_agent.config.clone(), |v| {
                serde_json::from_value(v.clone()).unwrap_or(existing_agent.config.clone())
            }),
            cron_tasks: None,
            full_identity_name: local_full_identity_name.clone(),
            tools_config_override: partial_agent
                .get("tools_config_override")
                .map_or(existing_agent.tools_config_override.clone(), |v| {
                    serde_json::from_value(v.clone()).unwrap_or(existing_agent.tools_config_override.clone())
                }),
            edited: true,
        };

        // Update the agent in the database
        match db.update_agent(updated_agent.clone()) {
            Ok(_) => {
                let _ = res.send(Ok(updated_agent)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update agent: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_get_agent(
        db: Arc<SqliteManager>,
        bearer: String,
        agent_id: String,
        res: Sender<Result<Agent, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the agent from the database
        match db.get_agent(&agent_id) {
            Ok(Some(mut agent)) => {
                // Get cron tasks for this agent
                match db.get_cron_tasks_by_llm_provider_id(&agent.agent_id) {
                    Ok(cron_tasks) => {
                        agent.cron_tasks = if cron_tasks.is_empty() { None } else { Some(cron_tasks) };
                    }
                    Err(_e) => {
                        agent.cron_tasks = None;
                    }
                }
                let _ = res.send(Ok(agent)).await;
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Agent not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve agent: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_get_all_agents(
        db: Arc<SqliteManager>,
        bearer: String,
        filter: Option<String>,
        res: Sender<Result<Vec<Agent>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let agents_result = db.get_all_agents();
        match agents_result {
            Ok(mut agents) => {
                // If filter is Some("recently_used"), filter agents by recently used
                if let Some(ref filter_val) = filter {
                    if filter_val == "recently_used" {
                        // Get the last N recently used agent IDs (let's use 10 as a default)
                        let recent_ids = db.get_last_n_parent_agent_or_llm_provider_ids(10).unwrap_or_default();
                        agents.retain(|agent| recent_ids.contains(&agent.agent_id));
                    }
                }
                // Get cron tasks for each agent
                for agent in &mut agents {
                    match db.get_cron_tasks_by_llm_provider_id(&agent.agent_id) {
                        Ok(cron_tasks) => {
                            agent.cron_tasks = if cron_tasks.is_empty() { None } else { Some(cron_tasks) };
                        }
                        Err(_e) => {
                            agent.cron_tasks = None;
                        }
                    }
                }
                let _ = res.send(Ok(agents)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve agents: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_test_llm_provider(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Option<Arc<Mutex<JobManager>>>,
        identity_secret_key: SigningKey,
        bearer: String,
        provider: SerializedLLMProvider,
        node_name: ShinkaiName,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        _node_signing_sk: SigningKey,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Create a new SerializedLLMProvider with id and full_identity_name set to "llm_test"
        let name = node_name.extract_node().get_node_name_string();
        let profile = ShinkaiName::new(format!("{}/main", name)).unwrap();
        let llm_name = format!("{}/main/agent/test_llm_provider", name);

        let provider = SerializedLLMProvider {
            id: "llm_test".to_string(),
            name: None,
            description: None,
            full_identity_name: ShinkaiName::new(llm_name).unwrap(),
            external_url: provider.external_url.clone(),
            api_key: provider.api_key.clone(),
            model: provider.model.clone(),
        };

        Self::ensure_llm_provider(db.clone(), &profile, provider.clone()).await?;

        // Ensure job_manager is available
        let job_manager = match job_manager {
            Some(manager) => manager,
            None => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "JobManager is required".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Get the provider name as a ShinkaiName
        let profile = {
            let identity_manager_lock = identity_manager.lock().await;
            match identity_manager_lock.get_main_identity() {
                Some(identity) => identity.get_shinkai_name(),
                None => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to retrieve main identity".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Add the LLM provider
        match Self::internal_add_llm_provider(
            db.clone(),
            identity_manager.clone(),
            job_manager.clone(),
            identity_secret_key.clone(),
            provider.clone(),
            &profile,
            ws_manager.clone(),
        )
        .await
        {
            Ok(_) => {
                // Create Job and Send Message
                match tool_generation::v2_create_and_send_job_message(
                    bearer.clone(),
                    JobCreationInfo {
                        scope: MinimalJobScope::default(),
                        is_hidden: Some(true),
                        associated_ui: None,
                    },
                    provider.id.clone(),
                    "Repeat back the following message: dogcat. Just the word, no other words.".to_string(),
                    None, // tools
                    None, // fs_file_paths
                    None, // job_filenames
                    db.clone(),
                    profile.extract_node().clone(),
                    identity_manager.clone(),
                    job_manager.clone(),
                    node_encryption_sk,
                    node_encryption_pk,
                    identity_secret_key.clone(),
                )
                .await
                {
                    Ok(job_id) => {
                        // Wait for response
                        let timeout_duration = Duration::from_secs(60); // Set a timeout duration
                        match Self::check_job_response(db.clone(), job_id.clone(), "dogcat", timeout_duration).await {
                            Ok(_) => {
                                // Clean up the test LLM provider
                                if let Err(e) = db.remove_llm_provider(&provider.id, &profile) {
                                    eprintln!("Warning: Failed to clean up test LLM provider: {}", e);
                                }

                                let response = serde_json::json!({
                                    "message": "LLM provider tested successfully",
                                    "status": "success"
                                });
                                let _ = res.send(Ok(response)).await;
                                Ok(())
                            }
                            Err(err) => {
                                // Clean up the test LLM provider even if test failed
                                if let Err(e) = db.remove_llm_provider(&provider.id, &profile) {
                                    eprintln!("Warning: Failed to clean up test LLM provider: {}", e);
                                }

                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Error: {:?}", err.message),
                                };
                                let _ = res.send(Err(api_error)).await;
                                Ok(())
                            }
                        }
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to create job and send message: {:?}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add LLM provider: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    async fn check_job_response(
        db: Arc<SqliteManager>,
        job_id: String,
        expected_response: &str,
        timeout_duration: Duration,
    ) -> Result<(), APIError> {
        let start = Instant::now();
        loop {
            // Fetch the last messages from the job inbox
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
            let inbox_name_value = match inbox_name {
                InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
            };

            let last_messages_inbox = db
                .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 10, None)
                .unwrap_or_default();

            // Ensure there are at least two messages
            if last_messages_inbox.len() >= 2 {
                // Check the content of the second message
                if let Some(second_message_group) = last_messages_inbox.get(1) {
                    for message in second_message_group {
                        if let Ok(content) = message.get_message_content() {
                            if !content.is_empty() && content.contains(expected_response) {
                                return Ok(());
                            }
                        }
                    }
                }
                // If the second message does not contain the expected response, return an error
                let error_message = if let Some(second_message_group) = last_messages_inbox.get(1) {
                    let content = second_message_group
                        .iter()
                        .filter_map(|msg| {
                            // Get the raw content from the message
                            if let MessageBody::Unencrypted(body) = &msg.body {
                                if let MessageData::Unencrypted(data) = &body.message_data {
                                    // Parse the raw content as JobMessage
                                    if let Ok(job_message) =
                                        serde_json::from_str::<JobMessage>(&data.message_raw_content)
                                    {
                                        return Some(job_message.content);
                                    }
                                }
                            }
                            None
                        })
                        .collect::<Vec<String>>()
                        .join(", ");

                    content
                } else {
                    "Error but no specific error message received. Double-check the model name and your key."
                        .to_string()
                };
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: error_message,
                });
            }

            // Check if the timeout has been reached
            if start.elapsed() > timeout_duration {
                return Err(APIError {
                    code: StatusCode::REQUEST_TIMEOUT.as_u16(),
                    error: "Request Timeout".to_string(),
                    message: "Failed to receive the expected response in time".to_string(),
                });
            }

            // Sleep for a short duration before checking again
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn ensure_llm_provider(
        db: Arc<SqliteManager>,
        profile: &ShinkaiName,
        input_provider: SerializedLLMProvider,
    ) -> Result<(), NodeError> {
        // Check if the provider already exists
        let provider_exists = db.get_llm_provider(&input_provider.id, profile).is_ok();

        // If it exists, remove it
        if provider_exists {
            db.remove_llm_provider(&input_provider.id, profile)?;
        }

        Ok(())
    }

    pub async fn v2_api_export_agent(
        db: Arc<SqliteManager>,
        bearer: String,
        shinkai_name: ShinkaiName,
        node_env: NodeEnvironment,
        agent_id: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let agent_zip: Result<Vec<u8>, APIError> = generate_agent_zip(db, shinkai_name, node_env, agent_id, true).await;
        if let Err(err) = agent_zip {
            let _ = res.send(Err(err)).await;
            return Ok(());
        }
        let agent_zip = agent_zip.unwrap();
        let _ = res.send(Ok(agent_zip)).await;

        Ok(())
    }

    pub async fn v2_api_publish_agent(
        db: Arc<SqliteManager>,
        bearer: String,
        shinkai_name: ShinkaiName,
        node_env: NodeEnvironment,
        agent_id: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        signing_secret_key: SigningKey,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let response = Self::publish_agent(
            db.clone(),
            shinkai_name,
            node_env,
            agent_id,
            identity_manager,
            signing_secret_key,
        )
        .await;

        let _ = match response {
            Ok(response) => res.send(Ok(response)).await,
            Err(err) => res.send(Err(err)).await,
        };

        Ok(())
    }

    async fn publish_agent(
        db: Arc<SqliteManager>,
        shinkai_name: ShinkaiName,
        node_env: NodeEnvironment,
        agent_id: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        signing_secret_key: SigningKey,
    ) -> Result<Value, APIError> {
        // Generate zip file.
        let file_bytes: Vec<u8> =
            generate_agent_zip(db.clone(), shinkai_name.clone(), node_env, agent_id.clone(), true).await?;

        let identity_manager = identity_manager.lock().await;
        let local_node_name = identity_manager.local_node_name.clone();
        let identity_name = local_node_name.to_string();
        drop(identity_manager);

        // Hash
        let hash_raw = blake3::hash(&file_bytes.clone());
        let hash_hex = hash_raw.to_hex();
        let hash = hash_hex.to_string();

        // Signature
        let signature = signing_secret_key
            .clone()
            .try_sign(hash_hex.as_bytes())
            .map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to sign tool: {}", e),
            })?;

        let signature_bytes = signature.to_bytes();
        let signature_hex = hex::encode(signature_bytes);

        // Publish the tool to the store.
        let client = reqwest::Client::new();
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes).file_name(format!("{}.zip", agent_id.clone())),
            )
            .text("type", "Agent")
            .text("routerKey", agent_id.clone())
            .text("hash", hash.clone())
            .text("signature", signature_hex.clone())
            .text("identity", identity_name.clone());

        println!("[Publish Agent] Type: {}", "agent");
        println!("[Publish Agent] Agent ID: {}", agent_id.clone());
        println!("[Publish Agent] Hash: {}", hash.clone());
        println!("[Publish Agent] Signature: {}", signature_hex.clone());
        println!("[Publish Agent] Identity: {}", identity_name.clone());

        let store_url = env::var("SHINKAI_STORE_URL").unwrap_or("https://store-api.shinkai.com".to_string());
        let response = client
            .post(format!("{}/store/revisions", store_url))
            .multipart(form)
            .send()
            .await
            .map_err(|e| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to publish tool: {}", e),
            })?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default().clone();
        println!("Response: {:?}", response_text);

        if !status.is_success() {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Store Upload Error".to_string(),
                message: format!("Failed to upload to store: {}: {}", status, response_text),
            });
        }

        let r = json!({
            "status": "success",
            "message": "Agent published successfully",
            "agent_id": agent_id.clone(),
        });
        let r: Value = match r {
            Value::Object(mut map) => {
                let response_json = serde_json::from_str(&response_text).unwrap_or_default();
                map.insert("response".to_string(), response_json);
                Value::Object(map)
            }
            _ => unreachable!(),
        };
        return Ok(r);
    }

    pub async fn v2_api_import_agent_url(
        db: Arc<SqliteManager>,
        bearer: String,
        full_identity: ShinkaiName,
        url: String,
        node_env: NodeEnvironment,
        signing_secret_key: SigningKey,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let _ = match Self::v2_api_import_agent_url_internal(
            db.clone(),
            url.clone(),
            full_identity.clone(),
            node_env.clone(),
            signing_secret_key,
            embedding_generator,
        )
        .await
        {
            Ok(response) => res.send(Ok(response)).await,
            Err(err) => res.send(Err(err)).await,
        };
        Ok(())
    }

    pub async fn v2_api_import_agent_url_internal(
        db: Arc<SqliteManager>,
        url: String,
        full_identity: ShinkaiName,
        node_env: NodeEnvironment,
        signing_secret_key: SigningKey,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
    ) -> Result<Value, APIError> {
        let zip_contents = match download_zip_from_url(
            url,
            "__agent.json".to_string(),
            full_identity.node_name.clone(),
            signing_secret_key,
        )
        .await
        {
            Ok(contents) => contents,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Agent Zip".to_string(),
                    message: format!("Failed to extract agent.json: {:?}", err),
                };
                return Err(api_error);
            }
        };
        // Save the agent to the database
        // Parse the JSON into an Agent
        let agent: Agent = match serde_json::from_slice(&zip_contents.buffer) {
            Ok(agent) => agent,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid Agent Zip".to_string(),
                    message: format!("Failed to parse agent.json: {}", err),
                };
                return Err(api_error);
            }
        };

        let status = import_dependencies_tools(
            db.clone(),
            full_identity.clone(),
            node_env.clone(),
            zip_contents.archive.clone(),
            embedding_generator.clone(),
        )
        .await;
        if let Err(err) = status {
            return Err(err);
        }

        import_agent(
            db.clone(),
            full_identity,
            zip_contents.archive,
            agent.clone(),
            embedding_generator.clone(),
        )
        .await
    }

    pub async fn v2_api_import_agent_zip(
        db: Arc<SqliteManager>,
        bearer: String,
        full_identity: ShinkaiName,
        node_env: NodeEnvironment,
        file_data: Vec<u8>,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Process the zip file
        let cursor = std::io::Cursor::new(file_data);
        let archive = match zip::ZipArchive::new(cursor) {
            Ok(archive) => archive,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to read zip archive: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let agent = match get_agent_from_zip(archive.clone()) {
            Ok(agent) => agent,
            Err(err) => {
                let _ = res.send(Err(err)).await;
                return Ok(());
            }
        };

        let status = import_dependencies_tools(
            db.clone(),
            full_identity.clone(),
            node_env.clone(),
            archive.clone(),
            embedding_generator.clone(),
        )
        .await;
        if let Err(err) = status {
            let _ = res.send(Err(err)).await;
            return Ok(());
        }

        // Parse the JSON into an Agent
        let _ = match import_agent(db.clone(), full_identity, archive, agent.clone(), embedding_generator).await {
            Ok(response) => res.send(Ok(response)).await,
            Err(err) => res.send(Err(err)).await,
        };

        Ok(())
    }

    pub async fn v2_api_add_regex_pattern(
        db: Arc<SqliteManager>,
        bearer: String,
        provider_name: String,
        pattern: String,
        response: String,
        description: Option<String>,
        priority: i32,
        res: Sender<Result<i64, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Create the regex pattern
        let regex_pattern = match RegexPattern::new(provider_name, pattern, response, description, priority) {
            Ok(pattern) => pattern,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to create regex pattern: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Add the pattern to the database
        match db.add_regex_pattern(&regex_pattern) {
            Ok(id) => {
                let _ = res.send(Ok(id)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add regex pattern: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn handle_periodic_maintenance(
        _: Arc<SqliteManager>,
        _: ShinkaiName,
        _: Arc<Mutex<IdentityManager>>,
        tool_router: Option<Arc<ToolRouter>>,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
    ) -> Result<(), NodeError> {
        // Import tools from directory if tool_router is available
        if let Some(tool_router) = tool_router {
            if let Err(e) = tool_router.sync_tools_from_directory(embedding_generator.clone()).await {
                eprintln!("Error during periodic tool import: {}", e);
            }
        }
        Ok(())
    }

    pub async fn v2_api_check_default_tools_sync(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<bool, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate bearer token
        if let Err(_) = Self::validate_bearer_token(&bearer, db.clone(), &res).await {
            return Ok(());
        }

        // Get the internal_comms preference from the database
        let internal_comms_synced = match db.get_preference::<ShinkaiInternalComms>("internal_comms") {
            Ok(Some(internal_comms)) => internal_comms.internal_has_sync_default_tools,
            Ok(None) => false,
            Err(e) => {
                eprintln!("Error getting internal_comms preference: {}", e);
                false
            }
        };

        // Check if Rust tools are installed
        let rust_tools_installed = match db.has_rust_tools() {
            Ok(installed) => installed,
            Err(e) => {
                eprintln!("Error checking Rust tools: {}", e);
                false
            }
        };

        // Both conditions must be true
        let _ = res.send(Ok(internal_comms_synced && rust_tools_installed)).await;
        Ok(())
    }

    async fn internal_compute_quests_status(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
    ) -> Result<Value, String> {
        let quests_status = compute_quests(
            db.clone(),
            node_name.clone(),
            encryption_public_key.clone(),
            identity_public_key.clone(),
        )
        .await?;

        // Convert the Vec into a Vec of objects with just name and status
        let quests_array: Vec<_> = quests_status
            .into_iter()
            .map(|(_quest_type, quest_info)| {
                json!({
                    "name": quest_info.name,
                    "status": quest_info.status
                })
            })
            .collect();

        // Convert to string for signature generation
        let payload_string =
            serde_json::to_string(&quests_array).map_err(|e| format!("Failed to serialize quests array: {}", e))?;

        // Get the node's signature public key from the database
        let node_signature_public_key = db
            .query_row(
                "SELECT node_signature_public_key FROM local_node_keys LIMIT 1",
                params![],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(|e| format!("Failed to get node signature public key: {}", e))?;

        // Generate proof using the node's signature public key
        let (signature, metadata) = generate_proof(hex::encode(node_signature_public_key), payload_string)?;

        // Create the quests payload
        let quests_payload = json!({
            "quests": quests_array,
            "signed_proof": signature,
            "metadata": metadata,
            "node_name": node_name.to_string()
        });

        Ok(json!({
            "status": "success",
            "message": "Quests status computed successfully",
            "data": quests_payload,
        }))
    }

    pub async fn v2_api_compute_quests_status(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match Self::internal_compute_quests_status(db, node_name, encryption_public_key, identity_public_key).await {
            Ok(response) => {
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to compute quests status: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_compute_and_send_quests_status(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match Self::internal_compute_quests_status(
            db.clone(),
            node_name.clone(),
            encryption_public_key.clone(),
            identity_public_key.clone(),
        )
        .await
        {
            Ok(response) => {
                // Use the production Galxe API endpoint
                let galxe_quests_url = std::env::var("GALXE_QUESTS_URL")
                    .unwrap_or_else(|_| "https://api.shinkai.com/galxe/user".to_string());

                // Wrap the data in the correct structure
                let payload = json!({
                    "data": response["data"]
                });

                // Send the quests data to the Galxe backend
                let client = reqwest::Client::new();
                match client
                    .post(&galxe_quests_url)
                    .header("Content-Type", "application/json; charset=utf-8")
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(galxe_response) => match galxe_response.status() {
                        StatusCode::OK => {
                            let success_response = json!({
                                "status": "success",
                                "message": "Quests status computed and sent successfully",
                                "data": response["data"],
                            });
                            let _ = res.send(Ok(success_response)).await;
                        }
                        status => {
                            let error_message = galxe_response
                                .text()
                                .await
                                .unwrap_or_else(|_| "Failed to read error response".to_string());
                            let api_error = APIError {
                                code: status.as_u16(),
                                error: "Failed to send quests status".to_string(),
                                message: error_message,
                            };
                            let _ = res.send(Err(api_error)).await;
                        }
                    },
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to send quests status: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to compute quests status: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_set_preferences(
        db: Arc<SqliteManager>,
        bearer: String,
        payload: HashMap<String, serde_json::Value>,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut errors = vec![];
        for (key, value) in payload {
            if let Err(e) = db.set_preference(&key, &value, None) {
                errors.push(format!("Failed to set preference '{}': {}", key, e));
            }
        }

        if errors.is_empty() {
            let _ = res.send(Ok("Preferences set successfully".to_string())).await;
        } else {
            let error_message = errors.join("; ");
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: error_message,
            };
            let _ = res.send(Err(api_error)).await;
        }

        Ok(())
    }

    pub async fn v2_api_get_preferences(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match db.get_all_preferences() {
            Ok(preferences) => {
                let _ = res.send(Ok(json!(preferences))).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get preferences: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_get_last_used_agents_and_llms(
        db: Arc<SqliteManager>,
        bearer: String,
        last: usize,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let last_used_agents_llms = db
            .get_last_n_parent_agent_or_llm_provider_ids(last)
            .unwrap_or_else(|_| vec![]);
        let _ = res.send(Ok(last_used_agents_llms)).await;
        Ok(())
    }

    pub async fn v2_api_list_mcp_servers(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Vec<MCPServer>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve MCP servers from the database
        match db.get_all_mcp_servers() {
            Ok(servers) => {
                let _ = res.send(Ok(servers)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve MCP servers: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_add_mcp_server(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        bearer: String,
        mcp_server: AddMCPServerRequest,
        res: Sender<Result<MCPServer, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        log::debug!("Received request to add MCP server: {:?}", mcp_server);

        match mcp_server.r#type {
            MCPServerType::Command => {
                if let Some(cmd) = &mcp_server.command {
                    // Log the command components for debugging, as was previously done.
                    let mut parts = cmd.splitn(2, ' ');
                    let command_executable = parts.next().unwrap_or("").to_string();
                    let arguments = parts.next().unwrap_or("").to_string();
                    log::info!(
                        "MCP Server Type Command: executable='{}', arguments='{}'",
                        command_executable,
                        arguments
                    );
                } else {
                    log::warn!("No command provided for MCP Server: '{}'", mcp_server.name);
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid MCP Server Configuration".to_string(),
                            message: format!(
                                "No command string provided for MCP Server '{}' of type Command.",
                                mcp_server.name
                            ),
                        }))
                        .await;
                    return Ok(());
                }
            }
            MCPServerType::Sse => {
                if let Some(url) = &mcp_server.url {
                    log::info!("MCP Server Type Sse: URL='{}'", url);
                } else {
                    log::warn!("No URL provided for MCP Server: '{}'", mcp_server.name);
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid MCP Server Configuration".to_string(),
                            message: format!("No URL provided for MCP Server '{}' of type Sse.", mcp_server.name),
                        }))
                        .await;
                    return Ok(());
                }
            }
            MCPServerType::Http => {
                if let Some(url) = &mcp_server.url {
                    log::info!("MCP Server Type Http: URL='{}'", url);
                } else {
                    log::warn!("No URL provided for MCP Server: '{}'", mcp_server.name);
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid MCP Server Configuration".to_string(),
                            message: format!("No URL provided for MCP Server '{}' of type Http.", mcp_server.name),
                        }))
                        .await;
                    return Ok(());
                }
            }
            _ => {
                log::warn!(
                    "MCP Server type not yet fully implemented or recognized: {:?}",
                    mcp_server.r#type
                );
                // For now, we allow adding other types to the DB but won't attempt to spawn them.
                // If a type is strictly unsupported, this block could return an error.
                // The current logic proceeds to add to DB, which is fine.
            }
        }

        let exists = db.check_if_server_exists(
            &mcp_server.r#type,
            mcp_server.command.clone().unwrap_or_default().to_string(),
            mcp_server.url.clone().unwrap_or_default().to_string(),
        )?;
        if exists {
            let message = match mcp_server.r#type {
                MCPServerType::Command => format!(
                    "MCP Server with command '{}' already exists.",
                    mcp_server.command.clone().unwrap_or_default().to_string()
                ),
                MCPServerType::Sse => format!(
                    "MCP Server with url '{}' already exists.",
                    mcp_server.url.clone().unwrap_or_default().to_string()
                ),
                MCPServerType::Http => format!(
                    "MCP Server with url '{}' already exists.",
                    mcp_server.url.clone().unwrap_or_default().to_string()
                ),
            };
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "MCP Server Exists".to_string(),
                    message,
                }))
                .await;
            return Ok(());
        }

        // Add the MCP server to the database
        match db.add_mcp_server(
            None,
            mcp_server.name.clone(), // Clone name for db insertion
            mcp_server.r#type,
            mcp_server.url.clone(),
            mcp_server.command.clone(),
            mcp_server.env.clone(),
            mcp_server.is_enabled,
        ) {
            Ok(server) => {
                log::info!(
                    "MCP Server '{}' (ID: {:?}) added to database successfully.",
                    server.name,
                    server.id
                );
                if let Some(env) = &server.env {
                    log::info!("MCP Server '{}' (ID: {:?}) env: {:?}", server.name, server.id, env);
                }
                let server_command_hash = server.get_command_hash();
                if server.r#type == MCPServerType::Command && server.is_enabled {
                    if let Some(command_str) = &server.command {
                        log::info!(
                            "Attempting to spawn MCP server '{}' (ID: {:?}) with command: '{}'",
                            server.name,
                            server.id,
                            command_str
                        );

                        log::info!("Attempting to list tools for command: '{}' (Note: this runs the command separately for listing tools)", command_str);
                        let mut tools_config: Vec<ToolConfig> = vec![];
                        // Iterate over server config and add each key-value pair as a BasicConfig
                        if let Some(env) = &server.env {
                            for (key, value) in env {
                                tools_config.push(ToolConfig::BasicConfig(BasicConfig {
                                    key_name: key.clone(),
                                    description: format!("Configuration for {}", key),
                                    required: true,
                                    type_name: Some("string".to_string()),
                                    key_value: Some(serde_json::Value::String(value.to_string())),
                                }));
                            }
                        }
                        match list_tools_via_command(command_str, server.env.clone()).await {
                            Ok(tools) => {
                                for tool in tools {
                                    // Use the new function from mcp_manager instead of inline conversion
                                    let server_id = server.id.as_ref().expect("Server ID should exist").to_string();
                                    let shinkai_tool = mcp_manager::convert_to_shinkai_tool(
                                        &tool,
                                        &server.name,
                                        &server_id,
                                        &server_command_hash,
                                        &node_name.to_string(),
                                        tools_config.clone(),
                                    );

                                    if let Err(err) = db.add_tool(shinkai_tool).await {
                                        eprintln!("Warning: Failed to add mcp server tool: {}", err);
                                    };
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to list tools for command '{}' via list_tools_via_command: {:?}",
                                    command_str,
                                    e
                                );
                            }
                        }
                    } else {
                        // This should ideally not be reached if the check at the beginning of the function is robust,
                        // as mcp_server.command would have been validated.
                        log::warn!(
                            "MCP Server '{}' (ID: {:?}) is of type Command and enabled, but has no command string for spawning. This indicates an inconsistent state.",
                            server.name,
                            server.id
                        );
                    }
                } else if server.r#type == MCPServerType::Sse && server.is_enabled {
                    if let Some(url) = &server.url {
                        match list_tools_via_sse(url, None).await {
                            Ok(tools) => {
                                for tool in tools {
                                    // Use the new function from mcp_manager instead of inline conversion
                                    let server_id = server.id.as_ref().expect("Server ID should exist").to_string();
                                    let shinkai_tool = mcp_manager::convert_to_shinkai_tool(
                                        &tool,
                                        &server.name,
                                        &server_id,
                                        &server_command_hash,
                                        &node_name.to_string(),
                                        vec![],
                                    );

                                    if let Err(err) = db.add_tool(shinkai_tool).await {
                                        eprintln!("Warning: Failed to add mcp server tool: {}", err);
                                    };
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to list tools for sse '{}' via list_tools_via_sse: {:?}", url, e);
                            }
                        }
                    } else {
                        log::warn!("MCP Server '{}' (ID: {:?}) is of type Sse and enabled, but has no URL for spawning. This indicates an inconsistent state.", server.name, server.id);
                    }
                } else if server.r#type == MCPServerType::Http && server.is_enabled {
                    if let Some(url) = &server.url {
                        match list_tools_via_http(url, None).await {
                            Ok(tools) => {
                                for tool in tools {
                                    let server_id = server.id.as_ref().expect("Server ID should exist").to_string();
                                    let shinkai_tool = mcp_manager::convert_to_shinkai_tool(
                                        &tool,
                                        &server.name,
                                        &server_id,
                                        &server_command_hash,
                                        &node_name.to_string(),
                                        vec![],
                                    );

                                    if let Err(err) = db.add_tool(shinkai_tool).await {
                                        eprintln!("Warning: Failed to add mcp server tool: {}", err);
                                    };
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to list tools for http '{}' via list_tools_via_http: {:?}",
                                    url,
                                    e
                                );
                            }
                        }
                    } else {
                        log::warn!("MCP Server '{}' (ID: {:?}) is of type Http and enabled, but has no URL for spawning. This indicates an inconsistent state.", server.name, server.id);
                    }
                }

                let _ = res.send(Ok(server)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add MCP server: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_update_mcp_server(
        db: Arc<SqliteManager>,
        bearer: String,
        mcp_server: UpdateMCPServerRequest,
        node_name: &ShinkaiName,
        res: Sender<Result<MCPServer, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        match mcp_server.r#type {
            MCPServerType::Command => {
                if let Some(cmd) = &mcp_server.command {
                    let mut parts = cmd.splitn(2, ' ');
                    let command_executable = parts.next().unwrap_or("").to_string();
                    let arguments = parts.next().unwrap_or("").to_string();
                    log::info!(
                        "MCP Server Type Command: executable='{}', arguments='{}'",
                        command_executable,
                        arguments
                    );
                } else {
                    log::warn!(
                        "No command provided for MCP Server: '{}'",
                        mcp_server.name.clone().unwrap_or_default()
                    );
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid MCP Server Configuration".to_string(),
                            message: format!(
                                "No command string provided for MCP Server '{}' of type Command.",
                                mcp_server.name.clone().unwrap_or_default()
                            ),
                        }))
                        .await;
                    return Ok(());
                }
            }
            MCPServerType::Sse => {
                if let Some(url) = &mcp_server.url {
                    log::info!("MCP Server Type Sse: URL='{}'", url);
                } else {
                    log::warn!(
                        "No URL provided for MCP Server: '{}'",
                        mcp_server.name.clone().unwrap_or_default()
                    );
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid MCP Server Configuration".to_string(),
                            message: format!(
                                "No URL provided for MCP Server '{}' of type Sse.",
                                mcp_server.name.clone().unwrap_or_default()
                            ),
                        }))
                        .await;
                    return Ok(());
                }
            }
            MCPServerType::Http => {
                if let Some(url) = &mcp_server.url {
                    log::info!("MCP Server Type Http: URL='{}'", url);
                } else {
                    log::warn!(
                        "No URL provided for MCP Server: '{}'",
                        mcp_server.name.clone().unwrap_or_default()
                    );
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Invalid MCP Server Configuration".to_string(),
                            message: format!(
                                "No URL provided for MCP Server '{}' of type Http.",
                                mcp_server.name.clone().unwrap_or_default()
                            ),
                        }))
                        .await;
                    return Ok(());
                }
            }
            _ => {
                log::warn!(
                    "MCP Server type not yet fully implemented or recognized: {:?}",
                    mcp_server.r#type
                );
            }
        }
        let mcp_server_found = db.get_mcp_server(mcp_server.id)?;
        if mcp_server_found.is_none() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid MCP Server ID".to_string(),
                    message: format!("No MCP Server found with ID: {}", mcp_server.id),
                }))
                .await;
            return Ok(());
        }
        let updated_mcp_server = db.update_mcp_server(
            mcp_server.id,
            mcp_server.name.clone().unwrap_or(mcp_server_found.unwrap().name),
            mcp_server.r#type,
            mcp_server.url.clone(),
            mcp_server.command.clone(),
            mcp_server.env.clone(),
            mcp_server.is_enabled.unwrap_or(true),
        );
        match updated_mcp_server {
            Ok(updated_mcp_server) => {
                log::info!(
                    "MCP Server '{}' (ID: {:?}) updated proceeding to reset tools.",
                    updated_mcp_server.name,
                    updated_mcp_server.id
                );
                let rows_deleted_result = db.delete_all_tools_from_mcp_server(mcp_server.id.to_string());
                match rows_deleted_result {
                    Ok(count) => {
                        log::info!(
                            "Deleted {} tools from MCP Server '{}' (ID: {:?})",
                            count,
                            updated_mcp_server.name,
                            updated_mcp_server.id
                        );
                    }
                    Err(err) => {
                        log::error!(
                            "Failed to delete tools from MCP Server '{}' (ID: {:?}): {}",
                            updated_mcp_server.name,
                            updated_mcp_server.id,
                            err
                        );
                    }
                }
                let server_id = updated_mcp_server
                    .id
                    .as_ref()
                    .expect("Server ID should exist")
                    .to_string();
                let server_command_hash = updated_mcp_server.get_command_hash();
                match updated_mcp_server.r#type {
                    MCPServerType::Command => {
                        if let Some(cmd) = &mcp_server.command {
                            log::info!("Attempting to list tools for command: '{}' (Note: this runs the command separately for listing tools)", cmd);
                            let mut tools_config: Vec<ToolConfig> = vec![];
                            // Iterate over server config and add each key-value pair as a BasicConfig
                            if let Some(env) = &mcp_server.env {
                                for (key, value) in env {
                                    tools_config.push(ToolConfig::BasicConfig(BasicConfig {
                                        key_name: key.clone(),
                                        description: format!("Configuration for {}", key),
                                        required: true,
                                        type_name: Some("string".to_string()),
                                        key_value: Some(serde_json::Value::String(value.to_string())),
                                    }));
                                }
                            }
                            match list_tools_via_command(cmd, mcp_server.env.clone()).await {
                                Ok(tools) => {
                                    for tool in tools {
                                        let shinkai_tool = mcp_manager::convert_to_shinkai_tool(
                                            &tool,
                                            &updated_mcp_server.name,
                                            &server_id,
                                            &server_command_hash,
                                            &node_name.to_string(),
                                            tools_config.clone(),
                                        );
                                        if let Err(err) = db.add_tool(shinkai_tool).await {
                                            eprintln!("Warning: Failed to add mcp server tool: {}", err);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!(
                                        "Failed to list tools for command '{}' via list_tools_via_command: {:?}",
                                        cmd,
                                        e
                                    );
                                }
                            }
                        }
                    }
                    MCPServerType::Sse => {
                        if let Some(url) = &mcp_server.url {
                            match list_tools_via_sse(url, None).await {
                                Ok(tools) => {
                                    for tool in tools {
                                        let shinkai_tool = mcp_manager::convert_to_shinkai_tool(
                                            &tool,
                                            &updated_mcp_server.name,
                                            &server_id,
                                            &server_command_hash,
                                            &node_name.to_string(),
                                            vec![],
                                        );
                                        if let Err(err) = db.add_tool(shinkai_tool).await {
                                            eprintln!("Warning: Failed to add mcp server tool: {}", err);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!(
                                        "Failed to list tools for sse '{}' via list_tools_via_sse: {:?}",
                                        url,
                                        e
                                    );
                                }
                            }
                        }
                    }
                    MCPServerType::Http => {
                        if let Some(url) = &mcp_server.url {
                            match list_tools_via_http(url, None).await {
                                Ok(tools) => {
                                    for tool in tools {
                                        let shinkai_tool = mcp_manager::convert_to_shinkai_tool(
                                            &tool,
                                            &updated_mcp_server.name,
                                            &server_id,
                                            &server_command_hash,
                                            &node_name.to_string(),
                                            vec![],
                                        );
                                        if let Err(err) = db.add_tool(shinkai_tool).await {
                                            eprintln!("Warning: Failed to add mcp server tool: {}", err);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!(
                                        "Failed to list tools for http '{}' via list_tools_via_http: {:?}",
                                        url,
                                        e
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        log::warn!(
                            "MCP Server type not yet fully implemented or recognized: {:?}",
                            updated_mcp_server.r#type
                        );
                        let _ = res
                            .send(Err(APIError {
                                code: StatusCode::BAD_REQUEST.as_u16(),
                                error: "Invalid MCP Server Configuration".to_string(),
                                message: format!(
                                    "MCP Server type not yet fully implemented or recognized: {:?}",
                                    updated_mcp_server.r#type
                                ),
                            }))
                            .await;
                    }
                }
                let _ = res.send(Ok(updated_mcp_server)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update MCP server: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_import_mcp_server_from_github_url(
        db: Arc<SqliteManager>,
        bearer: String,
        github_url: String,
        res: Sender<Result<AddMCPServerRequest, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let mcp_server = mcp_manager::import_mcp_server_from_github_url(github_url).await;
        match mcp_server {
            Ok(mcp_server) => {
                let _ = res.send(Ok(mcp_server)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to import MCP server from GitHub URL: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_get_all_mcp_server_tools(
        db: Arc<SqliteManager>,
        bearer: String,
        mcp_server_id: i64,
        res: Sender<Result<Vec<MCPServerTool>, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let mcp_server = db.get_mcp_server(mcp_server_id)?;
        if mcp_server.is_none() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid MCP Server ID".to_string(),
                    message: format!("No MCP Server found with ID: {}", mcp_server_id),
                }))
                .await;
            return Ok(());
        }
        let tools = db.get_all_tools_from_mcp_server(mcp_server.unwrap().id.unwrap_or_default().to_string())?;
        let _ = res.send(Ok(tools)).await;
        Ok(())
    }

    pub async fn v2_api_delete_mcp_server(
        db: Arc<SqliteManager>,
        bearer: String,
        mcp_server_id: i64,
        res: Sender<Result<DeleteMCPServerResponse, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let mcp_server = db.get_mcp_server(mcp_server_id)?;
        if mcp_server.is_none() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid MCP Server ID".to_string(),
                    message: format!("No MCP Server found with ID: {}", mcp_server_id),
                }))
                .await;
            return Ok(());
        }
        let _ = db.delete_mcp_server(mcp_server_id);
        let rows_deleted_result =
            db.delete_all_tools_from_mcp_server(mcp_server.clone().unwrap().id.unwrap_or_default().to_string());

        match rows_deleted_result {
            Ok(count) => {
                let response = DeleteMCPServerResponse {
                    message: Some("MCP Server and associated tools deleted successfully".to_string()),
                    tools_deleted: count as i64, // Cast usize to i64
                    deleted_mcp_server: mcp_server.clone().unwrap(),
                };
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Failed to delete MCP server tools".to_string(),
                    message: format!("Error deleting tools for MCP server ID {}: {}", mcp_server_id, err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_set_enable_mcp_server(
        db: Arc<SqliteManager>,
        bearer: String,
        mcp_server_id: i64,
        is_enabled: bool,
        res: Sender<Result<MCPServer, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Check if the MCP server exists
        let mcp_server = db.get_mcp_server(mcp_server_id)?;
        if mcp_server.is_none() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Invalid MCP Server ID".to_string(),
                    message: format!("No MCP Server found with ID: {}", mcp_server_id),
                }))
                .await;
            return Ok(());
        }

        // Update the MCP server's enabled status
        match db.update_mcp_server_enabled_status(mcp_server_id, is_enabled) {
            Ok(updated_server) => {
                let _ = res.send(Ok(updated_server)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Failed to update MCP server status".to_string(),
                    message: format!("Error updating MCP server ID {}: {}", mcp_server_id, err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_docker_status(res: Sender<Result<serde_json::Value, APIError>>) -> Result<(), NodeError> {
        let docker_status = match shinkai_tools_runner::tools::container_utils::is_docker_available() {
            shinkai_tools_runner::tools::container_utils::DockerStatus::NotInstalled => "not-installed",
            shinkai_tools_runner::tools::container_utils::DockerStatus::NotRunning => "not-running",
            shinkai_tools_runner::tools::container_utils::DockerStatus::Running => "running",
        };

        let _ = res
            .send(Ok(serde_json::json!({
                "docker_status": docker_status,
            })))
            .await;
        Ok(())
    }
}
