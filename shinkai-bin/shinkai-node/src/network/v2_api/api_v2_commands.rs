use std::{env, sync::Arc};

use async_channel::Sender;
use ed25519_dalek::{SigningKey, VerifyingKey};
use reqwest::StatusCode;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::{
    api_v1::api_v1_handlers::APIUseRegistrationCodeSuccessResponse,
    api_v2::api_v2_handlers_general::InitialRegistrationRequest,
    node_api_router::{APIError, GetPublicKeysResponse},
};
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::{
    schemas::{
        identity::{Identity, IdentityType, RegistrationCode},
        inbox_name::InboxName,
        llm_providers::{agent::Agent, serialized_llm_provider::SerializedLLMProvider},
        shinkai_name::ShinkaiName,
    },
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
};
use shinkai_sqlite::SqliteManager;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::{
    embedding_generator::RemoteEmbeddingGenerator, model_type::EmbeddingModelType, shinkai_time::ShinkaiStringTime,
};
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::{
    llm_provider::{job_manager::JobManager, llm_stopper::LLMStopper},
    managers::{identity_manager::IdentityManagerTrait, IdentityManager},
    network::{node_error::NodeError, Node},
    utils::update_global_identity::update_global_identity_name,
};

use x25519_dalek::StaticSecret as EncryptionStaticKey;

#[cfg(debug_assertions)]
fn check_bearer_token(api_key: &str, bearer: &str) -> Result<(), ()> {
    if api_key == bearer || bearer == "debug" {
        return Ok(());
    } else {
        return Err(());
    }
}

#[cfg(not(debug_assertions))]
fn check_bearer_token(api_key: &str, bearer: &str) {
    if api_key == bearer {
        return Ok(());
    } else {
        return Err(());
    }
}

impl Node {
    pub async fn validate_bearer_token<T>(
        bearer: &str,
        db: Arc<RwLock<SqliteManager>>,
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
        db: Arc<RwLock<SqliteManager>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        payload: InitialRegistrationRequest,
        public_https_certificate: Option<String>,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
        vector_fs: Arc<VectorFS>,
        first_device_needs_registration_code: bool,
        embedding_generator: Arc<RemoteEmbeddingGenerator>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        identity_secret_key: SigningKey,
        initial_llm_providers: Vec<SerializedLLMProvider>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        supported_embedding_models: Arc<Mutex<Vec<EmbeddingModelType>>>,
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
            vector_fs,
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

    pub async fn v2_api_get_default_embedding_model(
        db: Arc<RwLock<SqliteManager>>,
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
        db: Arc<RwLock<SqliteManager>>,
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
        db: Arc<RwLock<SqliteManager>>,
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

    pub async fn v2_api_update_supported_embedding_models(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        models: Vec<String>,
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

        // Convert the strings to EmbeddingModelType
        let new_supported_models: Vec<EmbeddingModelType> = models
            .into_iter()
            .map(|s| EmbeddingModelType::from_string(&s).expect("Failed to parse embedding model"))
            .collect();

        // Update the supported embedding models in the database
        if let Err(err) = db.update_supported_embedding_models(new_supported_models.clone()) {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to update supported embedding models: {}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        match vector_fs
            .set_profile_supported_models(&requester_name, &requester_name, new_supported_models)
            .await
        {
            Ok(_) => {
                let _ = res
                    .send(Ok("Supported embedding models updated successfully".to_string()))
                    .await;
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update supported embedding models: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_add_llm_provider(
        db: Arc<RwLock<SqliteManager>>,
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
                // If everything went well, send the success message
                let _ = res.send(Ok("Agent added successfully".to_string())).await;
                Ok(())
            }
            Err(err) => {
                // If there was an error, send the error message
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

    pub async fn v2_api_remove_llm_provider(
        db: Arc<RwLock<SqliteManager>>,
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
        db: Arc<RwLock<SqliteManager>>,
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

    pub async fn v2_api_change_nodes_name(
        bearer: String,
        db: Arc<RwLock<SqliteManager>>,
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
                .external_profile_to_global_identity(new_node_name.get_node_name_string().as_str())
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
        db: Arc<RwLock<SqliteManager>>,
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
        db: Arc<RwLock<SqliteManager>>,
        public_https_certificate: Option<String>,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        let public_https_certificate = match public_https_certificate {
            Some(cert) => cert,
            None => "".to_string(),
        };

        let version = env!("CARGO_PKG_VERSION");

        let _ = res
            .send(Ok(serde_json::json!({
                "is_pristine": !db.has_any_profile().unwrap_or(false),
                "public_https_certificate": public_https_certificate,
                "version": version,
            })))
            .await;
        Ok(())
    }

    pub async fn v2_api_scan_ollama_models(
        db: Arc<RwLock<SqliteManager>>,
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
        db: Arc<RwLock<SqliteManager>>,
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

    pub async fn v2_api_download_file_from_inbox(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
        bearer: String,
        inbox_name: String,
        filename: String,
        res: Sender<Result<Vec<u8>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Try to decode the filename first to check if it's already encoded
        let encoded_filename = if urlencoding::decode(&filename).is_ok() {
            filename.clone()
        } else {
            urlencoding::encode(&filename).into_owned()
        };

        // Retrieve the file from the inbox
        match vector_fs.db.get_file_from_inbox(inbox_name, encoded_filename) {
            Ok(file_data) => {
                let _ = res.send(Ok(file_data)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve file from inbox: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_list_files_in_inbox(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
        bearer: String,
        inbox_name: String,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // List the files in the inbox
        match vector_fs.db.get_all_filenames_from_inbox(inbox_name) {
            Ok(file_list) => {
                let _ = res.send(Ok(file_list)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to list files in inbox: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_stop_llm(
        db: Arc<RwLock<SqliteManager>>,
        stopper: Arc<LLMStopper>,
        bearer: String,
        inbox_name: String,
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

        // Stop the LLM
        stopper.stop(&inbox_name.get_value());
        let _ = res.send(Ok(())).await;
        Ok(())
    }

    pub async fn v2_api_add_agent(
        db: Arc<RwLock<SqliteManager>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        agent: Agent,
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
            agent.agent_id
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

        // Check if the llm_provider_id exists
        match db.get_llm_provider(&agent.llm_provider_id, &requester_name) {
            Ok(Some(_)) => {
                // Check if the agent_id already exists
                match db.get_agent(&agent.agent_id) {
                    Ok(Some(_)) => {
                        let api_error = APIError {
                            code: StatusCode::CONFLICT.as_u16(),
                            error: "Conflict".to_string(),
                            message: "agent_id already exists".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                    Ok(None) => {
                        // Add the agent to the database
                        match db.add_agent(agent, &requester_name) {
                            Ok(_) => {
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
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to check agent_id: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "llm_provider_id does not exist".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to check llm_provider_id: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_remove_agent(
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        agent_id: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Remove the agent from the database
        match db.remove_agent(&agent_id) {
            Ok(_) => {
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
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
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

        // Construct the full identity name
        let full_identity_name = match ShinkaiName::new(format!(
            "{}/main/agent/{}",
            existing_agent.full_identity_name.get_node_name_string(),
            agent_id
        )) {
            Ok(name) => name,
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Failed to construct full identity name.".to_string(),
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
            storage_path: partial_agent
                .get("storage_path")
                .and_then(|v| v.as_str())
                .unwrap_or(&existing_agent.storage_path)
                .to_string(),
            tools: partial_agent
                .get("tools")
                .and_then(|v| v.as_array())
                .map_or(existing_agent.tools.clone(), |v| {
                    v.iter().filter_map(|s| s.as_str().map(String::from)).collect()
                }),
            debug_mode: partial_agent
                .get("debug_mode")
                .and_then(|v| v.as_bool())
                .unwrap_or(existing_agent.debug_mode),
            config: partial_agent.get("config").map_or(existing_agent.config.clone(), |v| {
                serde_json::from_value(v.clone()).unwrap_or(existing_agent.config.clone())
            }),
            full_identity_name, // Set the constructed full identity name
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
        db: Arc<RwLock<SqliteManager>>,
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
            Ok(Some(agent)) => {
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
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        res: Sender<Result<Vec<Agent>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve all agents from the database
        match db.get_all_agents() {
            Ok(agents) => {
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
}
