use std::sync::Arc;

use async_channel::Sender;
use ed25519_dalek::{SigningKey, VerifyingKey};
use reqwest::StatusCode;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName, llm_providers::serialized_llm_provider::SerializedLLMProvider, shinkai_name::ShinkaiName,
    },
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{IdentityPermissions, JobMessage, MessageSchemaType, V2ChatMessage},
    },
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, EncryptionMethod},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::signature_public_key_to_string,
    },
};
use shinkai_vector_resources::{
    embedding_generator::RemoteEmbeddingGenerator, model_type::EmbeddingModelType, shinkai_time::ShinkaiStringTime,
};
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::{
    db::ShinkaiDB,
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{
        node_api_router::{APIError, GetPublicKeysResponse},
        node_error::NodeError,
        v1_api::api_v1_handlers::APIUseRegistrationCodeSuccessResponse,
        ws_manager::WSUpdateHandler,
        Node,
    },
    schemas::identity::{IdentityType, RegistrationCode},
    vector_fs::vector_fs::VectorFS,
};

use x25519_dalek::StaticSecret as EncryptionStaticKey;

use super::api_v2_handlers_general::InitialRegistrationRequest;

impl Node {
    pub async fn validate_bearer_token<T>(
        _bearer: &str,
        _db: Arc<ShinkaiDB>,
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
}
