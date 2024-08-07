use std::sync::Arc;

use async_channel::Sender;
use ed25519_dalek::{SigningKey, VerifyingKey};
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        llm_providers::serialized_llm_provider::SerializedLLMProvider,
        shinkai_name::{ShinkaiName, ShinkaiSubidentityType},
    },
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{APIVecFsRetrievePathSimplifiedJson, IdentityPermissions, JobCreationInfo, JobMessage, MessageSchemaType, V2ChatMessage},
    },
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, EncryptionMethod},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::signature_public_key_to_string,
    },
};
use shinkai_vector_resources::{
    embedding_generator::RemoteEmbeddingGenerator, model_type::EmbeddingModelType, shinkai_time::ShinkaiStringTime, vector_resource::VRPath,
};
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::{
    db::ShinkaiDB,
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{
        node_api_router::{APIError, GetPublicKeysResponse, SendResponseBodyData},
        node_error::NodeError,
        subscription_manager::external_subscriber_manager::{ExternalSubscriberManager, SharedFolderInfo},
        v1_api::api_v1_handlers::APIUseRegistrationCodeSuccessResponse,
        ws_manager::WSUpdateHandler,
        Node,
    },
    schemas::{
        identity::{Identity, IdentityType, RegistrationCode},
        smart_inbox::{SmartInbox, V2SmartInbox},
    },
    vector_fs::vector_fs::VectorFS,
};

use super::api_v2_router::InitialRegistrationRequest;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    async fn validate_bearer_token<T>(
        bearer: &str,
        db: Arc<ShinkaiDB>,
        res: &Sender<Result<T, APIError>>,
    ) -> Result<(), ()> {
        // Placeholder implementation that always returns true
        // In a real implementation, you would validate the token
        if true {
            Ok(())
        } else {
            let api_error = APIError {
                code: StatusCode::UNAUTHORIZED.as_u16(),
                error: "Unauthorized".to_string(),
                message: "Invalid bearer token".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            Err(())
        }
    }

    fn convert_shinkai_message_to_v2_chat_message(shinkai_message: ShinkaiMessage) -> Result<V2ChatMessage, NodeError> {
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

    fn convert_shinkai_messages_to_v2_chat_messages(
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

    fn convert_smart_inbox_to_v2_smart_inbox(smart_inbox: SmartInbox) -> Result<V2SmartInbox, NodeError> {
        let last_message = match smart_inbox.last_message {
            Some(msg) => Some(Node::convert_shinkai_message_to_v2_chat_message(msg)?),
            None => None,
        };

        Ok(V2SmartInbox {
            inbox_id: smart_inbox.inbox_id,
            custom_name: smart_inbox.custom_name,
            datetime_created: smart_inbox.datetime_created,
            last_message,
            is_finished: smart_inbox.is_finished,
            job_scope: smart_inbox.job_scope,
            agent: smart_inbox.agent,
        })
    }

    fn api_v2_create_shinkai_message(
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
        let sender_subidentity = sender
            .get_fullname_string_without_node_name()
            .ok_or("Failed to get sender subidentity")?;
        let receiver_subidentity = receiver
            .get_fullname_string_without_node_name()
            .ok_or("Failed to get receiver subidentity")?;

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
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        payload: InitialRegistrationRequest,
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

    pub async fn v2_create_new_job(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        bearer: String,
        job_creation_info: JobCreationInfo,
        llm_provider: String,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token and extract the sender identity
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the main identity from the identity manager
        let main_identity = {
            let identity_manager = identity_manager.lock().await;
            match identity_manager.get_main_identity() {
                Some(identity) => identity.clone(),
                None => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to get main identity".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Create a new job message
        let sender = match ShinkaiName::new(main_identity.get_full_identity_name()) {
            Ok(name) => name,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create sender name: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let recipient = match ShinkaiName::from_node_and_profile_names_and_type_and_name(
            node_name.node_name,
            "main".to_string(),
            ShinkaiSubidentityType::Agent,
            llm_provider,
        ) {
            Ok(name) => name,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create recipient name: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let shinkai_message = match Self::api_v2_create_shinkai_message(
            sender,
            recipient,
            &serde_json::to_string(&job_creation_info).unwrap(),
            MessageSchemaType::JobCreationSchema,
            node_encryption_sk,
            node_signing_sk,
            node_encryption_pk,
            None,
        ) {
            Ok(message) => message,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create Shinkai message: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Process the job creation
        match Self::internal_create_new_job(job_manager, db, shinkai_message, main_identity.clone()).await {
            Ok(job_id) => {
                let _ = res.send(Ok(job_id)).await;
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

    pub async fn v2_job_message(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        bearer: String,
        job_message: JobMessage,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token and extract the sender identity
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the main identity from the identity manager
        let main_identity = {
            let identity_manager = identity_manager.lock().await;
            match identity_manager.get_main_identity() {
                Some(identity) => identity.clone(),
                None => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to get main identity".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Retrieve the job to get the llm_provider
        let llm_provider = match db.get_job(&job_message.job_id) {
            Ok(job) => job.parent_llm_provider_id.clone(),
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve job: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Create a new job message
        let sender = match ShinkaiName::new(main_identity.get_full_identity_name()) {
            Ok(name) => name,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create sender name: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let recipient = match ShinkaiName::from_node_and_profile_names_and_type_and_name(
            node_name.node_name,
            "main".to_string(),
            ShinkaiSubidentityType::Agent,
            llm_provider,
        ) {
            Ok(name) => name,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create recipient name: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let shinkai_message = match Self::api_v2_create_shinkai_message(
            sender,
            recipient,
            &serde_json::to_string(&job_message).unwrap(),
            MessageSchemaType::JobMessageSchema,
            node_encryption_sk,
            node_signing_sk,
            node_encryption_pk,
            Some(job_message.job_id.clone()),
        ) {
            Ok(message) => message,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create Shinkai message: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Process the job message
        match Self::internal_job_message(job_manager, shinkai_message.clone()).await {
            Ok(_) => {
                let inbox_name = match InboxName::get_job_inbox_name_from_params(job_message.job_id) {
                    Ok(inbox) => inbox.to_string(),
                    Err(_) => "".to_string(),
                };

                let scheduled_time = shinkai_message.clone().external_metadata.scheduled_time;
                let message_hash = shinkai_message.calculate_message_hash_for_pagination();

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

    pub async fn v2_get_last_messages_from_inbox(
        db: Arc<ShinkaiDB>,
        bearer: String,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Result<Vec<V2ChatMessage>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the last messages from the inbox
        let messages = match db.get_last_messages_from_inbox(inbox_name.clone(), limit, offset_key.clone()) {
            Ok(messages) => messages,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve messages: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Convert the retrieved messages to V2ChatMessage
        let v2_chat_messages = match Self::convert_shinkai_messages_to_v2_chat_messages(messages) {
            Ok(v2_messages) => v2_messages.into_iter().filter_map(|msg| msg.first().cloned()).collect(),
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert messages: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Send the converted messages back to the requester
        if let Err(_) = res.send(Ok(v2_chat_messages)).await {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: "Failed to send messages".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
        }

        Ok(())
    }

    pub async fn v2_get_all_smart_inboxes(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        res: Sender<Result<Vec<V2SmartInbox>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the main identity from the identity manager
        let main_identity = {
            let identity_manager = identity_manager.lock().await;
            match identity_manager.get_main_identity() {
                Some(Identity::Standard(identity)) => identity.clone(),
                _ => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to get main identity".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        };

        // Retrieve all smart inboxes for the profile
        let smart_inboxes = match db.get_all_smart_inboxes_for_profile(main_identity) {
            Ok(inboxes) => inboxes,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve smart inboxes: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Convert SmartInbox to V2SmartInbox
        let v2_smart_inboxes: Result<Vec<V2SmartInbox>, NodeError> = smart_inboxes
            .into_iter()
            .map(Self::convert_smart_inbox_to_v2_smart_inbox)
            .collect();

        match v2_smart_inboxes {
            Ok(inboxes) => {
                let _ = res.send(Ok(inboxes)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert smart inboxes: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_get_available_llm_providers(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        bearer: String,
        res: Sender<Result<Vec<SerializedLLMProvider>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve all LLM providers for the profile
        match Self::internal_get_llm_providers_for_profile(db.clone(), node_name.node_name, "main".to_string()).await {
            Ok(llm_providers) => {
                let _ = res.send(Ok(llm_providers)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve LLM providers: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_vec_fs_retrieve_path_simplified_json(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsRetrievePathSimplifiedJson,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
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

        let vr_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let reader = match vector_fs
            .new_reader(requester_name.clone(), vr_path, requester_name.clone())
            .await
        {
            Ok(reader) => reader,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create reader: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let result = vector_fs.retrieve_fs_path_simplified_json_value(&reader).await;

        fn add_shared_folder_info(obj: &mut serde_json::Value, shared_folders: &[SharedFolderInfo]) {
            if let Some(path) = obj.get("path") {
                if let Some(path_str) = path.as_str() {
                    if let Some(shared_folder) = shared_folders.iter().find(|sf| sf.path == path_str) {
                        let mut shared_folder_info = serde_json::to_value(shared_folder).unwrap();
                        if let Some(obj) = shared_folder_info.as_object_mut() {
                            obj.remove("tree");
                        }
                        obj.as_object_mut().unwrap().insert(
                            "shared_folder_info".to_string(),
                            serde_json::to_value(shared_folder).unwrap(),
                        );
                    }
                }
            }

            if let Some(child_folders) = obj.get_mut("child_folders") {
                if let Some(child_folders_array) = child_folders.as_array_mut() {
                    for child_folder in child_folders_array {
                        add_shared_folder_info(child_folder, shared_folders);
                    }
                }
            }
        }

        match result {
            Ok(mut result_value) => {
                let mut subscription_manager = ext_subscription_manager.lock().await;
                let shared_folders_result = subscription_manager
                    .available_shared_folders(
                        requester_name.extract_node(),
                        requester_name.get_profile_name_string().unwrap_or_default(),
                        requester_name.extract_node(),
                        requester_name.get_profile_name_string().unwrap_or_default(),
                        input_payload.path,
                    )
                    .await;
                drop(subscription_manager);

                if let Ok(shared_folders) = shared_folders_result {
                    add_shared_folder_info(&mut result_value, &shared_folders);
                }

                let _ = res.send(Ok(result_value)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve fs path json: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }
}
