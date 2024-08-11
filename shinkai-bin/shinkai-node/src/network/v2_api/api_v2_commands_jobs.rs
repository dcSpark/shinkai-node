use std::sync::Arc;

use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        llm_providers::serialized_llm_provider::SerializedLLMProvider,
        shinkai_name::{ShinkaiName, ShinkaiSubidentityType},
    },
    shinkai_message::shinkai_message_schemas::{APIChangeJobAgentRequest, JobCreationInfo, JobMessage, MessageSchemaType, V2ChatMessage},
};

use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::{
    db::ShinkaiDB,
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{
        node_api_router::{APIError, SendResponseBodyData},
        node_error::NodeError,
        Node,
    },
    schemas::{
        identity::Identity,
        smart_inbox::{SmartInbox, V2SmartInbox},
    },
    vector_fs::vector_fs::VectorFS,
};

use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub fn convert_smart_inbox_to_v2_smart_inbox(smart_inbox: SmartInbox) -> Result<V2SmartInbox, NodeError> {
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

    pub async fn v2_update_smart_inbox_name(
        db: Arc<ShinkaiDB>,
        bearer: String,
        inbox_name: String,
        custom_name: String,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Update the smart inbox name
        match db.update_smart_inbox_name(&inbox_name, &custom_name) {
            Ok(_) => {
                let _ = res.send(Ok(())).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update inbox name: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_create_files_inbox(
        db: Arc<ShinkaiDB>,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), APIError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let hash_hex = uuid::Uuid::new_v4().to_string();

        match db.create_files_message_inbox(hash_hex.clone()) {
            Ok(_) => {
                if let Err(_) = res.send(Ok(hash_hex)).await {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: "Failed to send response".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                }
                Ok(())
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to create files message inbox: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_add_file_to_inbox(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        file_inbox_name: String,
        filename: String,
        file: Vec<u8>,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), APIError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match vector_fs
            .db
            .add_file_to_files_message_inbox(file_inbox_name, filename, file)
        {
            Ok(_) => {
                let _ = res.send(Ok("File added successfully".to_string())).await;
                Ok(())
            }
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to add file to inbox: {}", err),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_change_job_llm_provider(
        db: Arc<ShinkaiDB>,
        bearer: String,
        payload: APIChangeJobAgentRequest,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Extract job ID and new agent ID from the payload
        let change_request = payload;

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
    }
}
