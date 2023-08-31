use super::{node_api::APIError, node_error::NodeError, node_message_handlers::verify_message_signature, Node};
use crate::{
    db::db_errors::ShinkaiDBError,
    managers::identity_manager::{self, IdentityManager},
    network::node_message_handlers::{ping_pong, PingPong},
    schemas::{
        identity::{DeviceIdentity, Identity, IdentityType, RegistrationCode, StandardIdentity},
        inbox_permission::InboxPermission,
    },
};
use async_channel::Sender;
use chrono::{TimeZone, Utc};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use log::{debug, error, info, trace, warn};
use reqwest::StatusCode;
use shinkai_message_wasm::{
    schemas::{
        agents::serialized_agent::SerializedAgent,
        inbox_name::InboxName,
        shinkai_name::{ShinkaiName, ShinkaiNameError},
    },
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            IdentityPermissions, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
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
    pub async fn send_peer_addresses(&self, sender: Sender<Vec<SocketAddr>>) -> Result<(), Error> {
        let peer_addresses: Vec<SocketAddr> = self.peers.clone().into_iter().map(|(k, _)| k.0).collect();
        sender.send(peer_addresses).await.unwrap();
        Ok(())
    }

    pub async fn handle_external_profile_data(&self, name: String, res: Sender<StandardIdentity>) -> Result<(), Error> {
        let external_global_identity = self
            .identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&name)
            .await
            .unwrap();
        res.send(external_global_identity).await.unwrap();
        Ok(())
    }

    pub async fn connect_node(&self, address: SocketAddr, profile_name: String) -> Result<(), NodeError> {
        let address_str = address.to_string();
        self.connect(&address_str, profile_name).await?;
        Ok(())
    }

    pub async fn internal_get_last_unread_messages_from_inbox(
        &self,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
    ) -> Vec<ShinkaiMessage> {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = match self
            .db
            .lock()
            .await
            .get_last_unread_messages_from_inbox(inbox_name, limit, offset_key)
        {
            Ok(messages) => messages,
            Err(e) => {
                error!("Failed to get last messages from inbox: {}", e);
                return Vec::new();
            }
        };

        result
    }

    pub async fn internal_get_all_inboxes_for_profile(&self, profile_name: String) -> Vec<String> {
        let result = match self.db.lock().await.get_inboxes_for_profile(profile_name) {
            Ok(inboxes) => inboxes,
            Err(e) => {
                error!("Failed to get inboxes for profile: {}", e);
                return Vec::new();
            }
        };

        result
    }

    pub async fn internal_get_last_messages_from_inbox(
        &self,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
    ) -> Vec<ShinkaiMessage> {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = match self
            .db
            .lock()
            .await
            .get_last_messages_from_inbox(inbox_name, limit, offset_key)
        {
            Ok(messages) => messages,
            Err(e) => {
                error!("Failed to get last messages from inbox: {}", e);
                return Vec::new();
            }
        };

        result
    }

    pub async fn send_public_keys(&self, res: Sender<(SignaturePublicKey, EncryptionPublicKey)>) -> Result<(), Error> {
        let identity_public_key = self.identity_public_key.clone();
        let encryption_public_key = self.encryption_public_key.clone();
        let _ = res
            .send((identity_public_key, encryption_public_key))
            .await
            .map_err(|_| ());
        Ok(())
    }

    pub async fn fetch_and_send_last_messages(
        &self,
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    ) -> Result<(), Error> {
        let db = self.db.lock().await;
        let messages = db.get_last_messages_from_all(limit).unwrap_or_else(|_| vec![]);
        let _ = res.send(messages).await.map_err(|_| ());
        Ok(())
    }

    pub async fn internal_mark_as_read_up_to(&self, inbox_name: String, up_to_time: String) -> Result<bool, NodeError> {
        // Attempt to mark messages as read in the database
        self.db
            .lock()
            .await
            .mark_as_read_up_to(inbox_name, up_to_time)
            .map_err(|e| {
                let error_message = format!("Failed to mark messages as read: {}", e);
                error!("{}", &error_message);
                NodeError { message: error_message }
            })?;
        Ok(true)
    }

    pub async fn has_inbox_permission(
        &self,
        inbox_name: String,
        perm_type: String,
        identity_name: String,
        res: Sender<bool>,
    ) {
        // Obtain the IdentityManager and ShinkaiDB locks
        let mut identity_manager = self.identity_manager.lock().await;

        // Find the identity based on the provided name
        let identity = identity_manager.search_identity(&identity_name).await;

        // If identity is None (doesn't exist), return an error message
        if identity.is_none() {
            let _ = res.send(false).await;
            return;
        }

        let identity = identity.unwrap();

        // Check if the found identity is a StandardIdentity. If not, send an error message.
        let standard_identity = match &identity {
            Identity::Standard(std_identity) => std_identity.clone(),
            Identity::Device(std_device) => match std_device.clone().to_standard_identity() {
                Some(identity) => identity,
                None => {
                    let _ = res.send(false).await;
                    return;
                }
            },
            Identity::Agent(_) => {
                let _ = res.send(false).await;
                return;
            }
        };

        let perm = match InboxPermission::from_str(&perm_type) {
            Ok(perm) => perm,
            Err(_) => {
                let _ = res.send(false).await;
                return;
            }
        };

        match self
            .db
            .lock()
            .await
            .has_permission(&inbox_name, &standard_identity, perm)
        {
            Ok(result) => {
                let _ = res.send(result).await;
            }
            Err(_) => {
                let _ = res.send(false).await;
            }
        }
    }

    pub async fn internal_create_new_job(&self, shinkai_message: ShinkaiMessage) -> Result<String, NodeError> {
        println!("Creating new job");
        match self
            .job_manager
            .lock()
            .await
            .process_job_message(shinkai_message)
            .await
        {
            Ok(job_id) => {
                // If everything went well, return Ok(true)
                Ok(job_id)
            }
            Err(err) => {
                // If there was an error, return the error
                Err(NodeError::from(err))
            }
        }
    }

    pub async fn internal_job_message(&self, shinkai_message: ShinkaiMessage) -> Result<(), NodeError> {
        match self
            .job_manager
            .lock()
            .await
            .process_job_message(shinkai_message)
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(NodeError {
                message: format!("Error with process job message: {}", err),
            }),
        }
    }

    pub async fn internal_add_agent(&self, agent: SerializedAgent) -> Result<(), NodeError> {
        match self.db.lock().await.add_agent(agent.clone()) {
            Ok(()) => {
                let mut subidentity_manager = self.identity_manager.lock().await;
                match subidentity_manager.add_agent_subidentity(agent).await {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        error!("Failed to add subidentity: {}", err);
                        Err(NodeError {
                            message: format!("Failed to add device subidentity: {}", err),
                        })
                    }
                }
            }
            Err(e) => Err(NodeError::from(e)),
        }
    }

    pub async fn ping_all(&self) -> io::Result<()> {
        info!("{} > Pinging all peers {} ", self.listen_address, self.peers.len());
        let mut db_lock = self.db.lock().await;
        for (peer, _) in self.peers.clone() {
            let sender = self.node_profile_name.clone().get_node_name();
            let receiver_profile_identity = self
                .identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&peer.1.clone())
                .await
                .unwrap();
            let receiver = receiver_profile_identity.full_identity_name.get_node_name();
            let receiver_public_key = receiver_profile_identity.node_encryption_public_key;

            // Important: the receiver doesn't really matter per se as long as it's valid because we are testing the connection
            let _ = ping_pong(
                peer,
                PingPong::Ping,
                clone_static_secret_key(&self.encryption_secret_key),
                clone_signature_secret_key(&self.identity_secret_key),
                receiver_public_key,
                sender,
                receiver,
                &mut db_lock,
                self.identity_manager.clone(),
            )
            .await;
        }
        Ok(())
    }
}
