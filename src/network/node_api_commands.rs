use super::{node_api::APIError, node_message_handlers::verify_message_signature, Node};
use crate::{
    db::db_errors::ShinkaiDBError,
    managers::identity_manager::{self, IdentityManager},
    network::{
        node::NodeError,
        node_message_handlers::{ping_pong, PingPong},
    },
    schemas::{
        identity::{DeviceIdentity, Identity, IdentityType, RegistrationCode, StandardIdentity, StandardIdentityType},
        inbox_permission::InboxPermission,
    },
};
use async_channel::Sender;
use chrono::{TimeZone, Utc};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use log::{debug, error, info, trace, warn};
use mupdf::Device;
use reqwest::StatusCode;
use shinkai_message_wasm::{
    schemas::{
        inbox_name::InboxName,
        shinkai_name::{ShinkaiName, ShinkaiNameError, ShinkaiSubidentityType},
    },
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{
            APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions, MessageSchemaType,
            RegistrationCodeRequest, RegistrationCodeType,
        },
    },
    shinkai_utils::{
        encryption::{
            clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string,
            string_to_encryption_public_key,
        },
        signatures::{clone_signature_secret_key, string_to_signature_public_key},
    },
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
    pub async fn validate_message(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        schema_type: Option<MessageSchemaType>,
    ) -> Result<(ShinkaiMessage, Identity), APIError> {
        // Decrypt the message body if needed
        let msg: ShinkaiMessage;
        {
            // check if the message is encrypted
            let is_body_encrypted = potentially_encrypted_msg.clone().is_body_currently_encrypted();
            if is_body_encrypted {
                /*
                When someone sends an encrypted message, we need to compute the shared key using Diffie-Hellman,
                but what if they are using a subidentity? We don't know which one because it's encrypted.
                So the only way to get the pk is if they send it to us in the external_metadata.other field.
                For other cases, we can find it in the identity manager.
                */
                let sender_encryption_pk_string = potentially_encrypted_msg.external_metadata.clone().other;
                let sender_encryption_pk = string_to_encryption_public_key(sender_encryption_pk_string.as_str()).ok();

                if sender_encryption_pk.is_some() {
                    msg = match potentially_encrypted_msg.clone()
                        .decrypt_outer_layer(&self.encryption_secret_key, &sender_encryption_pk.unwrap())
                    {
                        Ok(msg) => msg,
                        Err(e) => {
                            return Err(APIError {
                                code: StatusCode::BAD_REQUEST.as_u16(),
                                error: "Bad Request".to_string(),
                                message: format!("Failed to decrypt message body: {}", e),
                            })
                        }
                    };
                } else {
                    let sender_name = ShinkaiName::from_shinkai_message_only_using_sender_node_name(
                        &potentially_encrypted_msg.clone(),
                    )?;

                    let sender_encryption_pk = match self.identity_manager
                    .lock()
                    .await
                    .search_identity(sender_name.clone().to_string().as_str())
                    .await  // Add this line to await the Future
                    {
                    Some(identity) => {
                        match identity {
                            Identity::Standard(std_identity) => std_identity.node_encryption_public_key,
                            Identity::Device(device) => device.node_encryption_public_key,
                            Identity::Agent(_) => {
                                return Err(APIError {
                                    code: StatusCode::BAD_REQUEST.as_u16(),
                                    error: "Bad Request".to_string(),
                                    message: "Failed to get sender encryption pk from message: Agent identity not supported".to_string(),
                                })
                            },
                        }
                    },
                    None => {
                        return Err(APIError {
                            code: StatusCode::BAD_REQUEST.as_u16(),
                            error: "Bad Request".to_string(),
                            message: "Failed to get sender encryption pk from message: Identity not found".to_string(),
                        })
                    }};
                    msg = match potentially_encrypted_msg.clone()
                        .decrypt_outer_layer(&self.encryption_secret_key, &sender_encryption_pk)
                    {
                        Ok(msg) => msg,
                        Err(e) => {
                            return Err(APIError {
                                code: StatusCode::BAD_REQUEST.as_u16(),
                                error: "Bad Request".to_string(),
                                message: format!("Failed to decrypt message body: {}", e),
                            })
                        }
                    };
                }
                println!("after decrypt_message_body_if_needed> msg: {:?}", msg);
            } else {
                msg = potentially_encrypted_msg.clone();
            }
        }

        // Check that the message has the right schema type
        if let Some(schema) = schema_type {
            println!("schema: {:?}", schema);
            if let Err(e) = msg.validate_message_schema(schema) {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Invalid message schema: {}", e),
                });
            }
        }

        // Check if the message is coming from one of our subidentities and validate signature
        let sender_name = match ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone()) {
            Ok(name) => name,
            Err(e) => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to get sender name from message: {}", e),
                })
            }
        };

        // We (currently) don't proxy external messages from other nodes to other nodes
        if sender_name.get_node_name() != self.node_profile_name.get_node_name() {
            return Err(APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "sender_name.node_name is not the same as self.node_name. It can't proxy through this node."
                    .to_string(),
            });
        }

        // Check that the subidentity that's trying to prox through us exist / is valid and linked to the node
        let subidentity_manager = self.identity_manager.lock().await;
        let sender_subidentity = subidentity_manager.find_by_identity_name(sender_name).cloned();
        std::mem::drop(subidentity_manager);

        println!(
            "after find_by_identity_name> sender_subidentity: {:?}",
            sender_subidentity
        );

        // Check that the identity exists locally
        let sender_subidentity = match sender_subidentity.clone() {
            Some(sender) => sender,
            None => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Sender subidentity is None".to_string(),
                });
            }
        };

        // Check that the message signature is valid according to the local keys
        match IdentityManager::verify_message_signature(
            Some(sender_subidentity.clone()),
            &potentially_encrypted_msg,
            &msg.clone(),
        ) {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to verify message signature: {}", e);
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to verify message signature: {}", e),
                });
            }
        }

        Ok((msg, sender_subidentity))
    }

    async fn has_standard_identity_access(
        &self,
        inbox_name: &InboxName,
        std_identity: &StandardIdentity,
    ) -> Result<bool, NodeError> {
        let db_lock = self.db.lock().await;
        let has_permission = db_lock
            .has_permission(&inbox_name.get_value(), &std_identity, InboxPermission::Read)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        Ok(has_permission)
    }

    async fn has_device_identity_access(
        &self,
        inbox_name: &InboxName,
        std_identity: &DeviceIdentity,
    ) -> Result<bool, NodeError> {
        let std_device = std_identity.clone().to_standard_identity().ok_or(NodeError {
            message: "Failed to convert to standard identity".to_string(),
        })?;
        self.has_standard_identity_access(inbox_name, &std_device).await
    }

    async fn has_inbox_access(&self, inbox_name: &InboxName, sender_subidentity: &Identity) -> Result<bool, NodeError> {
        let sender_shinkai_name = ShinkaiName::new(sender_subidentity.get_full_identity_name())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let has_creation_permission = inbox_name.has_creation_access(sender_shinkai_name);
        if let Ok(true) = has_creation_permission {
            return Ok(true);
        }

        match sender_subidentity {
            Identity::Standard(std_identity) => {
                return self.has_standard_identity_access(inbox_name, std_identity).await;
            }
            Identity::Device(std_device) => {
                return self.has_device_identity_access(inbox_name, std_device).await;
            }
            _ => Err(NodeError {
                message: format!(
                    "Invalid Identity type. You don't have enough permissions to access the inbox: {}",
                    inbox_name.get_value()
                ),
            }),
        }
    }

    pub async fn api_get_last_messages_from_inbox(
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
        let last_messages_inbox_request: APIGetMessagesFromInboxRequest =
            serde_json::from_str(&content).map_err(|e| NodeError {
                message: format!("Failed to parse GetLastMessagesFromInboxRequest: {}", e),
            })?;

        let inbox_name = last_messages_inbox_request.inbox;
        let count = last_messages_inbox_request.count;
        let offset = last_messages_inbox_request.offset;
        println!("offset: {:?}", offset);

        // Check that the message is coming from someone with the right permissions to do this action
        // TODO(Discuss): can local admin read any messages from any device or profile?
        match self.has_inbox_access(&inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value == true {
                    let response = self
                        .internal_get_last_messages_from_inbox(inbox_name.get_value(), count, offset)
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
                                inbox_name.get_value()
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
        let last_messages_inbox_request: APIGetMessagesFromInboxRequest =
            serde_json::from_str(&content).map_err(|e| NodeError {
                message: format!("Failed to parse GetLastMessagesFromInboxRequest: {}", e),
            })?;

        let inbox_name = last_messages_inbox_request.inbox;
        let count = last_messages_inbox_request.count;
        let offset = last_messages_inbox_request.offset;

        // Check that the message is coming from someone with the right permissions to do this action
        // TODO(Discuss): can local admin read any messages from any device or profile?
        match self.has_inbox_access(&inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value == true {
                    let response = self
                        .internal_get_last_unread_messages_from_inbox(inbox_name.get_value(), count, offset)
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
                                inbox_name.get_value()
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

        // TODO: add permissions to check if the sender has the right permissions to contact the agent

        match self.internal_create_new_job(msg).await {
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
        match self.has_inbox_access(&inbox_name, &sender_subidentity).await {
            Ok(value) => {
                if value == true {
                    let response = self
                        .internal_mark_as_read_up_to(inbox_name.get_value(), up_to_time.clone())
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
                        Err(e) => {
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::FORBIDDEN.as_u16(),
                                    error: "Don't have access".to_string(),
                                    message: format!(
                                        "Permission denied. You don't have enough permissions to access the inbox: {}",
                                        inbox_name.get_value()
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
                                inbox_name.get_value()
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
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("handle_registration_code_usage");
        let sender_encryption_pk_string = msg.external_metadata.clone().other;
        let sender_encryption_pk = string_to_encryption_public_key(sender_encryption_pk_string.as_str()).unwrap();

        // Decrypt the message
        let message_to_decrypt = msg.clone();
        let sender_encryption_pk_string = encryption_public_key_to_string(sender_encryption_pk);
        let encryption_secret_key_string = encryption_secret_key_to_string(self.encryption_secret_key.clone());

        let decrypted_message =
            message_to_decrypt.decrypt_outer_layer(&self.encryption_secret_key, &sender_encryption_pk)?;

        // Deserialize body.content into RegistrationCode
        let content = decrypted_message.get_message_content()?;
        println!("handle_registration_code_usage> content: {:?}", content);
        // let registration_code: RegistrationCode = serde_json::from_str(&content).unwrap();
        let registration_code: RegistrationCode = serde_json::from_str(&content).map_err(|e| NodeError {
            message: format!("Failed to deserialize the content: {}", e),
        })?;
        println!(
            "handle_registration_code_usage> registration_code: {:?}",
            registration_code
        );

        // Extract values from the ShinkaiMessage
        let code = registration_code.code;
        let registration_name = registration_code.registration_name;
        let identity_pk = registration_code.identity_pk;
        let encryption_pk = registration_code.encryption_pk;
        let identity_type = registration_code.identity_type;
        println!("handle_registration_code_usage> identity_type: {:?}", identity_type);
        // Comment (to me): this should be able to handle Device and Agent identities
        // why are we forcing standard_idendity_type?
        // let standard_identity_type = identity_type.to_standard().unwrap();
        let permission_type = registration_code.permission_type;

        println!("handle_registration_code_usage> code: {:?}", code);
        println!("identity_type: {:?}", identity_type);
        println!("registration name: {}", registration_name);
        println!("permission_type: {:?}", permission_type);

        let db = self.db.lock().await;
        // TODO: remove this
        println!("handle_registration_code_usage> before use_registration_code");
        db.debug_print_all_keys_for_profiles_identity_key();
        let result = db
            .use_registration_code(
                &code.clone(),
                self.node_profile_name.get_node_name().as_str(),
                registration_name.as_str(),
                &identity_pk,
                &encryption_pk,
            )
            .map_err(|e| e.to_string())
            .map(|_| "true".to_string());

        // TODO: remove this eventually or make it a debug
        println!("handle_registration_code_usage> after use_registration_code");
        db.debug_print_all_keys_for_profiles_identity_key();
        std::mem::drop(db);

        match result {
            Ok(success) => {
                match identity_type {
                    IdentityType::Profile | IdentityType::Global => {
                        // Existing logic for handling profile identity
                        let signature_pk_obj = string_to_signature_public_key(identity_pk.as_str()).unwrap();
                        let encryption_pk_obj = string_to_encryption_public_key(encryption_pk.as_str()).unwrap();
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
                                let _ = res.send(Ok(success)).await.map_err(|_| ());
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

                        let signature_pk_obj = string_to_signature_public_key(identity_pk.as_str()).unwrap();
                        let encryption_pk_obj = string_to_encryption_public_key(encryption_pk.as_str()).unwrap();

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

                        let device_identity = DeviceIdentity {
                            full_identity_name,
                            node_encryption_public_key: self.encryption_public_key.clone(),
                            node_signature_public_key: self.identity_public_key.clone(),
                            profile_encryption_public_key: Some(encryption_pk_obj),
                            profile_signature_public_key: Some(signature_pk_obj),
                            device_signature_public_key: None, // NOTE: This assumes you don't have the device signature PK in the RegistrationCode. Adjust if necessary.
                            permission_type,
                        };

                        let mut identity_manager = self.identity_manager.lock().await;
                        match identity_manager.add_device_subidentity(device_identity).await {
                            Ok(_) => {
                                let _ = res.send(Ok(success)).await.map_err(|_| ());
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

        let profile_requested = msg.get_message_content()?;

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
                    || (sender_profile_name == profile_requested)
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

    pub async fn api_handle_send_onionized_message(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // This command is used to send messages that are already signed and (potentially) encrypted
        eprintln!("handle_onionized_message msg: {:?}", potentially_encrypted_msg);

        let validation_result = self.validate_message(potentially_encrypted_msg, None).await;
        let (msg, _) = match validation_result {
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
                            db_guard.unsafe_insert_inbox_message(&msg.clone()).map_err(|e| {
                                eprintln!("handle_onionized_message > Error inserting message into db: {}", e);
                                std::io::Error::new(std::io::ErrorKind::Other, format!("Insertion error: {}", e))
                            })?;
                        }
                        Err(e) => {
                            eprintln!(
                                "handle_onionized_message > Error checking if sender has access to inbox: {}",
                                e
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
        let recipient_profile_name_string =
            ShinkaiName::from_shinkai_message_only_using_recipient_node_name(&msg.clone())
                .unwrap()
                .to_string();

        let external_global_identity = self
            .identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&recipient_profile_name_string.clone())
            .await
            .unwrap();

        println!(
            "handle_onionized_message > recipient_profile_name_string: {}",
            recipient_profile_name_string
        );

        let body_encrypted_msg = msg.encrypt_outer_layer(
            &self.encryption_secret_key.clone(),
            &external_global_identity.node_encryption_public_key,
        )?;

        // We update the signature so it comes from the node and not the profile
        // that way the recipient will be able to verify it
        let signature_sk = clone_signature_secret_key(&self.identity_secret_key);
        let msg = body_encrypted_msg.sign_outer_layer(&signature_sk)?;

        let mut db_guard = self.db.lock().await;

        let node_addr = external_global_identity.addr.unwrap();

        Node::send(
            &msg,
            clone_static_secret_key(&self.encryption_secret_key),
            (node_addr, recipient_profile_name_string),
            &mut db_guard,
            self.identity_manager.clone(),
        )
        .await?;

        if res.send(Ok(())).await.is_err() {
            eprintln!("Failed to send response");
        }

        Ok(())
    }
}
