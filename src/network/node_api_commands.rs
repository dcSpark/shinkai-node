use super::{
    node::NEW_PROFILE_SUPPORTED_EMBEDDING_MODELS,
    node_api::{APIError, APIUseRegistrationCodeSuccessResponse, SendResponseBody, SendResponseBodyData},
    node_error::NodeError,
    node_shareable_logic::validate_message_main_logic,
    Node,
};
use crate::{
    db::db_errors::ShinkaiDBError,
    planner::{kai_files::KaiJobFile, kai_manager::KaiJobFileManager},
    schemas::{
        identity::{DeviceIdentity, Identity, IdentityType, RegistrationCode, StandardIdentity, StandardIdentityType},
        inbox_permission::InboxPermission,
        smart_inbox::SmartInbox,
    },
    tools::js_toolkit_executor::JSToolkitExecutor,
    utils::update_global_identity::update_global_identity_name,
};
use crate::{db::ShinkaiDB, managers::identity_manager::IdentityManagerTrait};
use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_channel::Sender;
use blake3::Hasher;
use log::error;
use reqwest::StatusCode;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::{
    schemas::{
        agents::serialized_agent::SerializedAgent,
        inbox_name::InboxName,
        shinkai_name::{ShinkaiName, ShinkaiSubidentityType},
    },
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage}, shinkai_message_error::ShinkaiMessageError, shinkai_message_schemas::{
            APIAddAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions,
            MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
        }
    },
    shinkai_utils::{
        encryption::{
            clone_static_secret_key, encryption_public_key_to_string, string_to_encryption_public_key, EncryptionMethod,
        },
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::{clone_signature_secret_key, signature_public_key_to_string, string_to_signature_public_key},
    },
};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use std::{convert::TryInto, sync::Arc};
use tokio::sync::Mutex;

impl Node {
    pub async fn validate_message(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        schema_type: Option<MessageSchemaType>,
    ) -> Result<(ShinkaiMessage, Identity), APIError> {
        let identity_manager_trait: Box<dyn IdentityManagerTrait + Send> =
            Box::new(self.identity_manager.lock().await.clone());
        // println!("validate_message: {:?}", potentially_encrypted_msg);
        // Decrypt the message body if needed

        validate_message_main_logic(
            &self.encryption_secret_key,
            Arc::new(Mutex::new(identity_manager_trait)),
            &self.node_profile_name,
            potentially_encrypted_msg,
            schema_type,
        )
        .await
    }

    async fn has_standard_identity_access(
        db: Arc<Mutex<ShinkaiDB>>,
        inbox_name: &InboxName,
        std_identity: &StandardIdentity,
    ) -> Result<bool, NodeError> {
        let db_lock = db.lock().await;
        let has_permission = db_lock
            .has_permission(&inbox_name.to_string(), &std_identity, InboxPermission::Read)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(has_permission)
    }

    async fn has_device_identity_access(
        db: Arc<Mutex<ShinkaiDB>>,
        inbox_name: &InboxName,
        std_identity: &DeviceIdentity,
    ) -> Result<bool, NodeError> {
        let std_device = std_identity.clone().to_standard_identity().ok_or(NodeError {
            message: "Failed to convert to standard identity".to_string(),
        })?;
        Self::has_standard_identity_access(db, inbox_name, &std_device).await
    }

    pub async fn has_inbox_access(
        db: Arc<Mutex<ShinkaiDB>>,
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
            Identity::Standard(std_identity) => {
                return Self::has_standard_identity_access(db, inbox_name, std_identity).await;
            }
            Identity::Device(std_device) => {
                return Self::has_device_identity_access(db, inbox_name, std_device).await;
            }
            _ => Err(NodeError {
                message: format!(
                    "Invalid Identity type. You don't have enough permissions to access the inbox: {}",
                    inbox_name.to_string()
                ),
            }),
        }
    }

    async fn process_last_messages_from_inbox<F, T>(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<T, APIError>>,
        response_handler: F,
    ) -> Result<(), NodeError>
    where
        F: FnOnce(Vec<Vec<ShinkaiMessage>>) -> T,
    {
        let validation_result = self
            .validate_message(
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
                _ => return Err(NodeError { message: "Message data is encrypted".into() }),
            },
            _ => return Err(NodeError { message: "Message body is encrypted".into() }),
        };
        let last_messages_inbox_request_result: Result<APIGetMessagesFromInboxRequest, _> = serde_json::from_str(&content);

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

        match Self::has_inbox_access(self.db.clone(), &inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value {
                    let response = self
                        .internal_get_last_messages_from_inbox(inbox_name.to_string(), count, offset)
                        .await;
                    let processed_response = response_handler(response);
                    let _ = res.send(Ok(processed_response)).await;
                    return Ok(());
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to access the inbox: {}",
                                inbox_name.to_string()
                            ),
                        }))
                        .await;

                    return Ok(());
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
                return Ok(());
            }
        }
    }

    pub async fn api_get_last_messages_from_inbox(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    ) -> Result<(), NodeError> {
        self.process_last_messages_from_inbox(potentially_encrypted_msg, res, |response| {
            response.into_iter().filter_map(|msg| msg.first().cloned()).collect()
        }).await
    }

    pub async fn api_get_last_messages_from_inbox_with_branches(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<Vec<ShinkaiMessage>>, APIError>>,
    ) -> Result<(), NodeError> {
        self.process_last_messages_from_inbox(potentially_encrypted_msg, res, |response| {
            response
        }).await
    }

    pub async fn api_get_last_unread_messages_from_inbox(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(
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
        match Self::has_inbox_access(self.db.clone(), &inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value == true {
                    let response = self
                        .internal_get_last_unread_messages_from_inbox(inbox_name.to_string(), count, offset)
                        .await;
                    let _ = res.send(Ok(response)).await;
                    return Ok(());
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to access the inbox: {}",
                                inbox_name.to_string()
                            ),
                        }))
                        .await;

                    return Ok(());
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
                return Ok(());
            }
        }
    }

    pub async fn api_create_and_send_registration_code(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(
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

        let db = self.db.lock().await;
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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::JobCreationSchema))
            .await;
        let (msg, sender_subidentity) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // TODO: add permissions to check if the sender has the right permissions to contact the agent
        match self.internal_create_new_job(msg, sender_subidentity).await {
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
                return Ok(());
            }
        }
    }

    pub async fn api_mark_as_read_up_to(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(
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
        match Self::has_inbox_access(self.db.clone(), &inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value == true {
                    let response = self
                        .internal_mark_as_read_up_to(inbox_name.to_string(), up_to_time.clone())
                        .await;
                    match response {
                        Ok(true) => {
                            let _ = res.send(Ok("true".to_string())).await;
                            return Ok(());
                        }
                        Ok(false) => {
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                    error: "Bad Request".to_string(),
                                    message: format!("Failed to mark as read up to time: {}", up_to_time),
                                }))
                                .await;
                            return Ok(());
                        }
                        Err(_e) => {
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::FORBIDDEN.as_u16(),
                                    error: "Don't have access".to_string(),
                                    message: format!(
                                        "Permission denied. You don't have enough permissions to access the inbox: {}",
                                        inbox_name.to_string()
                                    ),
                                }))
                                .await;

                            return Ok(());
                        }
                    }
                } else {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::FORBIDDEN.as_u16(),
                            error: "Don't have access".to_string(),
                            message: format!(
                                "Permission denied. You don't have enough permissions to access the inbox: {}",
                                inbox_name.to_string()
                            ),
                        }))
                        .await;

                    return Ok(());
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
                return Ok(());
            }
        }
    }

    pub async fn api_handle_registration_code_usage(
        &self,
        msg: ShinkaiMessage,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                return Ok(());
            }
        };

        // Decrypt the message
        let message_to_decrypt = msg.clone();

        let decrypted_message_result =
            message_to_decrypt.decrypt_outer_layer(&self.encryption_secret_key, &sender_encryption_pk);

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

        // Extract values from the ShinkaiMessage
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
        let db = self.db.lock().await;

        // if first_device_registration_needs_code is false
        // then create a new registration code and use it
        // else use the code provided
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!(
                "registration code usage> first device needs registration code?: {:?}",
                self.first_device_needs_registration_code
            )
            .as_str(),
        );

        let main_profile_exists = match db.main_profile_exists(self.node_profile_name.get_node_name().as_str()) {
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

        if self.first_device_needs_registration_code == false {
            if main_profile_exists == false {
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
        }

        let result = db
            .use_registration_code(
                &code.clone(),
                self.node_profile_name.get_node_name().as_str(),
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
        let mut profile_list = vec![];
        profile_list = match db.get_all_profiles(self.node_profile_name.clone()) {
            Ok(profiles) => profiles.iter().map(|p| p.full_identity_name.clone()).collect(),
            Err(e) => panic!("Failed to fetch profiles: {}", e),
        };
        let mut vfs = self.vector_fs.lock().await;
        vfs.initialize_new_profiles(
            &self.node_profile_name,
            profile_list,
            self.embedding_generator.model_type.clone(),
            NEW_PROFILE_SUPPORTED_EMBEDDING_MODELS.clone(),
        )?;

        std::mem::drop(db);
        std::mem::drop(vfs);

        match result {
            Ok(success) => {
                match identity_type {
                    IdentityType::Profile | IdentityType::Global => {
                        // Existing logic for handling profile identity
                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();
                        // let full_identity_name = format!("{}/{}", self.node_profile_name.clone(), profile_name.clone());

                        let full_identity_name_result = ShinkaiName::from_node_and_profile(
                            self.node_profile_name.get_node_name(),
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
                            node_encryption_public_key: self.encryption_public_key.clone(),
                            node_signature_public_key: self.identity_public_key.clone(),
                            identity_type: standard_identity_type,
                            permission_type,
                        };
                        let mut subidentity_manager = self.identity_manager.lock().await;
                        match subidentity_manager.add_profile_subidentity(subidentity).await {
                            Ok(_) => {
                                let success_response = APIUseRegistrationCodeSuccessResponse {
                                    message: success,
                                    node_name: self.node_profile_name.get_node_name().clone(),
                                    encryption_public_key: encryption_public_key_to_string(
                                        self.encryption_public_key.clone(),
                                    ),
                                    identity_public_key: signature_public_key_to_string(
                                        self.identity_public_key.clone(),
                                    ),
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
                        let db = self.db.lock().await;
                        let code_info = db.get_registration_code_info(code.clone().as_str()).unwrap();
                        let profile_name = match code_info.code_type {
                            RegistrationCodeType::Device(profile_name) => profile_name,
                            _ => return Err(Box::new(ShinkaiDBError::InvalidData)),
                        };
                        std::mem::drop(db);

                        let signature_pk_obj = string_to_signature_public_key(profile_identity_pk.as_str()).unwrap();
                        let encryption_pk_obj =
                            string_to_encryption_public_key(profile_encryption_pk.as_str()).unwrap();

                        // Check if the profile exists in the identity_manager
                        {
                            let mut identity_manager = self.identity_manager.lock().await;
                            let profile_identity_name = ShinkaiName::from_node_and_profile(
                                self.node_profile_name.get_node_name(),
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
                                    node_encryption_public_key: self.encryption_public_key.clone(),
                                    node_signature_public_key: self.identity_public_key.clone(),
                                    identity_type: StandardIdentityType::Profile,
                                    permission_type: IdentityPermissions::Admin,
                                };
                                identity_manager.add_profile_subidentity(profile_identity).await?;
                            }
                        }

                        // Logic for handling device identity
                        // let full_identity_name = format!("{}/{}", self.node_profile_name.clone(), profile_name.clone());
                        let full_identity_name = ShinkaiName::from_node_and_profile_and_type_and_name(
                            self.node_profile_name.get_node_name(),
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
                            node_encryption_public_key: self.encryption_public_key.clone(),
                            node_signature_public_key: self.identity_public_key.clone(),
                            profile_encryption_public_key: encryption_pk_obj,
                            profile_signature_public_key: signature_pk_obj,
                            device_encryption_public_key: device_encryption_pk_obj,
                            device_signature_public_key: device_signature_pk_obj,
                            permission_type,
                        };

                        let mut identity_manager = self.identity_manager.lock().await;
                        match identity_manager.add_device_subidentity(device_identity).await {
                            Ok(_) => {
                                if main_profile_exists == false && !self.initial_agents.is_empty() {
                                    std::mem::drop(identity_manager);
                                    let profile = full_identity_name.extract_profile()?;
                                    for agent in &self.initial_agents {
                                        self.internal_add_agent(agent.clone(), &profile).await?;
                                    }
                                }

                                let success_response = APIUseRegistrationCodeSuccessResponse {
                                    message: success,
                                    node_name: self.node_profile_name.get_node_name().clone(),
                                    encryption_public_key: encryption_public_key_to_string(
                                        self.encryption_public_key.clone(),
                                    ),
                                    identity_public_key: signature_public_key_to_string(
                                        self.identity_public_key.clone(),
                                    ),
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

    pub async fn api_update_smart_inbox_name(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::TextContent))
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
                    match self
                        .internal_update_smart_inbox_name(inbox_name.clone(), new_name)
                        .await
                    {
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
                    let db_lock = self.db.lock().await;
                    let has_permission = db_lock
                        .has_permission(&inbox_name, &std_identity, InboxPermission::Admin)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                    if has_permission {
                        match self.internal_update_smart_inbox_name(inbox_name, new_name).await {
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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<SmartInbox>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::TextContent))
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
                let sender_profile_name = match std_identity.full_identity_name.get_profile_name() {
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
                    let inboxes = self.internal_get_all_smart_inboxes_for_profile(profile_requested).await;

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

                    return Ok(());
                }
            }
            Identity::Device(std_device) => {
                let sender_profile_name = std_device.full_identity_name.get_profile_name().unwrap();

                if (std_device.permission_type == IdentityPermissions::Admin)
                    || (sender_profile_name == profile_requested)
                {
                    // Get all inboxes for the profilei
                    let inboxes = self.internal_get_all_smart_inboxes_for_profile(profile_requested).await;

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
                    return Ok(());
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
    }

    pub async fn api_get_all_inboxes_for_profile(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::TextContent))
            .await;
        let (msg, sender) = match validation_result {
            Ok((msg, sender)) => (msg, sender),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile_requested_str: String = msg.get_message_content()?;
        let profile_requested: ShinkaiName;
        if ShinkaiName::validate_name(&profile_requested_str).is_ok() {
            profile_requested = ShinkaiName::new(profile_requested_str.clone()).map_err(|err| err.to_string())?;
        } else {
            profile_requested = ShinkaiName::from_node_and_profile(
                self.node_profile_name.get_node_name(),
                profile_requested_str.clone(),
            )
            .map_err(|err| err.to_string())?;
        }

        // Check that the message is coming from someone with the right permissions to do this action
        match sender {
            Identity::Standard(std_identity) => {
                // should be safe. previously checked in validate_message
                let sender_profile_name = match std_identity.full_identity_name.get_profile_name() {
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
                    || (sender_profile_name == profile_requested.get_profile_name().unwrap_or("".to_string()))
                {
                    // Get all inboxes for the profile
                    let inboxes = self.internal_get_all_inboxes_for_profile(profile_requested).await;

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

                    return Ok(());
                }
            }
            Identity::Device(std_device) => {
                let sender_profile_name = std_device.full_identity_name.get_profile_name().unwrap();

                if (std_device.permission_type == IdentityPermissions::Admin)
                    || (sender_profile_name == profile_requested.get_profile_name().unwrap_or("".to_string()))
                {
                    // Get all inboxes for the profile
                    let inboxes = self.internal_get_all_inboxes_for_profile(profile_requested).await;

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
                    return Ok(());
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
    }

    pub async fn api_add_toolkit(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::TextContent))
            .await;
        let (msg, _) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile = ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone())?;

        let hex_blake3_hash = msg.get_message_content()?;

        let files = {
            let db_lock = self.db.lock().await;
            match db_lock.get_all_files_from_inbox(hex_blake3_hash) {
                Ok(files) => files,
                Err(err) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("{}", err),
                        }))
                        .await;
                    return Ok(());
                }
            }
        };

        let header_file = files.iter().find(|(name, _)| name.ends_with(".json"));
        let packaged_toolkit = files.iter().find(|(name, _)| name.ends_with(".js"));

        if header_file.is_none() || packaged_toolkit.is_none() {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: "Required file is missing".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // get and validate packaged_toolkit
        let toolkit_file = String::from_utf8(packaged_toolkit.unwrap().1.clone());
        if let Err(err) = toolkit_file {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("{}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        let toolkit_file = toolkit_file.unwrap();

        // Get and validate header values file
        let header_values_json = match String::from_utf8(header_file.unwrap().1.clone()) {
            Ok(s) => s,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let header_values = serde_json::from_str(&header_values_json).unwrap_or(JsonValue::Null);

        // initialize the executor (locally or remotely depending on ENV)
        let executor_result = match &self.js_toolkit_executor_remote {
            Some(remote_address) => JSToolkitExecutor::new_remote(remote_address.clone()).await,
            None => JSToolkitExecutor::new_local().await,
        };

        let executor = match executor_result {
            Ok(executor) => executor,
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Generate toolkit json from JS source code
        let toolkit = executor.submit_toolkit_json_request(&toolkit_file).await;
        if let Err(err) = toolkit {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "User Error".to_string(),
                message: format!("{}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        let toolkit = toolkit.unwrap();

        {
            eprintln!("api_add_toolkit> toolkit tool structs: {:?}", toolkit);
            let db_lock = self.db.lock().await;
            let init_result = db_lock.init_profile_tool_structs(&profile);
            if let Err(err) = init_result {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            eprintln!("api_add_toolkit> profile install toolkit: {:?}", profile);
            let install_result = db_lock.install_toolkit(&toolkit, &profile);
            if let Err(err) = install_result {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            eprintln!(
                "api_add_toolkit> profile setting toolkit header values: {:?}",
                header_values
            );
            let set_header_result = db_lock
                .set_toolkit_header_values(
                    &toolkit.name.clone(),
                    &profile.clone(),
                    &header_values.clone(),
                    &executor,
                )
                .await;
            if let Err(err) = set_header_result {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            // Instantiate a RemoteEmbeddingGenerator to generate embeddings for the tools being added to the node
            let embedding_generator = Box::new(RemoteEmbeddingGenerator::new_default());
            eprintln!("api_add_toolkit> profile activating toolkit: {}", toolkit.name);
            let activate_toolkit_result = db_lock
                .activate_toolkit(&toolkit.name.clone(), &profile.clone(), &executor, embedding_generator)
                .await;
            if let Err(err) = activate_toolkit_result {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }
        let _ = res.send(Ok("Toolkit installed successfully".to_string())).await;
        Ok(())
    }

    pub async fn api_list_toolkits(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::TextContent))
            .await;
        let (msg, _) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile = ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone())?.extract_profile();
        if let Err(err) = profile {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("{}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        let profile = profile.unwrap();
        let toolkit_map;
        {
            let db_lock = self.db.lock().await;
            toolkit_map = match db_lock.get_installed_toolkit_map(&profile) {
                Ok(t) => t,
                Err(err) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("{}", err),
                        }))
                        .await;
                    return Ok(());
                }
            };
        }

        // Convert the toolkit_map into a JSON string
        let toolkit_map_json = match serde_json::to_string(&toolkit_map) {
            Ok(json) => json,
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to convert toolkit map to JSON: {}", err),
                    }))
                    .await;
                return Ok(());
            }
        };

        let _ = res.send(Ok(toolkit_map_json)).await;
        Ok(())
    }

    pub async fn api_update_job_to_finished(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the message
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::APIFinishJob))
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
                    let db_lock = self.db.lock().await;
                    match db_lock.update_job_to_finished(&job_id) {
                        Ok(_) => {
                            match db_lock.get_kai_file_from_inbox(inbox_name.to_string()).await {
                                Ok(Some((_, kai_file_bytes))) => {
                                    let kai_file_str = match String::from_utf8(kai_file_bytes) {
                                        Ok(s) => s,
                                        Err(_) => {
                                            let _ = res
                                                .send(Err(APIError {
                                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                                    error: "Bad Request".to_string(),
                                                    message: "Failed to convert bytes to string".to_string(),
                                                }))
                                                .await;
                                            return Ok(());
                                        }
                                    };

                                    let kai_file: KaiJobFile = match KaiJobFile::from_json_str(&kai_file_str) {
                                        Ok(k) => k,
                                        Err(_) => {
                                            let _ = res
                                                .send(Err(APIError {
                                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                                    error: "Bad Request".to_string(),
                                                    message: "Failed to parse KaiJobFile".to_string(),
                                                }))
                                                .await;
                                            return Ok(());
                                        }
                                    };

                                    match KaiJobFileManager::execute(kai_file, self).await {
                                        Ok(_) => (),
                                        Err(e) => shinkai_log(
                                            ShinkaiLogOption::DetailedAPI,
                                            ShinkaiLogLevel::Error,
                                            format!("Error executing KaiJobFileManager: {}", e).as_str(),
                                        ),
                                    }
                                }
                                Ok(None) => shinkai_log(
                                    ShinkaiLogOption::DetailedAPI,
                                    ShinkaiLogLevel::Info,
                                    format!("No file found in the inbox").as_str(),
                                ),
                                Err(err) => shinkai_log(
                                    ShinkaiLogOption::DetailedAPI,
                                    ShinkaiLogLevel::Error,
                                    format!("Error getting file from inbox: {:?}", err).as_str(),
                                ),
                            }

                            let _ = res.send(Ok(())).await;
                            Ok(())
                        }
                        Err(err) => {
                            match err {
                                ShinkaiDBError::SomeError(_) => {
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
        &self,
        res: Sender<Result<Vec<StandardIdentity>, APIError>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Obtain the IdentityManager lock
        let identity_manager = self.identity_manager.lock().await;

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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg.clone(), Some(MessageSchemaType::JobMessageSchema))
            .await;
        let (msg, _) = match validation_result {
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
        // TODO: add permissions to check if the sender has the right permissions to send the job message

        match self.internal_job_message(msg.clone()).await {
            Ok(_) => {
                let inbox_name = match InboxName::from_message(&msg.clone()) {
                    Ok(inbox) => inbox.to_string(),
                    Err(_) => "".to_string(),
                };
    
                let scheduled_time = msg.external_metadata.scheduled_time;
                let message_hash = potentially_encrypted_msg.calculate_message_hash_for_pagination();
    
                let parent_key = if !inbox_name.is_empty() {
                    let db_guard = self.db.lock().await;
                    match db_guard.get_parent_message_hash(&inbox_name, &message_hash) {
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
                return Ok(());
            }
        }
    }

    pub async fn api_available_agents(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<SerializedAgent>, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::Empty))
            .await;
        let (msg, _) = match validation_result {
            Ok((msg, sender_subidentity)) => (msg, sender_subidentity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let profile = ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone())?
            .get_profile_name()
            .ok_or(NodeError {
                message: "Profile name not found".to_string(),
            })?;

        match self.internal_get_agents_for_profile(profile).await {
            Ok(agents) => {
                let _ = res.send(Ok(agents)).await;
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

    pub async fn api_add_agent(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::APIAddAgentRequest))
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

        let serialized_agent_result = serde_json::from_str::<APIAddAgentRequest>(&serialized_agent_string);

        let serialized_agent = match serialized_agent_result {
            Ok(agent) => agent,
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

        match self.internal_add_agent(serialized_agent.agent, &profile).await {
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
                return Ok(());
            }
        }
    }

    pub async fn api_create_files_inbox_with_symmetric_key(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the message
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::SymmetricKeyExchange))
            .await;
        let (msg, _) = match validation_result {
            Ok((msg, identity)) => (msg, identity),
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Decrypt the message
        let decrypted_msg = msg.decrypt_outer_layer(&self.encryption_secret_key, &self.encryption_public_key)?;

        // Extract the content of the message
        let content = decrypted_msg.get_message_content()?;

        // Convert the hex string to bytes
        let private_key_bytes = match hex::decode(&content) {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Invalid private key".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        };

        // Convert the Vec<u8> to a [u8; 32]
        let private_key_array: Result<[u8; 32], _> = private_key_bytes.try_into();
        let private_key_array = match private_key_array {
            Ok(array) => array,
            Err(_) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Failed to convert private key to array".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        };

        // Calculate the hash of it using blake3 which will act as a sort of public identifier
        let mut hasher = Hasher::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        let hash_hex = hex::encode(result.as_bytes());

        // Write the symmetric key to the database
        let mut db = self.db.lock().await;
        match db.write_symmetric_key(&hash_hex, &private_key_array) {
            Ok(_) => {
                // Create the files message inbox
                match db.create_files_message_inbox(hash_hex.clone()) {
                    Ok(_) => {
                        let _ = res
                            .send(Ok(
                                "Symmetric key stored and files message inbox created successfully".to_string()
                            ))
                            .await;
                        Ok(())
                    }
                    Err(err) => {
                        let api_error = APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: format!("Failed to create files message inbox: {}", err),
                        };
                        let _ = res.send(Err(api_error)).await;
                        return Ok(());
                    }
                }
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("{}", err),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }
    }

    pub async fn api_get_filenames_in_inbox(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the message
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::TextContent))
            .await;
        let msg = match validation_result {
            Ok((msg, _)) => msg,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Decrypt the message
        let decrypted_msg = msg.decrypt_outer_layer(&self.encryption_secret_key, &self.encryption_public_key)?;

        // Extract the content of the message
        let hex_blake3_hash = decrypted_msg.get_message_content()?;

        match self.db.lock().await.get_all_filenames_from_inbox(hex_blake3_hash) {
            Ok(filenames) => {
                let _ = res.send(Ok(filenames)).await;
                Ok(())
            }
            Err(err) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("{}", err),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_add_file_to_inbox_with_symmetric_key(
        &self,
        filename: String,
        file_data: Vec<u8>,
        hex_blake3_hash: String,
        encrypted_nonce: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let private_key_array = {
            let db = self.db.lock().await;
            match db.read_symmetric_key(&hex_blake3_hash) {
                Ok(key) => key,
                Err(_) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: "Invalid public key".to_string(),
                        }))
                        .await;
                    return Ok(());
                }
            }
        };

        let private_key_slice = &private_key_array[..];
        let private_key_generic_array = GenericArray::from_slice(private_key_slice);
        let cipher = Aes256Gcm::new(private_key_generic_array);

        // Assuming `encrypted_nonce` is a hex string of the nonce used in encryption
        let nonce_bytes = hex::decode(&encrypted_nonce).unwrap();
        let nonce = GenericArray::from_slice(&nonce_bytes);

        // Decrypt file
        let decrypted_file_result = cipher.decrypt(nonce, file_data.as_ref());
        let decrypted_file = match decrypted_file_result {
            Ok(file) => file,
            Err(_) => {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Failed to decrypt the file.".to_string(),
                    }))
                    .await;
                return Ok(());
            }
        };

        shinkai_log(
            ShinkaiLogOption::DetailedAPI,
            ShinkaiLogLevel::Debug,
            format!(
                "api_add_file_to_inbox_with_symmetric_key> filename: {}, hex_blake3_hash: {}, decrypted_file.len(): {}",
                filename,
                hex_blake3_hash,
                decrypted_file.len()
            )
            .as_str(),
        );

        match self
            .db
            .lock()
            .await
            .add_file_to_files_message_inbox(hex_blake3_hash, filename, decrypted_file)
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
                        message: format!("{}", err),
                    }))
                    .await;
                Ok(())
            }
        }
    }

    pub async fn api_is_pristine(&self, res: Sender<Result<bool, APIError>>) -> Result<(), NodeError> {
        let db_lock = self.db.lock().await;
        let has_any_profile = db_lock.has_any_profile().unwrap_or(false);
        let _ = res.send(Ok(!has_any_profile)).await;
        Ok(())
    }

    pub async fn api_change_nodes_name(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // TODO: check if name is valid and exists in the blockchain
        // Send message back to the API
        // 1 sec later? panic! and exit the program
        // Validate the message

        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(MessageSchemaType::ChangeNodesName))
            .await;
        let msg = match validation_result {
            Ok((msg, _)) => msg,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Decrypt the message
        let decrypted_msg = msg.decrypt_outer_layer(&self.encryption_secret_key, &self.encryption_public_key)?;

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
            let identity_manager = self.identity_manager.lock().await;
            match identity_manager
                .external_profile_to_global_identity(new_node_name.get_node_name().as_str())
                .await
            {
                Ok(standard_identity) => {
                    if standard_identity.node_encryption_public_key != self.encryption_public_key
                        || standard_identity.node_signature_public_key != self.identity_public_key
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
        match update_global_identity_name(new_node_name.get_node_name().as_str()) {
            Ok(_) => {
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

    pub async fn api_handle_send_onionized_message(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    ) -> Result<(), NodeError> {
        // This command is used to send messages that are already signed and (potentially) encrypted
        if self.node_profile_name.get_node_name() == "@@localhost.shinkai" {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Invalid node name: @@localhost.shinkai".to_string(),
                }))
                .await;
            return Ok(());
        }

        let validation_result = self.validate_message(potentially_encrypted_msg.clone(), None).await;
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
            .get_node_name();

        let sender_node_name = ShinkaiName::from_shinkai_message_only_using_sender_node_name(&msg.clone())
            .unwrap()
            .get_node_name();

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
                            let mut db_guard = self.db.lock().await;
                            let parent_message_id = match msg.get_message_parent_key() {
                                Ok(key) => Some(key),
                                Err(_) => None,
                            };

                            db_guard
                                .unsafe_insert_inbox_message(&msg.clone(), parent_message_id)
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

        let external_global_identity_result = self
            .identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&recipient_node_name_string.clone())
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

        msg.external_metadata.intra_sender = "".to_string();
        msg.encryption = EncryptionMethod::DiffieHellmanChaChaPoly1305;

        let encrypted_msg = msg.encrypt_outer_layer(
            &self.encryption_secret_key.clone(),
            &external_global_identity.node_encryption_public_key,
        )?;

        // We update the signature so it comes from the node and not the profile
        // that way the recipient will be able to verify it
        let signature_sk = clone_signature_secret_key(&self.identity_secret_key);
        let encrypted_msg = encrypted_msg.sign_outer_layer(&signature_sk)?;
        let node_addr = external_global_identity.addr.unwrap();

        Node::send(
            encrypted_msg,
            Arc::new(clone_static_secret_key(&self.encryption_secret_key)),
            (node_addr, recipient_node_name_string),
            self.db.clone(),
            self.identity_manager.clone(),
            true,
            None,
        );

        {
            let inbox_name = match InboxName::from_message(&msg.clone()) {
                Ok(inbox) => inbox.to_string(),
                Err(_) => "".to_string(),
            };

            let scheduled_time = msg.external_metadata.scheduled_time;
            let message_hash = potentially_encrypted_msg.calculate_message_hash_for_pagination();

            let parent_key = if !inbox_name.is_empty() {
                let db_guard = self.db.lock().await;
                match db_guard.get_parent_message_hash(&inbox_name, &message_hash) {
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
