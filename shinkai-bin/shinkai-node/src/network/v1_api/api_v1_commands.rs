use crate::managers::identity_manager::IdentityManagerTrait;
use crate::managers::tool_router::ToolRouter;
use crate::{
    llm_provider::job_manager::JobManager, managers::IdentityManager, network::{
        node::ProxyConnectionInfo, node_error::NodeError, node_shareable_logic::validate_message_main_logic, Node
    }, utils::update_global_identity::update_global_identity_name
};
use async_channel::Sender;
use blake3::Hasher;
use ed25519_dalek::{SigningKey, VerifyingKey};
use log::error;
use reqwest::StatusCode;
use serde_json::{json, Value as JsonValue};

use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_embedding::model_type::EmbeddingModelType;
use shinkai_http_api::node_api_router::{APIError, APIUseRegistrationCodeSuccessResponse, SendResponseBodyData};
use shinkai_message_primitives::schemas::identity::{
    DeviceIdentity, Identity, IdentityType, RegistrationCode, StandardIdentity, StandardIdentityType
};
use shinkai_message_primitives::schemas::inbox_permission::InboxPermission;
use shinkai_message_primitives::schemas::smart_inbox::SmartInbox;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::encryption::encryption_secret_key_to_string;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName, llm_providers::serialized_llm_provider::SerializedLLMProvider, shinkai_name::{ShinkaiName, ShinkaiSubidentityType}
    }, shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage}, shinkai_message_schemas::{
            APIAddAgentRequest, APIAddOllamaModels, APIChangeJobAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType
        }
    }, shinkai_utils::{
        encryption::{
            clone_static_secret_key, encryption_public_key_to_string, string_to_encryption_public_key, EncryptionMethod
        }, shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption}, signatures::{clone_signature_secret_key, signature_public_key_to_string, string_to_signature_public_key}
    }
};
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;

use std::{convert::TryInto, env, sync::Arc, time::Instant};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
    pub async fn validate_message(
        encryption_secret_key: EncryptionStaticKey,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: &ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        schema_type: Option<MessageSchemaType>,
    ) -> Result<(ShinkaiMessage, Identity), APIError> {
        validate_message_main_logic(
            &encryption_secret_key,
            identity_manager,
            &node_name.clone(),
            potentially_encrypted_msg,
            schema_type,
        )
        .await
    }

    async fn has_standard_identity_access(
        db: Arc<SqliteManager>,
        inbox_name: &InboxName,
        std_identity: &StandardIdentity,
    ) -> Result<bool, NodeError> {
        let has_permission = db
            .has_permission(&inbox_name.to_string(), std_identity, InboxPermission::Read)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(has_permission)
    }

    async fn has_device_identity_access(
        db: Arc<SqliteManager>,
        inbox_name: &InboxName,
        std_identity: &DeviceIdentity,
    ) -> Result<bool, NodeError> {
        let std_device = std_identity.clone().to_standard_identity().ok_or(NodeError {
            message: "Failed to convert to standard identity".to_string(),
        })?;
        Self::has_standard_identity_access(db, inbox_name, &std_device).await
    }

    pub async fn has_inbox_access(
        db: Arc<SqliteManager>,
        inbox_name: &InboxName,
        sender_subidentity: &Identity,
    ) -> Result<bool, NodeError> {
        let sender_shinkai_name = ShinkaiName::new(sender_subidentity.get_full_identity_name())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let has_creation_permission = inbox_name.has_creation_access(sender_shinkai_name);
        if let Ok(true) = has_creation_permission {
            println!("has_creation_permission: true");
            return Ok(true);
        }

        match sender_subidentity {
            Identity::Standard(std_identity) => Self::has_standard_identity_access(db, inbox_name, std_identity).await,
            Identity::Device(std_device) => Self::has_device_identity_access(db, inbox_name, std_device).await,
            _ => Err(NodeError {
                message: format!(
                    "Invalid Identity type. You don't have enough permissions to access the inbox: {}",
                    inbox_name
                ),
            }),
        }
    }

    async fn process_last_messages_from_inbox<F, T>(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<T, APIError>>,
        response_handler: F,
    ) -> Result<(), NodeError>
    where
        F: FnOnce(Vec<Vec<ShinkaiMessage>>) -> T,
    {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager,
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::APIGetMessagesFromInboxRequest),
        )
        .await;
        let (msg, sender_subidentity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let content = match msg.body {
            MessageBody::Unencrypted(body) => match body.message_data {
                MessageData::Unencrypted(data) => data.message_raw_content,
                _ => {
                    return Err(NodeError {
                        message: "Message data is encrypted".into(),
                    })
                }
            },
            _ => {
                return Err(NodeError {
                    message: "Message body is encrypted".into(),
                })
            }
        };
        let last_messages_inbox_request_result: Result<APIGetMessagesFromInboxRequest, _> =
            serde_json::from_str(&content);

        let last_messages_inbox_request = match last_messages_inbox_request_result {
            Ok(request) => request,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to parse GetLastMessagesFromInboxRequest: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let inbox_name_result = InboxName::new(last_messages_inbox_request.inbox.clone());
        let inbox_name = match inbox_name_result {
            Ok(inbox) => inbox,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to parse InboxName: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let count = last_messages_inbox_request.count;
        let offset = last_messages_inbox_request.offset;

        match Self::has_inbox_access(db.clone(), &inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value {
                    let response =
                        Self::internal_get_last_messages_from_inbox(db.clone(), inbox_name.to_string(), count, offset)
                            .await;
                    let processed_response = response_handler(response);
                    let _ = res.send(Ok(processed_response)).await;
                    Ok(())
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to access the inbox: {}",
                                inbox_name
                            ),
                        }))
                        .await;

                    Ok(())
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender_subidentity.get_full_identity_name()
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_get_last_messages_from_inbox(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    ) -> Result<(), NodeError> {
        Self::process_last_messages_from_inbox(
            encryption_secret_key,
            db,
            identity_manager,
            node_name,
            potentially_encrypted_msg,
            res,
            |response| response.into_iter().filter_map(|msg| msg.first().cloned()).collect(),
        )
        .await
    }

    pub async fn api_get_last_messages_from_inbox_with_branches(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<Vec<ShinkaiMessage>>, APIError>>,
    ) -> Result<(), NodeError> {
        Self::process_last_messages_from_inbox(
            encryption_secret_key,
            db,
            identity_manager,
            node_name,
            potentially_encrypted_msg,
            res,
            |response| response,
        )
        .await
    }

    pub async fn api_get_last_unread_messages_from_inbox(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager,
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::APIGetMessagesFromInboxRequest),
        )
        .await;
        let (msg, sender_subidentity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let content = msg.get_message_content()?;
        let last_messages_inbox_request_result: Result<APIGetMessagesFromInboxRequest, _> =
            serde_json::from_str(&content);

        let last_messages_inbox_request = match last_messages_inbox_request_result {
            Ok(request) => request,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to parse GetLastMessagesFromInboxRequest: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let inbox_name_result = InboxName::new(last_messages_inbox_request.inbox.clone());
        let inbox_name = match inbox_name_result {
            Ok(inbox) => inbox,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to parse InboxName: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let count = last_messages_inbox_request.count;
        let offset = last_messages_inbox_request.offset;

        // Check that the message is coming from someone with the right permissions to do this action
        // TODO(Discuss): can local admin read any messages from any device or profile?
        match Self::has_inbox_access(db.clone(), &inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value {
                    let response = Self::internal_get_last_unread_messages_from_inbox(
                        db.clone(),
                        inbox_name.to_string(),
                        count,
                        offset,
                    )
                    .await;
                    let _ = res.send(Ok(response)).await;
                    Ok(())
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to access the inbox: {}",
                                inbox_name
                            ),
                        }))
                        .await;

                    Ok(())
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender_subidentity.get_full_identity_name()
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_create_and_send_registration_code(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager,
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::CreateRegistrationCode),
        )
        .await;
        let (msg, sender) = match validation_result {
            Ok((msg, sender)) => (msg, sender),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        // Check that the message is coming from someone with the right permissions to do this action
        match sender {
            Identity::Standard(std_identity) => {
                if std_identity.permission_type != IdentityPermissions::Admin {
                    return Err(NodeError {
                        message: "Permission denied. Only Admin can perform this operation.".to_string(),
                    });
                }
            }
            Identity::Device(std_device) => {
                if std_device.permission_type != IdentityPermissions::Admin {
                    return Err(NodeError {
                        message: "Permission denied. Only Admin can perform this operation.".to_string(),
                    });
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender
                        )
                        .to_string(),
                    }))
                    .await;
                return Ok(());
            }
        }

        // Parse the message content (message.body.content). it's of type CreateRegistrationCode and continue.
        let content = msg.get_message_content()?;
        let create_registration_code: RegistrationCodeRequest =
            serde_json::from_str(&content).map_err(|e| NodeError {
                message: format!("Failed to parse CreateRegistrationCode: {}", e),
            })?;

        let permissions = create_registration_code.permissions;
        let code_type = create_registration_code.code_type;

        // permissions: IdentityPermissions,
        // code_type: RegistrationCodeType,

        match db.generate_registration_new_code(permissions, code_type) {
            Ok(code) => {
                let _ = res.send(Ok(code)).await.map_err(|_| ());
            }
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to generate registration code: {}", err),
                    }))
                    .await;
            }
        }
        Ok(())
    }

    pub async fn api_create_new_job(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        job_manager: Arc<Mutex<JobManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager,
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::JobCreationSchema),
        )
        .await;
        let (msg, sender_subidentity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // TODO: add permissions to check if the sender has the right permissions to contact the agent
        match Self::internal_create_new_job(job_manager, db, msg, sender_subidentity).await {
            Ok(job_id) => {
                // If everything went well, send the job_id back with an empty string for error
                let _ = res.send(Ok(job_id.clone())).await;
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

    pub async fn api_mark_as_read_up_to(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager,
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::APIReadUpToTimeRequest),
        )
        .await;
        let (msg, sender_subidentity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let content = msg.get_message_content()?;
        let read_up_to_time: APIReadUpToTimeRequest = serde_json::from_str(&content).map_err(|e| NodeError {
            message: format!("Failed to parse APIReadUpToTimeRequest: {}", e),
        })?;

        let inbox_name = read_up_to_time.inbox_name;
        let up_to_time = read_up_to_time.up_to_time;

        // Check that the message is coming from someone with the right permissions to do this action
        // TODO(Discuss): can local admin read any messages from any device or profile?
        match Self::has_inbox_access(db.clone(), &inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value {
                    let response =
                        Self::internal_mark_as_read_up_to(db, inbox_name.to_string(), up_to_time.clone()).await;
                    match response {
                        Ok(true) => {
                            let _ = res.send(Ok("true".to_string())).await;
                            Ok(())
                        }
                        Ok(false) => {
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                    error: "Bad Request".to_string(),
                                    message: format!("Failed to mark as read up to time: {}", up_to_time),
                                }))
                                .await;
                            Ok(())
                        }
                        Err(_e) => {
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::FORBIDDEN.as_u16(),
                                    error: "Don't have access".to_string(),
                                    message: format!(
                                        "Permission denied. You don't have enough permissions to access the inbox: {}",
                                        inbox_name
                                    ),
                                }))
                                .await;
                            Ok(())
                        }
                    }
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to access the inbox: {}",
                                inbox_name
                            ),
                        }))
                        .await;

                    Ok(())
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender_subidentity.get_full_identity_name()
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn api_handle_registration_code_usage(
        db: Arc<SqliteManager>,

        node_name: ShinkaiName,
        encryption_secret_key: EncryptionStaticKey,
        first_device_needs_registration_code: bool,
        _embedding_generator: Arc<RemoteEmbeddingGenerator>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        identity_secret_key: SigningKey,
        initial_llm_providers: Vec<SerializedLLMProvider>,
        public_https_certificate: Option<String>,
        msg: ShinkaiMessage,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        supported_embedding_models: Arc<Mutex<Vec<EmbeddingModelType>>>,
        libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<crate::network::libp2p_manager::NetworkEvent>>,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        eprintln!("api_handle_registration_code_usage");
        let sender_encryption_pk_string = msg.external_metadata.clone().other;
        let sender_encryption_pk = string_to_encryption_public_key(sender_encryption_pk_string.as_str());

        let sender_encryption_pk = match sender_encryption_pk {
            Ok(pk) => pk,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to parse encryption public key: {}", err),
                    }))
                    .await;
                eprintln!("Failed to parse encryption public key: {}", err);
                return Ok(());
            }
        };

        // Decrypt the message
        let message_to_decrypt = msg.clone();

        let decrypted_message_result =
            message_to_decrypt.decrypt_outer_layer(&encryption_secret_key, &sender_encryption_pk);

        let decrypted_message = match decrypted_message_result {
            Ok(message) => message,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to decrypt message: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Deserialize body.content into RegistrationCode
        let content = decrypted_message.get_message_content();
        let content = match content {
            Ok(c) => c,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to get message content: {}", err),
                    }))
                    .await;
                return Ok(());
            }
        };
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Debug,
            format!("Registration code usage content: {}", content).as_str(),
        );

        // let registration_code: RegistrationCode = serde_json::from_str(&content).unwrap();
        let registration_code: Result<RegistrationCode, serde_json::Error> = serde_json::from_str(&content);
        let registration_code = match registration_code {
            Ok(code) => code,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to deserialize the content: {}", err),
                    }))
                    .await;
                return Ok(());
            }
        };

        Self::handle_registration_code_usage(
            db,
            node_name,
            first_device_needs_registration_code,
            _embedding_generator,
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
            res,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn handle_registration_code_usage(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        first_device_needs_registration_code: bool,
        embedding_generator: Arc<RemoteEmbeddingGenerator>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        identity_secret_key: SigningKey,
        initial_llm_providers: Vec<SerializedLLMProvider>,
        registration_code: RegistrationCode,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        supported_embedding_models: Arc<Mutex<Vec<EmbeddingModelType>>>,
        public_https_certificate: Option<String>,
        libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<crate::network::libp2p_manager::NetworkEvent>>,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        eprintln!("handle_registration_code_usage");

        let mut code = registration_code.code;
        let registration_name = registration_code.registration_name;
        let profile_identity_pk = registration_code.profile_identity_pk;
        let profile_encryption_pk = registration_code.profile_encryption_pk;
        let device_identity_pk = registration_code.device_identity_pk;
        let device_encryption_pk = registration_code.device_encryption_pk;
        let identity_type = registration_code.identity_type;
        // Comment (to me): this should be able to handle Device and Agent identities
        // why are we forcing standard_idendity_type?
        // let standard_identity_type = identity_type.to_standard().unwrap();
        let permission_type = registration_code.permission_type;

        // if first_device_registration_needs_code is false
        // then create a new registration code and use it
        // else use the code provided
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!(
                "registration code usage> first device needs registration code?: {:?}",
                first_device_needs_registration_code
            )
            .as_str(),
        );

        let main_profile_exists = match db.main_profile_exists(node_name.get_node_name_string().as_str()) {
            Ok(exists) => exists,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to check if main profile exists: {}", err),
                    }))
                    .await;
                return Ok(());
            }
        };

        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Debug,
            format!(
                "registration code usage> main_profile_exists: {:?}",
                main_profile_exists
            )
            .as_str(),
        );

        if !first_device_needs_registration_code && !main_profile_exists {
            let code_type = RegistrationCodeType::Device("main".to_string());
            let permissions = IdentityPermissions::Admin;

            match db.generate_registration_new_code(permissions, code_type) {
                Ok(new_code) => {
                    code = new_code;
                }
                Err(err) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to generate registration code: {}", err),
                        }))
                        .await;
                }
            }
        }

        let result = db
            .use_registration_code(
                &code.clone(),
                node_name.get_node_name_string().as_str(),
                registration_name.as_str(),
                &profile_identity_pk,
                &profile_encryption_pk,
                Some(&device_identity_pk),
                Some(&device_encryption_pk),
            )
            .map_err(|e| e.to_string())
            .map(|_| "true".to_string());

        // If any new profile has been created using the registration code, we update the VectorFS
        // to initialize the new profile
        let _profile_list: Vec<ShinkaiName> = match db.get_all_profiles(node_name.clone()) {
            Ok(profiles) => profiles.iter().map(|p| p.full_identity_name.clone()).collect(),
            Err(e) => panic!("Failed to fetch profiles: {}", e),
        };
        let _create_default_folders = std::env::var("WELCOME_MESSAGE").unwrap_or("true".to_string()) == "true";
        let _supported_models = {
            let models = supported_embedding_models.lock().await;
            models.clone()
        };
        // TODO: migrate
        // vector_fs
        //     .initialize_new_profiles(
        //         &node_name,
        //         profile_list,
        //         embedding_generator.model_type.clone(),
        //         supported_models,
        //         create_default_folders,
        //     )
        //     .await?;

        match result {
            Ok(success) => {
                match identity_type {
                    IdentityType::Profile | IdentityType::Global => {
                        // Existing logic for handling profile identity
                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();
                        // let full_identity_name = format!("{}/{}", self.node_profile_name.clone(),
                        // profile_name.clone());

                        let full_identity_name_result = ShinkaiName::from_node_and_profile_names(
                            node_name.get_node_name_string(),
                            registration_name.clone(),
                        );

                        if let Err(err) = &full_identity_name_result {
                            error!("Failed to add subidentity: {}", err);
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to add device subidentity: {}", err),
                                }))
                                .await;
                        }

                        let full_identity_name = full_identity_name_result.unwrap();
                        let standard_identity_type = identity_type.to_standard().unwrap();

                        let subidentity = StandardIdentity {
                            full_identity_name,
                            addr: None,
                            profile_signature_public_key: Some(signature_pk_obj),
                            profile_encryption_public_key: Some(encryption_pk_obj),
                            node_encryption_public_key: encryption_public_key,
                            node_signature_public_key: identity_public_key,
                            identity_type: standard_identity_type,
                            permission_type,
                        };

                        let api_v2_key = match db.read_api_v2_key() {
                            Ok(Some(api_key)) => api_key,
                            Ok(None) | Err(_) => {
                                let api_error = APIError {
                                    code: StatusCode::UNAUTHORIZED.as_u16(),
                                    error: "Unauthorized".to_string(),
                                    message: "Invalid bearer token".to_string(),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        };

                        let mut subidentity_manager = identity_manager.lock().await;
                        match subidentity_manager.add_profile_subidentity(subidentity).await {
                            Ok(_) => {
                                std::mem::drop(subidentity_manager);
                                if !first_device_needs_registration_code && !main_profile_exists {
                                    // Call the new function to scan and add Ollama models
                                    if let Err(err) = Self::scan_and_add_ollama_models(
                                        db.clone(),
                                        identity_manager.clone(),
                                        job_manager.clone(),
                                        identity_secret_key.clone(),
                                        node_name.clone(),
                                        ws_manager.clone(),
                                    )
                                    .await
                                    {
                                        error!("Failed to scan and add Ollama models: {}", err);
                                        // Note: We're not failing the entire operation if this fails
                                    }
                                }

                                let success_response = APIUseRegistrationCodeSuccessResponse {
                                    message: success,
                                    node_name: node_name.get_node_name_string().clone(),
                                    encryption_public_key: encryption_public_key_to_string(encryption_public_key),
                                    identity_public_key: signature_public_key_to_string(identity_public_key),
                                    api_v2_key,
                                    api_v2_cert: public_https_certificate,
                                };
                                let _ = res.send(Ok(success_response)).await.map_err(|_| ());
                            }
                            Err(err) => {
                                error!("Failed to add subidentity: {}", err);
                                let _ = res
                                    .send(Err(APIError {
                                        code: StatusCode::BAD_REQUEST.as_u16(),
                                        error: "Internal Server Error".to_string(),
                                        message: format!("Failed to add device subidentity: {}", err),
                                    }))
                                    .await;
                            }
                        }
                    }
                    IdentityType::Device => {
                        // use get_code_info to get the profile name
                        let code_info: shinkai_message_primitives::schemas::identity_registration::RegistrationCodeInfo = db.get_registration_code_info(code.clone().as_str()).unwrap();
                        let profile_name = match code_info.code_type {
                            RegistrationCodeType::Device(profile_name) => profile_name,
                            _ => return Err(Box::new(SqliteManagerError::InvalidData)),
                        };

                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();

                        // Check if the profile exists in the identity_manager
                        {
                            let mut identity_manager = identity_manager.lock().await;
                            let profile_identity_name = ShinkaiName::from_node_and_profile_names(
                                node_name.get_node_name_string(),
                                profile_name.clone(),
                            )
                            .unwrap();
                            if identity_manager
                                .find_by_identity_name(profile_identity_name.clone())
                                .is_none()
                            {
                                // If the profile doesn't exist, create and add it
                                let profile_identity = StandardIdentity {
                                    full_identity_name: profile_identity_name.clone(),
                                    addr: None,
                                    profile_encryption_public_key: Some(encryption_pk_obj),
                                    profile_signature_public_key: Some(signature_pk_obj),
                                    node_encryption_public_key: encryption_public_key,
                                    node_signature_public_key: identity_public_key,
                                    identity_type: StandardIdentityType::Profile,
                                    permission_type: IdentityPermissions::Admin,
                                };
                                identity_manager.add_profile_subidentity(profile_identity).await?;
                            }
                        }

                        // Logic for handling device identity
                        // let full_identity_name = format!("{}/{}", self.node_profile_name.clone(),
                        // profile_name.clone());
                        let full_identity_name = ShinkaiName::from_node_and_profile_names_and_type_and_name(
                            node_name.get_node_name_string(),
                            profile_name,
                            ShinkaiSubidentityType::Device,
                            registration_name.clone(),
                        )
                        .unwrap();

                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();

                        let device_signature_pk_obj =
                            string_to_signature_public_key(device_identity_pk.as_str()).unwrap();
                        let device_encryption_pk_obj =
                            string_to_encryption_public_key(device_encryption_pk.as_str()).unwrap();

                        let device_identity = DeviceIdentity {
                            full_identity_name: full_identity_name.clone(),
                            node_encryption_public_key: encryption_public_key,
                            node_signature_public_key: identity_public_key,
                            profile_encryption_public_key: encryption_pk_obj,
                            profile_signature_public_key: signature_pk_obj,
                            device_encryption_public_key: device_encryption_pk_obj,
                            device_signature_public_key: device_signature_pk_obj,
                            permission_type,
                        };

                        let api_v2_key = match db.read_api_v2_key() {
                            Ok(Some(api_key)) => api_key,
                            Ok(None) | Err(_) => {
                                let api_error = APIError {
                                    code: StatusCode::UNAUTHORIZED.as_u16(),
                                    error: "Unauthorized".to_string(),
                                    message: "Invalid bearer token".to_string(),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        };

                        let mut identity_manager_mut = identity_manager.lock().await;
                        match identity_manager_mut.add_device_subidentity(device_identity).await {
                            Ok(_) => {
                                std::mem::drop(identity_manager_mut);
                                if !main_profile_exists && !initial_llm_providers.is_empty() {
                                    let profile = full_identity_name.extract_profile()?;
                                    for llm_provider in &initial_llm_providers {
                                        Self::internal_add_llm_provider(
                                            db.clone(),
                                            identity_manager.clone(),
                                            job_manager.clone(),
                                            identity_secret_key.clone(),
                                            llm_provider.clone(),
                                            &profile,
                                            ws_manager.clone(),
                                        )
                                        .await?;
                                    }
                                }

                                if !first_device_needs_registration_code && !main_profile_exists {
                                    // Call the new function to scan and add Ollama models
                                    if let Err(err) = Self::scan_and_add_ollama_models(
                                        db.clone(),
                                        identity_manager.clone(),
                                        job_manager.clone(),
                                        identity_secret_key.clone(),
                                        node_name.clone(),
                                        ws_manager.clone(),
                                    )
                                    .await
                                    {
                                        error!("Failed to scan and add Ollama models: {}", err);
                                        // Note: We're not failing the entire operation if this fails
                                    }
                                }

                                let success_response = APIUseRegistrationCodeSuccessResponse {
                                    message: success,
                                    node_name: node_name.get_node_name_string().clone(),
                                    encryption_public_key: encryption_public_key_to_string(encryption_public_key),
                                    identity_public_key: signature_public_key_to_string(identity_public_key),
                                    api_v2_key,
                                    api_v2_cert: public_https_certificate,
                                };
                                let _ = res.send(Ok(success_response)).await.map_err(|_| ());
                            }
                            Err(err) => {
                                error!("Failed to add device subidentity: {}", err);
                                let _ = res
                                    .send(Err(APIError {
                                        code: StatusCode::BAD_REQUEST.as_u16(),
                                        error: "Internal Server Error".to_string(),
                                        message: format!("Failed to add device subidentity: {}", err),
                                    }))
                                    .await;
                            }
                        }
                    }
                    _ => {
                        // Handle other cases if required.
                    }
                }
            }
            Err(err) => {
                error!("Failed to add subidentity: {}", err);
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to add device subidentity: {}", err),
                    }))
                    .await;
            }
        }
        Ok(())
    }

    async fn scan_and_add_ollama_models(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        node_name: ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // if IS_TESTING then don't scan for ollama models
        let is_testing = env::var("IS_TESTING").unwrap_or_default();
        if is_testing == "true" || is_testing == "1" {
            return Ok(());
        }

        // Scan Ollama models
        let ollama_models = match Self::internal_scan_ollama_models().await {
            Ok(models) => models,
            Err(err) => {
                error!("Failed to scan Ollama models: {}", err);
                return Ok(()); // Continue even if scanning fails
            }
        };

        // Add Ollama models if any were found
        if !ollama_models.is_empty() {
            let models_to_add: Vec<String> = ollama_models
                .iter()
                .filter_map(|model| model["name"].as_str().map(String::from))
                .collect();

            if !models_to_add.is_empty() {
                let add_models_result = Self::internal_add_ollama_models(
                    db,
                    identity_manager,
                    job_manager,
                    identity_secret_key,
                    models_to_add,
                    node_name,
                    ws_manager,
                )
                .await;

                if let Err(err) = add_models_result {
                    error!("Failed to add Ollama models: {}", err);
                    // Note: We're not failing the entire operation if adding models fails
                }
            }
        }

        Ok(())
    }

    pub async fn api_update_smart_inbox_name(
        encryption_secret_key: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager,
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::TextContent),
        )
        .await;
        let (msg, sender) = match validation_result {
            Ok((msg, sender)) => (msg, sender),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let new_name: String = msg.get_message_content()?;

        let inbox_name: String = match &msg.body {
            MessageBody::Unencrypted(body) => body.internal_metadata.inbox.clone(),
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Inbox name must be in an unencrypted message.".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        };

        match sender {
            Identity::Standard(std_identity) => {
                if std_identity.permission_type == IdentityPermissions::Admin {
                    match Self::internal_update_smart_inbox_name(db.clone(), inbox_name.clone(), new_name).await {
                        Ok(_) => {
                            if res.send(Ok(())).await.is_err() {
                                let error = APIError {
                                    code: 500,
                                    error: "ChannelSendError".to_string(),
                                    message: "Failed to send data through the channel".to_string(),
                                };
                                let _ = res.send(Err(error)).await;
                            }
                            Ok(())
                        }
                        Err(e) => {
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Failed to update inbox name".to_string(),
                                    message: e,
                                }))
                                .await;
                            Ok(())
                        }
                    }
                } else {
                    let has_permission = db
                        .has_permission(&inbox_name, &std_identity, InboxPermission::Admin)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                    if has_permission {
                        match Self::internal_update_smart_inbox_name(db.clone(), inbox_name, new_name).await {
                            Ok(_) => {
                                if res.send(Ok(())).await.is_err() {
                                    let error = APIError {
                                        code: 500,
                                        error: "ChannelSendError".to_string(),
                                        message: "Failed to send data through the channel".to_string(),
                                    };
                                    let _ = res.send(Err(error)).await;
                                }
                                Ok(())
                            }
                            Err(e) => {
                                let _ = res
                                    .send(Err(APIError {
                                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                        error: "Failed to update inbox name".to_string(),
                                        message: e,
                                    }))
                                    .await;
                                Ok(())
                            }
                        }
                    } else {
                        let _ = res
                            .send(Err(APIError {
                                code: StatusCode::FORBIDDEN.as_u16(),
                                error: "Don't have access".to_string(),
                                message:
                                    "Permission denied. You don't have enough permissions to update this inbox name."
                                        .to_string(),
                            }))
                            .await;
                        Ok(())
                    }
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_get_all_smart_inboxes_for_profile(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<SmartInbox>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::TextContent),
        )
        .await;
        let (msg, sender) = match validation_result {
            Ok((msg, sender)) => (msg, sender),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile_requested: String = msg.get_message_content()?;

        // Check that the message is coming from someone with the right permissions to do this action
        match sender {
            Identity::Standard(std_identity) => {
                // should be safe. previously checked in validate_message
                let sender_profile_name = match std_identity.full_identity_name.get_profile_name_string() {
                    Some(name) => name,
                    None => {
                        let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to see this profile's inboxes list: {}",
                                profile_requested
                            ),
                        }))
                        .await;
                        return Ok(());
                    }
                };

                if (std_identity.permission_type == IdentityPermissions::Admin)
                    || (sender_profile_name == profile_requested)
                {
                    // Get all inboxes for the profile
                    let inboxes = Self::internal_get_all_smart_inboxes_for_profile(
                        db.clone(),
                        identity_manager.clone(),
                        profile_requested,
                    )
                    .await;

                    // Send the result back
                    if res.send(Ok(inboxes)).await.is_err() {
                        let error = APIError {
                            code: 500,
                            error: "ChannelSendError".to_string(),
                            message: "Failed to send data through the channel".to_string(),
                        };
                        let _ = res.send(Err(error)).await;
                    }

                    Ok(())
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to see this profile's inboxes list: {}",
                                profile_requested
                            ),
                        }))
                        .await;

                    Ok(())
                }
            }
            Identity::Device(std_device) => {
                let sender_profile_name = std_device.full_identity_name.get_profile_name_string().unwrap();

                if (std_device.permission_type == IdentityPermissions::Admin)
                    || (sender_profile_name == profile_requested)
                {
                    // Get all inboxes for the profilei
                    let inboxes = Self::internal_get_all_smart_inboxes_for_profile(
                        db.clone(),
                        identity_manager.clone(),
                        profile_requested,
                    )
                    .await;

                    // Send the result back
                    if res.send(Ok(inboxes)).await.is_err() {
                        let error = APIError {
                            code: 500,
                            error: "ChannelSendError".to_string(),
                            message: "Failed to send data through the channel".to_string(),
                        };
                        let _ = res.send(Err(error)).await;
                    }

                    Ok(())
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to see this profile's inboxes list: {}",
                                profile_requested
                            ),
                        }))
                        .await;
                    Ok(())
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_get_all_inboxes_for_profile(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::TextContent),
        )
        .await;
        let (msg, sender) = match validation_result {
            Ok((msg, sender)) => (msg, sender),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile_requested_str: String = msg.get_message_content()?;
        let profile_requested: ShinkaiName = if ShinkaiName::validate_name(&profile_requested_str).is_ok() {
            ShinkaiName::new(profile_requested_str.clone()).map_err(|err| err.to_string())?
        } else {
            ShinkaiName::from_node_and_profile_names(node_name.get_node_name_string(), profile_requested_str.clone())
                .map_err(|err| err.to_string())?
        };

        // Check that the message is coming from someone with the right permissions to do this action
        match sender {
            Identity::Standard(std_identity) => {
                // should be safe. previously checked in validate_message
                let sender_profile_name = match std_identity.full_identity_name.get_profile_name_string() {
                    Some(name) => name,
                    None => {
                        let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to see this profile's inboxes list: {}",
                                profile_requested
                            ),
                        }))
                        .await;
                        return Ok(());
                    }
                };

                if (std_identity.permission_type == IdentityPermissions::Admin)
                    || (sender_profile_name == profile_requested.get_profile_name_string().unwrap_or("".to_string()))
                {
                    // Get all inboxes for the profile
                    let inboxes = Self::internal_get_all_inboxes_for_profile(
                        identity_manager.clone(),
                        db.clone(),
                        profile_requested,
                    )
                    .await;

                    // Send the result back
                    if res.send(Ok(inboxes)).await.is_err() {
                        let error = APIError {
                            code: 500,
                            error: "ChannelSendError".to_string(),
                            message: "Failed to send data through the channel".to_string(),
                        };
                        let _ = res.send(Err(error)).await;
                    }

                    Ok(())
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to see this profile's inboxes list: {}",
                                profile_requested
                            ),
                        }))
                        .await;

                    Ok(())
                }
            }
            Identity::Device(std_device) => {
                let sender_profile_name = std_device.full_identity_name.get_profile_name_string().unwrap();

                if (std_device.permission_type == IdentityPermissions::Admin)
                    || (sender_profile_name == profile_requested.get_profile_name_string().unwrap_or("".to_string()))
                {
                    // Get all inboxes for the profile
                    let inboxes = Self::internal_get_all_inboxes_for_profile(
                        identity_manager.clone(),
                        db.clone(),
                        profile_requested,
                    )
                    .await;

                    // Send the result back
                    if res.send(Ok(inboxes)).await.is_err() {
                        let error = APIError {
                            code: 500,
                            error: "ChannelSendError".to_string(),
                            message: "Failed to send data through the channel".to_string(),
                        };
                        let _ = res.send(Err(error)).await;
                    }

                    Ok(())
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to see this profile's inboxes list: {}",
                                profile_requested
                            ),
                        }))
                        .await;
                    Ok(())
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_update_job_to_finished(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the message
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::APIFinishJob),
        )
        .await;
        let (msg, sender) = match validation_result {
            Ok((msg, sender)) => (msg, sender),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let inbox_name = match InboxName::from_message(&msg.clone()) {
            Ok(inbox_name) => inbox_name,
            _ => {
                let error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Failed to extract inbox name from the message".to_string(),
                };
                let _ = res.send(Err(error)).await;
                return Ok(());
            }
        };

        let job_id = match inbox_name.clone() {
            InboxName::JobInbox { unique_id, .. } => unique_id,
            _ => {
                let error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Expected a JobInbox".to_string(),
                };
                let _ = res.send(Err(error)).await;
                return Ok(());
            }
        };

        // Check that the message is coming from someone with the right permissions to do this action
        match sender {
            Identity::Standard(std_identity) => {
                if std_identity.permission_type == IdentityPermissions::Admin {
                    // Update the job to finished in the database
                    match db.update_job_to_finished(&job_id) {
                        Ok(_) => {
                            let _ = res.send(Ok(())).await;
                            Ok(())
                        }
                        Err(err) => {
                            match err {
                                SqliteManagerError::DataNotFound => {
                                    let _ = res
                                        .send(Err(APIError {
                                            code: StatusCode::BAD_REQUEST.as_u16(),
                                            error: "Bad Request".to_string(),
                                            message: format!("{}", err),
                                        }))
                                        .await;
                                }
                                _ => {
                                    let _ = res
                                        .send(Err(APIError {
                                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                            error: "Internal Server Error".to_string(),
                                            message: format!("{}", err),
                                        }))
                                        .await;
                                }
                            }
                            Ok(())
                        }
                    }
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: "Permission denied. You don't have enough permissions to update this job."
                                .to_string(),
                        }))
                        .await;
                    Ok(())
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_get_all_profiles(
        identity_manager: Arc<Mutex<IdentityManager>>,
        res: Sender<Result<Vec<StandardIdentity>, APIError>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Obtain the IdentityManager lock
        let identity_manager = identity_manager.lock().await;

        // Get all identities (both standard and agent)
        let identities = identity_manager.get_all_subidentities();

        // Filter out only the StandardIdentity instances
        let subidentities: Vec<StandardIdentity> = identities
            .into_iter()
            .filter_map(|identity| {
                if let Identity::Standard(std_identity) = identity {
                    Some(std_identity)
                } else {
                    None
                }
            })
            .collect();

        // Send the result back
        if res.send(Ok(subidentities)).await.is_err() {
            let error = APIError {
                code: 500,
                error: "ChannelSendError".to_string(),
                message: "Failed to send data through the channel".to_string(),
            };
            let _ = res.send(Err(error)).await;
        }

        Ok(())
    }

    pub async fn api_job_message(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        job_manager: Arc<Mutex<JobManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg.clone(),
            Some(MessageSchemaType::JobMessageSchema),
        )
        .await;
        let (msg, sender_identity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        shinkai_log(
            ShinkaiLogOption::DetailedAPI,
            ShinkaiLogLevel::Debug,
            format!("api_job_message> msg: {:?}", msg).as_str(),
        );

        let sender_shinkai_name = sender_identity.get_full_identity_name();
        let sender_node_name = match ShinkaiName::new(sender_shinkai_name) {
            Ok(name) => name.get_node_name_string(),
            Err(_) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Invalid sender identity name.".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        };

        // Check if the sender's node name matches the input node name
        if sender_node_name != node_name.get_node_name_string() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::FORBIDDEN.as_u16(),
                    error: "Don't have access".to_string(),
                    message: "Permission denied. The sender identity does not belong to this node.".to_string(),
                }))
                .await;
            return Ok(());
        }

        match Self::internal_job_message(job_manager, msg.clone(), false).await {
            Ok(_) => {
                let inbox_name = match InboxName::from_message(&msg.clone()) {
                    Ok(inbox) => inbox.to_string(),
                    Err(_) => "".to_string(),
                };

                let scheduled_time = msg.external_metadata.scheduled_time;
                let message_hash = potentially_encrypted_msg.calculate_message_hash_for_pagination();

                let parent_key = if !inbox_name.is_empty() {
                    match db.get_parent_message_hash(&inbox_name, &message_hash) {
                        Ok(result) => result,
                        Err(_) => None,
                    }
                } else {
                    None
                };

                let response = SendResponseBodyData {
                    message_id: message_hash,
                    parent_message_id: parent_key,
                    inbox: inbox_name,
                    scheduled_time,
                };

                // If everything went well, send the job_id back with an empty string for error
                let _ = res.send(Ok(response)).await;
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

    pub async fn api_available_llm_providers(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<SerializedLLMProvider>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::Empty),
        )
        .await;
        let (msg, _) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile = ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone())?
            .get_profile_name_string()
            .ok_or(NodeError {
                message: "Profile name not found".to_string(),
            })?;

        match Self::internal_get_llm_providers_for_profile(db.clone(), node_name.clone().node_name, profile).await {
            Ok(llm_providers) => {
                let _ = res.send(Ok(llm_providers)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn api_scan_ollama_models(
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<serde_json::Value>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::APIScanOllamaModels),
        )
        .await;
        let (_, sender_identity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Convert DeviceIdentity to StandardIdentity if necessary and check if it's a Profile type with admin
        // privileges
        let standard_identity = match sender_identity {
            Identity::Standard(std_identity) => Some(std_identity),
            Identity::Device(device_identity) => device_identity.to_standard_identity(),
            _ => None,
        };

        if let Some(std_identity) = standard_identity {
            let is_profile_type = matches!(std_identity.identity_type, StandardIdentityType::Profile);
            let has_appropriate_privileges = matches!(
                std_identity.permission_type,
                IdentityPermissions::Admin | IdentityPermissions::Standard
            );

            if !is_profile_type || !has_appropriate_privileges {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::UNAUTHORIZED.as_u16(),
                        error: "Unauthorized".to_string(),
                        message: "Sender identity must be a Profile type with admin privileges.".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        } else {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::UNAUTHORIZED.as_u16(),
                    error: "Unauthorized".to_string(),
                    message: "Sender identity is not supported or cannot be converted to a StandardIdentity."
                        .to_string(),
                }))
                .await;
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

    #[allow(clippy::too_many_arguments)]
    pub async fn api_add_ollama_models(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIAddOllamaModels>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::APIAddOllamaModels,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Convert ShinkaiName to StandardIdentity if necessary and check if it's a Profile type with admin privileges
        let identity = identity_manager
            .lock()
            .await
            .search_identity(requester_name.full_name.as_str())
            .await;
        let standard_identity = match identity {
            Some(Identity::Standard(std_identity)) => Some(std_identity),
            Some(Identity::Device(device_identity)) => device_identity.to_standard_identity(),
            _ => None,
        };

        if let Some(std_identity) = standard_identity {
            let is_profile_type = matches!(std_identity.identity_type, StandardIdentityType::Profile);
            let has_appropriate_privileges = matches!(
                std_identity.permission_type,
                IdentityPermissions::Admin | IdentityPermissions::Standard
            );

            if !is_profile_type || !has_appropriate_privileges {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::UNAUTHORIZED.as_u16(),
                        error: "Unauthorized".to_string(),
                        message: "Sender identity must be a Profile type with admin privileges.".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        } else {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::UNAUTHORIZED.as_u16(),
                    error: "Unauthorized".to_string(),
                    message: "Sender identity is not supported or cannot be converted to a StandardIdentity."
                        .to_string(),
                }))
                .await;
            return Ok(());
        }

        match Node::internal_add_ollama_models(
            db,
            identity_manager,
            job_manager,
            identity_secret_key,
            input_payload.models,
            requester_name,
            ws_manager,
        )
        .await
        {
            Ok(_) => {
                let _ = res.send(Ok::<(), APIError>(())).await;
                return Ok(());
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to add model: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn api_add_agent(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::APIAddAgentRequest),
        )
        .await;
        let (msg, sender_identity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // TODO: add permissions to check if the sender has the right permissions to contact the agent
        let serialized_agent_string_result = msg.get_message_content();

        let serialized_agent_string = match serialized_agent_string_result {
            Ok(content) => content,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to get message content: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let serialized_llm_provider_result = serde_json::from_str::<APIAddAgentRequest>(&serialized_agent_string);

        let serialized_llm_provider = match serialized_llm_provider_result {
            Ok(llm_provider) => llm_provider,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to parse APIAddAgentRequest: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile_result = {
            let identity_name = sender_identity.get_full_identity_name();
            ShinkaiName::new(identity_name)
        };

        let profile = match profile_result {
            Ok(profile) => profile,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create profile: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match Self::internal_add_llm_provider(
            db.clone(),
            identity_manager.clone(),
            job_manager.clone(),
            identity_secret_key.clone(),
            serialized_llm_provider.agent,
            &profile,
            ws_manager,
        )
        .await
        {
            Ok(_) => {
                // If everything went well, send the job_id back with an empty string for error
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

    pub async fn api_remove_agent(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::APIRemoveAgentRequest),
        )
        .await;
        let (msg, sender_subidentity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let llm_provider_id_result = msg.get_message_content();

        let llm_provider_id = match llm_provider_id_result {
            Ok(id) => id.to_string(),
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to get agent ID from message: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile = sender_subidentity.get_full_identity_name();
        let profile = match ShinkaiName::new(profile) {
            Ok(profile) => profile,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create profile: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut identity_manager = identity_manager.lock().await;
        match db.remove_llm_provider(&llm_provider_id, &profile) {
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

    pub async fn api_modify_agent(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<SerializedLLMProvider>(
            node_name,
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::APIModifyAgentRequest,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Check if the profile has access to modify the agent
        let profiles_with_access = match db.get_llm_provider_profiles_with_access(&input_payload.id, &requester_name) {
            Ok(access_list) => access_list,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to get profiles with access: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        if !profiles_with_access.contains(&requester_name.get_profile_name_string().unwrap_or_default()) {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::FORBIDDEN.as_u16(),
                    error: "Forbidden".to_string(),
                    message: "Profile does not have access to modify this agent".to_string(),
                }))
                .await;
            Ok(())
        } else {
            // Modify agent based on the input_payload
            match db.update_llm_provider(input_payload.clone(), &requester_name) {
                Ok(_) => {
                    let mut identity_manager = identity_manager.lock().await;
                    match identity_manager.modify_llm_provider_subidentity(input_payload).await {
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
    }

    pub async fn api_change_job_agent(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg.clone(),
            Some(MessageSchemaType::ChangeJobAgentRequest),
        )
        .await;
        let (validated_msg, sender_subidentity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Extract job ID and new agent ID from the message content
        let content = match validated_msg.get_message_content() {
            Ok(content) => content,
            Err(e) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to get message content: {}", e),
                    }))
                    .await;
                return Ok(());
            }
        };

        let change_request: APIChangeJobAgentRequest = match serde_json::from_str(&content) {
            Ok(request) => request,
            Err(e) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to parse APIChangeJobAgentRequest: {}", e),
                    }))
                    .await;
                return Ok(());
            }
        };

        let inbox_name = match InboxName::get_job_inbox_name_from_params(change_request.job_id.clone()) {
            Ok(name) => name.to_string(),
            Err(_) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::FORBIDDEN.as_u16(),
                        error: "Don't have access".to_string(),
                        message: "Permission denied. You don't have enough permissions to change this job agent."
                            .to_string(),
                    }))
                    .await;
                return Ok(());
            }
        };

        // Check if the sender has the right permissions to change the job agent
        match sender_subidentity {
            Identity::Standard(std_identity) => {
                if std_identity.permission_type == IdentityPermissions::Admin {
                    // Attempt to change the job agent in the job manager
                    match db.change_job_llm_provider(&change_request.job_id, &change_request.new_agent_id) {
                        Ok(_) => {
                            let _ = res.send(Ok("Job agent changed successfully".to_string())).await;
                            Ok(())
                        }
                        Err(err) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to change job agent: {}", err),
                            };
                            let _ = res.send(Err(api_error)).await;
                            Ok(())
                        }
                    }
                } else {
                    let has_permission = db
                        .has_permission(&inbox_name, &std_identity, InboxPermission::Admin)
                        .map_err(|e| NodeError {
                            message: format!("Failed to check permissions: {}", e),
                        })?;
                    if has_permission {
                        match db.change_job_llm_provider(&change_request.job_id, &change_request.new_agent_id) {
                            Ok(_) => {
                                let _ = res.send(Ok("Job agent changed successfully".to_string())).await;
                                Ok(())
                            }
                            Err(err) => {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to change job agent: {}", err),
                                };
                                let _ = res.send(Err(api_error)).await;
                                Ok(())
                            }
                        }
                    } else {
                        let _ = res
                            .send(Err(APIError {
                                code: StatusCode::FORBIDDEN.as_u16(),
                                error: "Don't have access".to_string(),
                                message:
                                    "Permission denied. You don't have enough permissions to change this job agent."
                                        .to_string(),
                            }))
                            .await;
                        Ok(())
                    }
                }
            }
            _ => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!(
                            "Invalid identity type. Only StandardIdentity is allowed. Value: {:?}",
                            sender_subidentity
                        )
                        .to_string(),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_create_files_inbox_with_symmetric_key(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the message
        let validation_result = Self::validate_message(
            encryption_secret_key.clone(),
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::SymmetricKeyExchange),
        )
        .await;
        let (msg, _) = match validation_result {
            Ok((msg, identity)) => (msg, identity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Decrypt the message
        let decrypted_msg = match msg.decrypt_outer_layer(&encryption_secret_key, &encryption_public_key) {
            Ok(decrypted) => decrypted,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to decrypt message: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Extract the content of the message
        let content = match decrypted_msg.get_message_content() {
            Ok(content) => content,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to extract message content: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match Self::process_symmetric_key(content, db.clone()).await {
            Ok(_) => {
                let _ = res
                    .send(Ok(
                        "Symmetric key stored and files message inbox created successfully".to_string()
                    ))
                    .await;
                Ok(())
            }
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn process_symmetric_key(content: String, db: Arc<SqliteManager>) -> Result<String, APIError> {
        // Convert the hex string to bytes
        let private_key_bytes = hex::decode(&content).map_err(|_| APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: "Invalid private key".to_string(),
        })?;

        // Convert the Vec<u8> to a [u8; 32]
        let private_key_array: [u8; 32] = private_key_bytes.try_into().map_err(|_| APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: "Failed to convert private key to array".to_string(),
        })?;

        // Calculate the hash of it using blake3 which will act as a sort of public identifier
        let mut hasher = Hasher::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        let hash_hex = hex::encode(result.as_bytes());

        // Lock the database and perform operations

        // Write the symmetric key to the database
        db.write_symmetric_key(&hash_hex, &private_key_array)
            .map_err(|err| APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("{}", err),
            })?;

        Ok(hash_hex)
    }

    pub async fn api_is_pristine(db: Arc<SqliteManager>, res: Sender<Result<bool, APIError>>) -> Result<(), NodeError> {
        let has_any_profile = db.has_any_profile().unwrap_or(false);
        let _ = res.send(Ok(!has_any_profile)).await;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn api_search_shinkai_tool(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        tool_router: Option<Arc<ToolRouter>>,
        potentially_encrypted_msg: ShinkaiMessage,
        _embedding_generator: Arc<RemoteEmbeddingGenerator>,
        res: Sender<Result<JsonValue, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the message
        let (search_query, requester_name) = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::SearchShinkaiTool,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Start the timer
        let start_time = Instant::now();

        // Perform the internal search using tool_router
        if let Some(tool_router) = tool_router {
            match tool_router.vector_search_all_tools(&search_query, 5).await {
                Ok(tools) => {
                    let tools_json = serde_json::to_value(tools).map_err(|err| NodeError {
                        message: format!("Failed to serialize tools: {}", err),
                    })?;
                    // Log the elapsed time if LOG_ALL is set to 1
                    if std::env::var("LOG_ALL").unwrap_or_default() == "1" {
                        let elapsed_time = start_time.elapsed();
                        let result_count = tools_json.as_array().map_or(0, |arr| arr.len());
                        println!("Time taken for tool search: {:?}", elapsed_time);
                        println!("Number of tool results: {}", result_count);
                    }
                    let _ = res.send(Ok(tools_json)).await;
                    Ok(())
                }
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to search tools: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    Ok(())
                }
            }
        } else {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: "Tool router is not available".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            Ok(())
        }
    }

    pub async fn api_list_all_shinkai_tools(
        sqlite_manager: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<serde_json::Value>, APIError>>,
    ) -> Result<(), NodeError> {
        let requester_name = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::ListAllShinkaiTools,
        )
        .await
        {
            Ok((_, requester_name)) => requester_name,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // List all Shinkai tools
        let tools = {
            match sqlite_manager.get_all_tool_headers() {
                Ok(tools) => tools,
                Err(err) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to list tools: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        let response = tools.into_iter().map(|tool| json!(tool)).collect();
        let _ = res.send(Ok(response)).await;
        Ok(())
    }

    pub async fn api_set_shinkai_tool(
        sqlite_manager: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        tool_router_key: String,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_value, requester_name) = match Self::validate_and_extract_payload::<serde_json::Value>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::SetShinkaiTool,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Get the full tool from lance_db
        let existing_tool = match sqlite_manager.get_tool_by_key(&tool_router_key) {
            Ok(tool) => tool,
            Err(SqliteManagerError::ToolNotFound(_)) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Tool not found in LanceShinkaiDb".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Convert existing_tool to Value
        let existing_tool_value = match serde_json::to_value(&existing_tool) {
            Ok(value) => value,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert existing tool to Value: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Merge existing_tool_value with input_value
        let merged_value = Self::merge_json(existing_tool_value, input_value);

        // Convert merged_value to ShinkaiTool
        let merged_tool: ShinkaiTool = match serde_json::from_value(merged_value) {
            Ok(tool) => tool,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert merged Value to ShinkaiTool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Save the tool to the LanceShinkaiDb
        let save_result = { sqlite_manager.update_tool(merged_tool).await };

        match save_result {
            Ok(_) => {
                // Fetch the updated tool from the database
                let updated_tool = { sqlite_manager.get_tool_by_key(&tool_router_key) };

                match updated_tool {
                    Ok(tool) => {
                        let response = serde_json::to_value(tool).unwrap_or_else(|_| json!({}));
                        let _ = res.send(Ok(response)).await;
                        Ok(())
                    }
                    Err(SqliteManagerError::ToolNotFound(_)) => {
                        let api_error = APIError {
                            code: StatusCode::NOT_FOUND.as_u16(),
                            error: "Not Found".to_string(),
                            message: "Tool not found in LanceShinkaiDb".to_string(),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to fetch tool from LanceShinkaiDb: {}", err),
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
                    message: format!("Failed to set tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_get_shinkai_tool(
        sqlite_manager: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        let (tool_key, requester_name) = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::GetShinkaiTool,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Fetch the tool from the LanceShinkaiDb
        let tool = match sqlite_manager.get_tool_by_key(&tool_key) {
            Ok(tool) => tool,
            Err(SqliteManagerError::ToolNotFound(_)) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Tool not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to fetch tool: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let response = json!(tool);
        let _ = res.send(Ok(response)).await;
        Ok(())
    }

    pub async fn api_update_default_embedding_model(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (new_default_model_str, requester_name) = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::UpdateDefaultEmbeddingModel,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Convert the string to EmbeddingModelType
        let new_default_model = match EmbeddingModelType::from_string(&new_default_model_str) {
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

    #[allow(clippy::too_many_arguments)]
    pub async fn api_change_nodes_name(
        secret_file_path: &str,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        encryption_public_key: EncryptionPublicKey,
        identity_public_key: VerifyingKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = Self::validate_message(
            encryption_secret_key.clone(),
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg,
            Some(MessageSchemaType::ChangeNodesName),
        )
        .await;
        let msg = match validation_result {
            Ok((msg, _)) => msg,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Decrypt the message
        let decrypted_msg = msg.decrypt_outer_layer(&encryption_secret_key.clone(), &encryption_public_key.clone())?;

        // Extract the content of the message
        let new_node_name = decrypted_msg.get_message_content()?;

        // Check that new_node_name is valid
        let new_node_name = match ShinkaiName::from_node_name(new_node_name) {
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

    #[allow(clippy::too_many_arguments)]
    pub async fn api_handle_send_onionized_message(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        identity_secret_key: SigningKey,
        potentially_encrypted_msg: ShinkaiMessage,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<crate::network::libp2p_manager::NetworkEvent>>,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        // This command is used to send messages that are already signed and (potentially) encrypted
        if potentially_encrypted_msg.external_metadata.recipient.starts_with("@@localhost.") {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Invalid recipient node name: @@localhost".to_string(),
                }))
                .await;
            return Ok(());
        }

        let validation_result = Self::validate_message(
            encryption_secret_key.clone(),
            identity_manager.clone(),
            &node_name,
            potentially_encrypted_msg.clone(),
            None,
        )
        .await;
        let (mut msg, _) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Error validating message: {}", api_error.message),
                    }))
                    .await;
                return Ok(());
            }
        };
        //
        // Part 2: Check if the message needs to be sent to another node or not
        //
        let recipient_node_name = ShinkaiName::from_shinkai_message_only_using_recipient_node_name(&msg.clone())
            .unwrap()
            .get_node_name_string();

        let sender_node_name = ShinkaiName::from_shinkai_message_only_using_sender_node_name(&msg.clone())
            .unwrap()
            .get_node_name_string();

        if recipient_node_name == sender_node_name {
            //
            // Part 3A: Validate and store message locally
            //

            // Has sender access to the inbox specified in the message?
            let inbox = InboxName::from_message(&msg.clone());
            match inbox {
                Ok(inbox) => {
                    // TODO: extend and verify that the sender may have access to the inbox using the access db method
                    match inbox.has_sender_creation_access(msg.clone()) {
                        Ok(_) => {
                            // use unsafe_insert_inbox_message because we already validated the message
                            let parent_message_id = match msg.get_message_parent_key() {
                                Ok(key) => Some(key),
                                Err(_) => None,
                            };

                            db.unsafe_insert_inbox_message(&msg.clone(), parent_message_id, ws_manager.clone())
                                .await
                                .map_err(|e| {
                                    shinkai_log(
                                        ShinkaiLogOption::DetailedAPI,
                                        ShinkaiLogLevel::Error,
                                        format!("Error inserting message into db: {}", e).as_str(),
                                    );
                                    std::io::Error::new(std::io::ErrorKind::Other, format!("Insertion error: {}", e))
                                })?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::DetailedAPI,
                                ShinkaiLogLevel::Error,
                                format!("Error checking if sender has access to inbox: {}", e).as_str(),
                            );
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                    error: "Bad Request".to_string(),
                                    message: format!("Error checking if sender has access to inbox: {}", e),
                                }))
                                .await;
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("handle_onionized_message > Error getting inbox from message: {}", e);
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Error getting inbox from message: {}", e),
                        }))
                        .await;
                    return Ok(());
                }
            }
        }

        //
        // Part 3B: Preparing to externally send Message
        //
        // By default we encrypt all the messages between nodes. So if the message is not encrypted do it
        // we know the node that we want to send the message to from the recipient profile name
        let recipient_node_name_string = ShinkaiName::from_shinkai_message_only_using_recipient_node_name(&msg.clone())
            .unwrap()
            .to_string();

        let external_global_identity_result = identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&recipient_node_name_string.clone(), None)
            .await;

        let external_global_identity = match external_global_identity_result {
            Ok(identity) => identity,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Error".to_string(),
                        message: err,
                    }))
                    .await;
                return Ok(());
            }
        };

        msg.encryption = EncryptionMethod::DiffieHellmanChaChaPoly1305;
        let encrypted_msg = msg.encrypt_outer_layer(
            &encryption_secret_key.clone(),
            &external_global_identity.node_encryption_public_key,
        )?;

        // We update the signature so it comes from the node and not the profile
        // that way the recipient will be able to verify it
        let signature_sk = clone_signature_secret_key(&identity_secret_key);
        let encrypted_msg = encrypted_msg.sign_outer_layer(&signature_sk)?;
        let node_addr = external_global_identity.addr.unwrap();

        Node::send(
            encrypted_msg,
            Arc::new(clone_static_secret_key(&encryption_secret_key)),
            (node_addr, recipient_node_name_string),
            proxy_connection_info,
            db.clone(),
            identity_manager.clone(),
            ws_manager.clone(),
            true,
            None,
            libp2p_event_sender, // Pass the libp2p_event_sender for LibP2P networking
        );

        {
            let inbox_name = match InboxName::from_message(&msg.clone()) {
                Ok(inbox) => inbox.to_string(),
                Err(_) => "".to_string(),
            };

            let scheduled_time = msg.external_metadata.scheduled_time;
            let message_hash = potentially_encrypted_msg.calculate_message_hash_for_pagination();            

            let parent_key = if !inbox_name.is_empty() {
                match db.get_parent_message_hash(&inbox_name, &message_hash) {
                    Ok(result) => result,
                    Err(_) => None,
                }
            } else {
                None
            };

            let response = SendResponseBodyData {
                message_id: message_hash,
                parent_message_id: parent_key,
                inbox: inbox_name,
                scheduled_time,
            };

            if res.send(Ok(response)).await.is_err() {
                eprintln!("Failed to send response");
            }
        }

        Ok(())
    }
}
