use super::{
    network_handlers::{
        handle_based_on_message_content_and_encryption, verify_message_signature
    },
};
use crate::network::{
    agent_payments_manager::{
        my_agent_offerings_manager::MyAgentOfferingsManager,
        external_agent_offerings_manager::ExtAgentOfferingsManager,
    },
    libp2p_manager::NetworkEvent,
    node::ProxyConnectionInfo,
};
use crate::managers::{IdentityManager, identity_manager::IdentityManagerTrait};
use ed25519_dalek::SigningKey;
use libp2p::{request_response::ResponseChannel, PeerId};
use shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key;
use shinkai_message_primitives::{
    schemas::{shinkai_name::ShinkaiName, ws_types::WSUpdateHandler},
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use shinkai_sqlite::SqliteManager;
use std::{net::SocketAddr, sync::{Arc, Weak}};
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use x25519_dalek::StaticSecret as EncryptionStaticKey;

/// Message handler that integrates libp2p messages with the existing Shinkai network logic
pub struct ShinkaiMessageHandler {
    db: Weak<SqliteManager>,
    node_name: ShinkaiName,
    encryption_secret_key: EncryptionStaticKey,
    signature_secret_key: SigningKey,
    identity_manager: Arc<Mutex<IdentityManager>>,
    my_agent_offerings_manager: Weak<Mutex<MyAgentOfferingsManager>>,
    ext_agent_offerings_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
    proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    local_addr: SocketAddr,
}

impl ShinkaiMessageHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Weak<SqliteManager>,
        node_name: ShinkaiName,
        encryption_secret_key: EncryptionStaticKey,
        signature_secret_key: SigningKey,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_agent_offerings_manager: Weak<Mutex<MyAgentOfferingsManager>>,
        ext_agent_offerings_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        local_addr: SocketAddr,
    ) -> Self {
        Self {
            db,
            node_name,
            encryption_secret_key,
            signature_secret_key,
            identity_manager,
            my_agent_offerings_manager,
            ext_agent_offerings_manager,
            proxy_connection_info,
            ws_manager,
            local_addr,
        }
    }

    /// Process the message directly (moved from NetworkJobManager)
    pub async fn handle_message_internode(
        &self,
        receiver_peer_id: PeerId,
        message: &ShinkaiMessage,
        channel: Option<ResponseChannel<ShinkaiMessage>>,
        libp2p_event_sender: Option<UnboundedSender<NetworkEvent>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let maybe_db = self.db
            .upgrade()
            .ok_or("Database reference upgrade failed")?;

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!(
                "{} {} > Network Job Got message from/to peer {:?}",
                self.node_name.get_node_name_string(), self.local_addr, receiver_peer_id
            ),
        );

        // Check if this message came through a relay (has intra_sender)
        // If so, we need to use the original sender's encryption key for decryption
        let (encryption_sender_identity, encryption_public_key, actual_sender_name) = if !message.external_metadata.intra_sender.is_empty() {
            // Message came through relay - use intra_sender for encryption/decryption
            println!("🔄 Message came through relay - original sender: {}, relay: {}", 
                message.external_metadata.intra_sender, 
                message.external_metadata.sender);

            let sender_profile_name_string = message.external_metadata.sender.clone();
            
            let sender_identity = self.identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&sender_profile_name_string, None)
                .await
                .map_err(|e| {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!(
                            "{} > Failed to get sender identity: {:?} {:?}",
                            self.local_addr, sender_profile_name_string, e
                        ),
                    );
                    format!("Failed to get sender identity: {:?}", e)
                })?;

            verify_message_signature(sender_identity.node_signature_public_key, message)
                .map_err(|e| {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Signature verification failed: {:?}", e)
                    );
                    format!("Signature verification failed: {:?}", e)
                })?;
            
            let original_sender_name = message.external_metadata.intra_sender.clone();
            let original_sender_node = ShinkaiName::new(original_sender_name.clone())
                .map_err(|e| format!("Failed to parse original sender name: {:?}", e))?
                .get_node_name_string();
            
            // First check if the 'other' field contains the encryption public key (like in node_shareable_logic.rs)
            eprintln!("🔍 Debug: message.external_metadata.other = {}", message.external_metadata.other);
            if !message.external_metadata.other.is_empty() {
                match string_to_encryption_public_key(&message.external_metadata.other) {
                    Ok(encryption_pk) => {
                        println!("✅ Using encryption public key from 'other' field for original sender: {}", original_sender_node);
                        // Use the relay's identity for address but original sender's encryption key
                        (sender_identity.clone(), encryption_pk, original_sender_name.clone())
                    },
                    Err(e) => {
                        println!("⚠️  Failed to parse encryption public key from 'other' field, using relay's identity: {}", e);
                        (sender_identity.clone(), sender_identity.node_encryption_public_key, sender_profile_name_string.clone())
                    }
                }
            } else {
                // No 'other' field, try to get the original sender's identity for decryption
                match self.identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&original_sender_node, None)
                .await {
                    Ok(original_identity) => {
                        println!("✅ Found original sender identity for decryption: {}", original_sender_node);
                        (original_identity.clone(), original_identity.node_encryption_public_key, original_sender_name)
                    },
                    Err(e) => {
                        // If we can't find the original sender's identity, fall back to using the relay's identity
                        // but log a warning as this will likely fail decryption
                        println!("⚠️  Could not find original sender identity {}, falling back to relay identity: {}", original_sender_node, e);
                        (sender_identity.clone(), sender_identity.node_encryption_public_key, sender_profile_name_string.clone())
                    }
                }                
            }
        } else {
            // Direct message - use the sender's identity
            let sender_profile_name_string = ShinkaiName::from_shinkai_message_only_using_sender_node_name(message)
                .map_err(|e| format!("Failed to extract sender name: {:?}", e))?
                .get_node_name_string();
            
            let sender_identity = self.identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&sender_profile_name_string, None)
                .await
                .map_err(|e| {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!(
                            "{} > Failed to get sender identity: {:?} {:?}",
                            self.local_addr, sender_profile_name_string, e
                        ),
                    );
                    format!("Failed to get sender identity: {:?}", e)
                })?;

            verify_message_signature(sender_identity.node_signature_public_key, message)
                .map_err(|e| {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Signature verification failed: {:?}", e)
                    );
                    format!("Signature verification failed: {:?}", e)
                })?;

            (sender_identity.clone(), sender_identity.node_encryption_public_key, sender_profile_name_string.clone())
        };

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!(
                "{} > Sender Profile Name: {:?}",
                self.local_addr, actual_sender_name
            ),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Node Sender Identity: {}", self.local_addr, encryption_sender_identity),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Verified message signature", self.local_addr),
        );

        let proxy_connection_info = self.proxy_connection_info
            .upgrade()
            .ok_or("ProxyConnectionInfo upgrade failed")?;

        handle_based_on_message_content_and_encryption(
            message.clone(),
            encryption_public_key,
            encryption_sender_identity.addr.unwrap(),
            actual_sender_name.clone(),
            &self.encryption_secret_key,
            &self.signature_secret_key,
            &self.node_name.get_node_name_string(),
            maybe_db,
            self.identity_manager.clone(),
            self.local_addr,
            receiver_peer_id,
            self.my_agent_offerings_manager.clone(),
            self.ext_agent_offerings_manager.clone(),
            proxy_connection_info,
            self.ws_manager.clone(),
            libp2p_event_sender.clone(),
            channel,
        )
        .await
        .map_err(|e| format!("Message processing failed: {:?}", e))?;

        Ok(())
    }
}
