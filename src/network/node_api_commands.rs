use super::{node_api::APIError, node_message_handlers::verify_message_signature, Node};
use crate::{
    db::db_errors::ShinkaiDBError,
    managers::identity_manager::{self, IdentityManager},
    network::{
        node::NodeError,
        node_message_handlers::{ping_pong, PingPong},
    },
    schemas::{
        identity::{DeviceIdentity, Identity, IdentityType, RegistrationCode, StandardIdentity},
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
        shinkai_name::{ShinkaiName, ShinkaiNameError},
    },
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIGetMessagesFromInbox, APIReadUpToTimeRequest, IdentityPermissions, MessageSchemaType,
            RegistrationCodeRequest, RegistrationCodeType,
        },
    },
    shinkai_utils::{
        encryption::{
            clone_static_secret_key, decrypt_body_message, encryption_public_key_to_string,
            encryption_secret_key_to_string, string_to_encryption_public_key,
        },
        shinkai_message_handler::ShinkaiMessageHandler,
        signatures::{clone_signature_secret_key, string_to_signature_public_key},
    },
};
use std::str::FromStr;
use std::{
    cell::RefCell,
    io::{self, Error},
    net::SocketAddr,
};
use tokio::sync::oneshot::error;
use uuid::Uuid;
use warp::path::full;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
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
        let result = match has_creation_permission {
            Ok(true) => Ok(true),
            Ok(false) => Err(NodeError {
                message: format!(
                    "Permission denied. You don't have enough permissions to access the inbox: {}",
                    inbox_name.get_value()
                ),
            }),
            Err(e) => Err(NodeError {
                message: format!(
                    "Permission denied. You don't have enough permissions to access the inbox: {}. Error: {}",
                    inbox_name.get_value(),
                    e
                ),
            }),
        };

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
        // Decrypt the message body if needed
        let msg = ShinkaiMessageHandler::decrypt_message_body_if_needed(
            potentially_encrypted_msg.clone(),
            &self.encryption_secret_key,
        )?;

        println!("api_get_last_messages_from_inbox > msg: {:?}", msg);

        // Check that the message has the right schema type
        ShinkaiMessageHandler::validate_message_schema(&msg, MessageSchemaType::APIGetMessagesFromInboxRequest)?;

        // Check if the message is coming from one of our subidentities and validate signature
        let sender_name = ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // We (currently) don't proxy external messages from other nodes to other nodes
        if sender_name.get_node_name() != self.node_profile_name.get_node_name() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message:
                        "sender_name.node_name is not the same as self.node_name. It can't proxy through this node."
                            .to_string(),
                }))
                .await;
        }

        // Check that the subidentity that's trying to prox through us exist / is valid and linked to the node
        let subidentity_manager = self.identity_manager.lock().await;
        let sender_subidentity = subidentity_manager.find_by_identity_name(sender_name).cloned();
        std::mem::drop(subidentity_manager);

        // Check that the identity exists locally
        let sender_subidentity = match sender_subidentity.clone() {
            Some(sender) => sender,
            None => {
                return Err(NodeError {
                    message: "Sender subidentity is None".to_string(),
                })
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
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to verify message signature: {}", e),
                    }))
                    .await;
                return Ok(());
            }
        }

        let content = msg.body.unwrap().content;
        let last_messages_inbox_request: APIGetMessagesFromInbox =
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
        // Decrypt the message body if needed
        let msg = ShinkaiMessageHandler::decrypt_message_body_if_needed(
            potentially_encrypted_msg.clone(),
            &self.encryption_secret_key,
        )?;

        println!("api_get_last_unread_messages_from_inbox > msg: {:?}", msg);

        // Check that the message has the right schema type
        ShinkaiMessageHandler::validate_message_schema(&msg, MessageSchemaType::APIGetMessagesFromInboxRequest)?;

        // Check if the message is coming from one of our subidentities and validate signature
        let sender_name = ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // We (currently) don't proxy external messages from other nodes to other nodes
        if sender_name.get_node_name() != self.node_profile_name.get_node_name() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message:
                        "sender_name.node_name is not the same as self.node_name. It can't proxy through this node."
                            .to_string(),
                }))
                .await;
        }

        // Check that the subidentity that's trying to prox through us exist / is valid and linked to the node
        let subidentity_manager = self.identity_manager.lock().await;
        let sender_subidentity = subidentity_manager.find_by_identity_name(sender_name).cloned();
        std::mem::drop(subidentity_manager);

        // Check that the identity exists locally
        let sender_subidentity = match sender_subidentity.clone() {
            Some(sender) => sender,
            None => {
                return Err(NodeError {
                    message: "Sender subidentity is None".to_string(),
                })
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
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to verify message signature: {}", e),
                    }))
                    .await;
                return Ok(());
            }
        }

        let content = msg.body.unwrap().content;
        let last_messages_inbox_request: APIGetMessagesFromInbox =
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

    pub async fn api_mark_as_read_up_to(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        // Decrypt the message body if needed
        let msg = ShinkaiMessageHandler::decrypt_message_body_if_needed(
            potentially_encrypted_msg.clone(),
            &self.encryption_secret_key,
        )?;

        println!("api_mark_as_read_up_to > msg: {:?}", msg);

        // Check that the message has the right schema type
        ShinkaiMessageHandler::validate_message_schema(&msg, MessageSchemaType::APIReadUpToTimeRequest)?;

        // Check if the message is coming from one of our subidentities and validate signature
        let sender_name = ShinkaiName::from_shinkai_message_using_sender_subidentity(&msg.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // We (currently) don't proxy external messages from other nodes to other nodes
        if sender_name.get_node_name() != self.node_profile_name.get_node_name() {
            let _ = res
                .send(Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message:
                        "sender_name.node_name is not the same as self.node_name. It can't proxy through this node."
                            .to_string(),
                }))
                .await;
        }

        // Check that the subidentity that's trying to prox through us exist / is valid and linked to the node
        let subidentity_manager = self.identity_manager.lock().await;
        let sender_subidentity = subidentity_manager.find_by_identity_name(sender_name).cloned();
        std::mem::drop(subidentity_manager);

        // Check that the identity exists locally
        let sender_subidentity = match sender_subidentity.clone() {
            Some(sender) => sender,
            None => {
                return Err(NodeError {
                    message: "Sender subidentity is None".to_string(),
                })
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
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to verify message signature: {}", e),
                    }))
                    .await;
                return Ok(());
            }
        }

        let content = msg.body.unwrap().content;
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
                            let _ = res.send(Ok(())).await;
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
}
