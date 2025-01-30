use std::{collections::HashMap, sync::Arc, usize};

use async_channel::Sender;
use ed25519_dalek::SigningKey;
use reqwest::StatusCode;
use serde_json::{json, Value};

use shinkai_http_api::node_api_router::{APIError, SendResponseBody, SendResponseBodyData};
use shinkai_message_primitives::{
    schemas::{
        identity::Identity,
        inbox_name::InboxName,
        job::{ForkedJob, JobLike},
        job_config::JobConfig,
        llm_providers::serialized_llm_provider::SerializedLLMProvider,
        shinkai_name::{ShinkaiName, ShinkaiSubidentityType},
        smart_inbox::{SmartInbox, V2SmartInbox},
    },
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData},
        shinkai_message_schemas::{
            APIChangeJobAgentRequest, ExportInboxMessagesFormat, JobCreationInfo, JobMessage, MessageSchemaType,
            V2ChatMessage,
        },
    },
    shinkai_utils::{
        job_scope::MinimalJobScope, shinkai_message_builder::ShinkaiMessageBuilder, shinkai_path::ShinkaiPath,
        signatures::clone_signature_secret_key,
    },
};

use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::{
    llm_provider::job_manager::JobManager,
    managers::IdentityManager,
    network::{node_error::NodeError, Node},
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
            job_config: smart_inbox.job_config,
            provider_type: smart_inbox.provider_type,
        })
    }

    pub async fn v2_create_new_job(
        db: Arc<SqliteManager>,
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
        db: Arc<SqliteManager>,
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
        let llm_provider = match db.get_job_with_options(&job_message.job_id, false) {
            Ok(job) => job.parent_agent_or_llm_provider_id.clone(),
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
        db: Arc<SqliteManager>,
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

    pub async fn v2_get_last_messages_from_inbox_with_branches(
        db: Arc<SqliteManager>,
        bearer: String,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Result<Vec<Vec<V2ChatMessage>>, APIError>>,
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

        // Convert the retrieved messages to Vec<Vec<V2ChatMessage>>
        let v2_chat_messages = match Self::convert_shinkai_messages_to_v2_chat_messages(messages) {
            Ok(v2_messages) => v2_messages,
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
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        _limit: Option<usize>,
        _offset: Option<String>,
        show_hidden: Option<bool>,
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

        // Retrieve all smart inboxes for the profile with pagination
        let smart_inboxes = match db.get_all_smart_inboxes_for_profile(main_identity, show_hidden) {
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

    pub async fn v2_get_all_smart_inboxes_paginated(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        limit: Option<usize>,
        offset: Option<String>,
        show_hidden: Option<bool>,
        res: Sender<Result<serde_json::Value, APIError>>,
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

        // Retrieve all smart inboxes for the profile with pagination
        let paginated_inboxes =
            match db.get_all_smart_inboxes_for_profile_with_pagination(main_identity, limit, offset, show_hidden) {
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
        let v2_smart_inboxes: Result<Vec<V2SmartInbox>, NodeError> = paginated_inboxes
            .inboxes
            .into_iter()
            .map(Self::convert_smart_inbox_to_v2_smart_inbox)
            .collect();

        match v2_smart_inboxes {
            Ok(inboxes) => {
                let response = json!({
                    "inboxes": inboxes,
                    "hasNextPage": paginated_inboxes.has_next_page
                });
                let _ = res.send(Ok(response)).await;
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
        db: Arc<SqliteManager>,
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
        db: Arc<SqliteManager>,
        bearer: String,
        inbox_name: String,
        custom_name: String,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Parse the inbox name to check if it's a job inbox
        let inbox = match InboxName::new(inbox_name.clone()) {
            Ok(inbox) => inbox,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to parse inbox name: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Get the job ID if it's a job inbox
        if let Some(job_id) = inbox.get_job_id() {
            // Get the current folder name before updating
            let old_folder = match db.get_job_folder_name(&job_id) {
                Ok(folder) => folder,
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to get old folder name: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

            // Update the inbox name
            if let Err(e) = db.unsafe_update_smart_inbox_name(&inbox_name, &custom_name) {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update inbox name: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            // Get the new folder name after updating
            let new_folder = match db.get_job_folder_name(&job_id) {
                Ok(folder) => folder,
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to get new folder name: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

            // Move the folder if it exists
            if old_folder.exists() {
                use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
                if let Err(e) = ShinkaiFileManager::move_folder(old_folder, new_folder, &db) {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to move folder: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }

            let _ = res.send(Ok(())).await;
        } else {
            // If it's not a job inbox, just update the name
            match db.unsafe_update_smart_inbox_name(&inbox_name, &custom_name) {
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
        }

        Ok(())
    }

    pub async fn v2_api_change_job_llm_provider(
        db: Arc<SqliteManager>,
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

    pub async fn v2_api_update_job_config(
        db: Arc<SqliteManager>,
        bearer: String,
        job_id: String,
        config: JobConfig,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Check if the job exists
        match db.get_job_with_options(&job_id, false) {
            Ok(_) => {
                // Job exists, proceed with updating the config
                match db.update_job_config(&job_id, config) {
                    Ok(_) => {
                        let success_message = format!("Job config updated successfully for job ID: {}", job_id);
                        let _ = res.send(Ok(success_message)).await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to update job config: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Job with ID {} not found", job_id),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_job_config(
        db: Arc<SqliteManager>,
        bearer: String,
        job_id: String,
        res: Sender<Result<JobConfig, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // TODO: Get default values for Ollama

        // Check if the job exists
        match db.get_job_with_options(&job_id, false) {
            Ok(job) => {
                let config = job.config().cloned().unwrap_or_else(|| JobConfig {
                    custom_system_prompt: None,
                    custom_prompt: None,
                    temperature: None,
                    seed: None,
                    top_k: None,
                    top_p: None,
                    stream: None,
                    max_tokens: None,
                    other_model_params: None,
                    use_tools: None,
                });
                let _ = res.send(Ok(config)).await;
                Ok(())
            }
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Job with ID {} not found", job_id),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_retry_message(
        db: Arc<SqliteManager>,
        job_manager: Arc<Mutex<JobManager>>,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
        bearer: String,
        inbox_name: String,
        message_id: String,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the message from the inbox
        let message = match db.fetch_message_and_hash(&message_id) {
            Ok(msg) => msg,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Message not found: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let sender = match ShinkaiName::from_shinkai_message_using_sender_subidentity(&message.0) {
            Ok(sender) => sender,
            Err(_) => {
                let sender = match ShinkaiName::from_shinkai_message_only_using_sender_node_name(&message.0) {
                    Ok(sender) => sender,
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to get sender: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };
                sender
            }
        };

        // Check if the message is from the agent
        if !(sender.has_profile() == false || sender.has_profile() && sender.has_agent()) {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Message is not from the agent".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Retrieve the parent message
        let parent_message_hash = match db.get_parent_message_hash(&inbox_name, &message_id) {
            Ok(parent_message) => match parent_message {
                Some(hash) => hash,
                None => {
                    let api_error = APIError {
                        code: StatusCode::NOT_FOUND.as_u16(),
                        error: "Not Found".to_string(),
                        message: "Parent message not found".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            },
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get parent message: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let original_message = match db.fetch_message_and_hash(&parent_message_hash) {
            Ok(msg) => msg.0,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Parent message not found: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let node_name = ShinkaiName::from_shinkai_message_only_using_sender_node_name(&original_message)
            .unwrap()
            .to_string();

        let sender = match ShinkaiName::from_node_and_profile_names(node_name, "main".to_string()) {
            Ok(sender) => sender,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to get sender: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let recipient = match ShinkaiName::from_shinkai_message_only_using_recipient_node_name(&original_message) {
            Ok(recipient) => recipient,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to get recipient: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Extract Job ID from the inbox_name
        let job_id = match InboxName::from_message(&original_message) {
            Ok(inbox_name) => match inbox_name.get_job_id() {
                Some(job_id) => job_id,
                None => {
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Job ID not found in inbox name".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            },
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to get job ID: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let message_content = match original_message.get_message_content() {
            Ok(content) => content,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get message content: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut job_message: JobMessage = match serde_json::from_str(&message_content) {
            Ok(message) => message,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to deserialize message content: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let parent_parent_key = if !inbox_name.is_empty() {
            match db.get_parent_message_hash(&inbox_name, &parent_message_hash) {
                Ok(result) => result,
                Err(_) => None,
            }
        } else {
            None
        };

        job_message.parent = parent_parent_key;

        let shinkai_message = match Self::api_v2_create_shinkai_message(
            sender,
            recipient,
            &serde_json::to_string(&job_message).unwrap(),
            MessageSchemaType::JobMessageSchema,
            node_encryption_sk.clone(),
            node_signing_sk.clone(),
            node_encryption_pk,
            Some(job_id.clone()),
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

        // Send it for processing
        // Process the job message
        match Self::internal_job_message(job_manager, shinkai_message.clone()).await {
            Ok(_) => {
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

    pub async fn v2_api_update_job_scope(
        db: Arc<SqliteManager>,
        bearer: String,
        job_id: String,
        job_scope: MinimalJobScope,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Check if the job exists
        match db.get_job_with_options(&job_id, false) {
            Ok(_) => {
                // Job exists, proceed with updating the job scope
                match db.update_job_scope(job_id.clone(), job_scope.clone()) {
                    Ok(_) => {
                        match serde_json::to_value(&job_scope) {
                            Ok(job_scope_value) => {
                                let _ = res.send(Ok(job_scope_value)).await;
                            }
                            Err(err) => {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to serialize job scope: {}", err),
                                };
                                let _ = res.send(Err(api_error)).await;
                            }
                        }
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to update job scope: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        Ok(())
                    }
                }
            }
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Job with ID {} not found", job_id),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_job_scope(
        db: Arc<SqliteManager>,
        bearer: String,
        job_id: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Check if the job exists
        match db.get_job_with_options(&job_id, false) {
            Ok(job) => {
                // Job exists, proceed with getting the job scope
                let job_scope = job.scope();
                match serde_json::to_value(&job_scope) {
                    Ok(job_scope_value) => {
                        let _ = res.send(Ok(job_scope_value)).await;
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize job scope: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
                Ok(())
            }
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Job with ID {} not found", job_id),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_fork_job_messages(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        job_id: String,
        message_id: String,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token and extract the sender identity
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let forked_job_id = Self::fork_job(
            db,
            node_name,
            identity_manager,
            job_id,
            Some(message_id),
            node_encryption_sk,
            node_encryption_pk,
            node_signing_sk,
        )
        .await;

        let _ = match forked_job_id {
            Ok(forked_job_id) => res.send(Ok(forked_job_id)).await,
            Err(err) => res.send(Err(err)).await,
        };
        Ok(())
    }

    pub async fn fork_job(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_id: String,
        message_id: Option<String>,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
    ) -> Result<String, APIError> {
        // Retrieve the job
        let source_job = db.get_job_with_options(&job_id, false).map_err(|err| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to retrieve job: {}", err),
        })?;

        // Retrieve the message from the inbox
        let message_id = match message_id {
            Some(message_id) => message_id,
            None => {
                let messages = db
                    .get_last_messages_from_inbox(source_job.conversation_inbox_name.to_string(), usize::MAX - 1, None)
                    .map_err(|err| APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to retrieve message: {}", err),
                    })?;
                let m = match messages.get(messages.len() - 1) {
                    Some(m) => m,
                    None => {
                        return Err(APIError {
                            code: StatusCode::NOT_FOUND.as_u16(),
                            error: "Not Found".to_string(),
                            message: "Message not found".to_string(),
                        })
                    }
                };
                let m = match m.get(m.len() - 1) {
                    Some(m) => m,
                    None => {
                        return Err(APIError {
                            code: StatusCode::NOT_FOUND.as_u16(),
                            error: "Not Found".to_string(),
                            message: "Message not found".to_string(),
                        });
                    }
                };
                // This should be the last message in the inbox
                println!("m: {:?}", m.get_message_content());
                m.calculate_message_hash_for_pagination()
            }
        };
        let source_message = db
            .fetch_message_and_hash(&message_id)
            .map_err(|err| APIError {
                code: StatusCode::NOT_FOUND.as_u16(),
                error: "Not Found".to_string(),
                message: format!("Message not found: {}", err),
            })?
            .0;

        // Get the main identity from the identity manager
        let main_identity = identity_manager
            .lock()
            .await
            .get_main_identity()
            .map_or(
                Err(APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: "Failed to get main identity".to_string(),
                }),
                |identity| Ok(identity.clone()),
            )?
            .clone();

        let sender = ShinkaiName::new(main_identity.get_full_identity_name())?;

        let recipient = ShinkaiName::from_node_and_profile_names_and_type_and_name(
            node_name.node_name,
            "main".to_string(),
            ShinkaiSubidentityType::Agent,
            source_job.parent_agent_or_llm_provider_id.clone(),
        )?;

        // Retrieve the messages from the inbox
        let inbox_name = source_job.conversation_inbox_name.to_string();
        let last_messages = db
            .get_last_messages_from_inbox(inbox_name.clone(), usize::MAX - 1, Some(message_id.clone()))
            .map_err(|err| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to retrieve messages: {}", err),
            })?;

        // Create a new job
        let forked_job_id = format!("jobid_{}", uuid::Uuid::new_v4());
        let _ = db
            .create_new_job(
                forked_job_id.clone(),
                source_job.parent_agent_or_llm_provider_id,
                source_job.scope.clone(),
                source_job.is_hidden,
                source_job.associated_ui,
                source_job.config,
            )
            .map_err(|err| APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to create new job: {}", err),
            })?;

        // Fork the messages
        let mut forked_message_map: HashMap<String, String> = HashMap::new();

        let mut joined_messages = last_messages.iter().flatten().collect::<Vec<_>>();
        joined_messages.push(&source_message);

        for message in joined_messages {
            if let MessageBody::Unencrypted(body) = &message.body {
                if let MessageData::Unencrypted(data) = &body.message_data {
                    if let MessageSchemaType::JobMessageSchema = data.message_content_schema {
                        let mut job_message =
                            serde_json::from_str::<JobMessage>(&data.message_raw_content).map_err(|err| APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to deserialize job message: {}", err),
                            })?;

                        job_message.job_id = forked_job_id.clone();
                        job_message.parent = job_message.parent.map(|parent| {
                            forked_message_map
                                .get(&parent)
                                .cloned()
                                .unwrap_or_else(|| parent.to_string())
                        });

                        let forked_message = Self::api_v2_create_shinkai_message(
                            sender.clone(),
                            recipient.clone(),
                            &serde_json::to_string(&job_message).unwrap(),
                            MessageSchemaType::JobMessageSchema,
                            node_encryption_sk.clone(),
                            node_signing_sk.clone(),
                            node_encryption_pk,
                            Some(job_message.job_id.clone()),
                        )?;

                        forked_message_map.insert(
                            message.calculate_message_hash_for_pagination(),
                            forked_message.calculate_message_hash_for_pagination(),
                        );

                        db.add_message_to_job_inbox(&forked_job_id, &forked_message, job_message.parent.clone(), None)
                            .await
                            .map_err(|err| APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to add message to job inbox: {}", err),
                            })?;
                    }
                }
            }
        }

        let forked_job = ForkedJob {
            job_id: forked_job_id.clone(),
            message_id: message_id.clone(),
        };

        db.add_forked_job(&job_id, forked_job).map_err(|err| APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed to add forked job: {}", err),
        })?;

        Ok(forked_job_id)
    }

    pub async fn v2_remove_job(
        db: Arc<SqliteManager>,
        bearer: String,
        job_id: String,
        res: Sender<Result<SendResponseBody, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let inbox_name = match InboxName::get_job_inbox_name_from_params(job_id.clone()) {
            Ok(inbox) => inbox.to_string(),
            Err(_) => "".to_string(),
        };

        // Retrieve the messages from the inbox
        let messages = match db.get_last_messages_from_inbox(inbox_name, usize::MAX - 1, None) {
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

        // Convert the retrieved messages to Vec<V2ChatMessage>
        let v2_chat_messages = match Self::convert_shinkai_messages_to_v2_chat_messages(messages) {
            Ok(v2_messages) => v2_messages,
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

        // Remove the job
        match db.remove_job(&job_id) {
            Ok(_) => {}
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to remove job: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        // TODO: remove the files from the job folder
        // Remove the file inboxes
        // for file_inbox in file_inboxes {
        //     match db.remove_inbox(&file_inbox) {
        //         Ok(_) => {}
        //         Err(err) => {
        //             let api_error = APIError {
        //                 code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //                 error: "Internal Server Error".to_string(),
        //                 message: format!("Failed to remove file inbox: {}", err),
        //             };
        //             let _ = res.send(Err(api_error)).await;
        //             return Ok(());
        //         }
        //     }
        // }

        let _ = res
            .send(Ok(SendResponseBody {
                status: "success".to_string(),
                message: "Job removed successfully".to_string(),
                data: None,
            }))
            .await;
        Ok(())
    }

    pub async fn v2_export_messages_from_inbox(
        db: Arc<SqliteManager>,
        bearer: String,
        inbox_name: String,
        format: ExportInboxMessagesFormat,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the messages from the inbox
        let messages = match db.get_last_messages_from_inbox(inbox_name.clone(), usize::MAX - 1, None) {
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

        // Convert the retrieved messages to Vec<V2ChatMessage>
        let v2_chat_messages = match Self::convert_shinkai_messages_to_v2_chat_messages(messages) {
            Ok(v2_messages) => v2_messages,
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

        // TODO: Review and fix this

        // Retrieve the filenames in the inboxes
        let file_inboxes = v2_chat_messages
            .iter()
            .flatten()
            .map(|message| message.job_message.fs_files_paths.clone())
            .collect::<Vec<_>>();

        // Export the messages in the requested format
        match format {
            ExportInboxMessagesFormat::CSV => {
                let mut writer = csv::WriterBuilder::new().delimiter(b';').from_writer(vec![]);
                let headers = vec!["timestamp", "sender", "receiver", "message", "files"];

                match writer.write_record(headers) {
                    Ok(_) => {}
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to write CSV headers: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                }

                for message in v2_chat_messages.into_iter().flatten() {
                    let timestamp = message.node_api_data.node_timestamp;
                    let sender = message.sender_subidentity;
                    let receiver = message.receiver_subidentity;
                    let content = message.job_message.content;
                    let files = message
                        .job_message
                        .fs_files_paths
                        .iter()
                        .map(|path| path.relative_path())
                        .collect::<Vec<&str>>()
                        .join(", ");

                    let row = vec![timestamp, sender, receiver, content, files];
                    match writer.write_record(row) {
                        Ok(_) => {}
                        Err(err) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to write CSV row: {}", err),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
                    }
                }

                let csv_data = match writer.into_inner() {
                    Ok(data) => match String::from_utf8(data) {
                        Ok(csv_string) => csv_string,
                        Err(err) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to convert CSV data to string: {}", err),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
                    },
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to retrieve CSV data: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                };

                let _ = res.send(Ok(json!(csv_data))).await;
            }
            ExportInboxMessagesFormat::JSON => {
                let result_messages = v2_chat_messages
                    .into_iter()
                    .map(|messages| {
                        messages
                            .into_iter()
                            .map(|message| {
                                let files: Vec<String> = message
                                    .clone()
                                    .job_message
                                    .fs_files_paths
                                    .into_iter()
                                    .map(|file| file.relative_path().to_string())
                                    .collect();

                                json!({
                                    "message": message,
                                    "files": files,
                                })
                            })
                            .collect::<Vec<serde_json::Value>>()
                    })
                    .collect::<Vec<_>>();

                match serde_json::to_value(&result_messages) {
                    Ok(value) => {
                        let _ = res.send(Ok(value)).await;
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to serialize messages: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                    }
                }
            }
            ExportInboxMessagesFormat::TXT => {
                let mut result_messages = String::new();

                for messages in v2_chat_messages {
                    for message in messages {
                        result_messages.push_str(&format!("{}\n\n", message.job_message.content));

                        for file in &message.job_message.fs_files_paths {
                            result_messages.push_str(&format!("Attached file: {}\n\n", file));
                        }
                    }
                }

                let _ = res.send(Ok(json!(result_messages))).await;
            }
        }

        Ok(())
    }

    pub async fn v2_add_messages_god_mode(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        job_id: String,
        messages: Vec<JobMessage>,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_signing_sk: SigningKey,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
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
        let llm_provider = match db.get_job_with_options(&job_id, false) {
            Ok(job) => job.parent_agent_or_llm_provider_id.clone(),
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: format!("Job with ID {} not found: {}", job_id, err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Check if the job is empty before adding messages
        if db.is_job_inbox_empty(&job_id)? && !messages.is_empty() {
            // Extract first 20 characters from the first message's content
            let first_message_content = &messages[0].content;
            let custom_name: String = if first_message_content.len() > 20 {
                first_message_content[..20].to_string()
            } else {
                first_message_content.to_string()
            };

            // Get the inbox name for the job
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone())?;

            // Update the inbox name
            if let Err(e) = db.unsafe_update_smart_inbox_name(&inbox_name.to_string(), &custom_name) {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to update inbox name: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        // Process each message alternating between user and AI
        for (index, message) in messages.into_iter().enumerate() {
            if index % 2 == 0 {
                // User message
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
                    node_name.node_name.clone(),
                    "main".to_string(),
                    ShinkaiSubidentityType::Agent,
                    llm_provider.clone(),
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
                    &serde_json::to_string(&message).unwrap(),
                    MessageSchemaType::JobMessageSchema,
                    node_encryption_sk.clone(),
                    node_signing_sk.clone(),
                    node_encryption_pk,
                    Some(job_id.clone()),
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

                // Add the user message to the job inbox
                if let Err(err) = db.add_message_to_job_inbox(&job_id, &shinkai_message, None, None).await {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to add user message to job inbox: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            } else {
                // AI message
                let identity_secret_key_clone = clone_signature_secret_key(&node_signing_sk);
                let ai_shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                    job_id.to_string(),
                    message.content,
                    message.fs_files_paths,
                    None,
                    identity_secret_key_clone,
                    node_name.node_name.clone(),
                    node_name.node_name.clone(),
                )
                .expect("Failed to build AI message");

                // Add the AI message to the job inbox
                if let Err(err) = db
                    .add_message_to_job_inbox(&job_id, &ai_shinkai_message, None, None)
                    .await
                {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to add AI message to job inbox: {}", err),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        }

        // Send success response
        let _ = res.send(Ok("Messages added successfully".to_string())).await;

        Ok(())
    }
}
