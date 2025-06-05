use super::{
    network_manager::network_handlers::{
        handle_based_on_message_content_and_encryption, verify_message_signature
    },
    agent_payments_manager::{
        my_agent_offerings_manager::MyAgentOfferingsManager,
        external_agent_offerings_manager::ExtAgentOfferingsManager,
    },
    node::ProxyConnectionInfo,
};
use crate::managers::{IdentityManager, identity_manager::IdentityManagerTrait};
use ed25519_dalek::SigningKey;
use libp2p::{Multiaddr, PeerId};
use shinkai_message_primitives::{
    schemas::{shinkai_name::ShinkaiName, ws_types::WSUpdateHandler},
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use shinkai_sqlite::SqliteManager;
use std::{net::SocketAddr, sync::{Arc, Weak}};
use tokio::sync::Mutex;
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
    // We'll store a mapping of PeerId to SocketAddr for compatibility
    peer_addr_map: Arc<Mutex<std::collections::HashMap<PeerId, SocketAddr>>>,
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
            peer_addr_map: Arc::new(Mutex::new(std::collections::HashMap::new())),
            local_addr,
        }
    }

    /// Add a peer mapping for PeerId to SocketAddr
    pub async fn add_peer_mapping(&self, peer_id: PeerId, addr: SocketAddr) {
        let mut map = self.peer_addr_map.lock().await;
        map.insert(peer_id, addr);
    }

    /// Get SocketAddr for a PeerId, or use a default if not found
    async fn get_peer_addr(&self, peer_id: PeerId) -> SocketAddr {
        let map = self.peer_addr_map.lock().await;
        map.get(&peer_id)
            .copied()
            .unwrap_or_else(|| {
                // Create a synthetic SocketAddr from PeerId
                // This is a workaround for compatibility with existing code
                let hash = peer_id.to_string().chars()
                    .filter_map(|c| c.to_digit(16))
                    .take(8)
                    .fold(0u32, |acc, d| acc * 16 + d);
                
                let ip_bytes = hash.to_be_bytes();
                let port = (hash % 50000 + 10000) as u16; // Port between 10000-60000
                
                SocketAddr::from(([127, ip_bytes[1], ip_bytes[2], ip_bytes[3]], port))
            })
    }

    /// Handle a message from a peer - this replaces the NetworkJobManager processing
    pub async fn handle_message(&self, peer_id: PeerId, message: ShinkaiMessage) {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Handling message from peer {} via libp2p", peer_id),
        );

        // Get or create a SocketAddr for this peer
        let unsafe_sender_address = self.get_peer_addr(peer_id).await;

        // Process the message directly using the existing network handlers
        if let Err(e) = self.handle_message_internode(
            self.local_addr,
            unsafe_sender_address,
            &message,
        ).await {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!("Failed to handle message from peer {}: {:?}", peer_id, e),
            );
        }
    }

    /// Process the message directly (moved from NetworkJobManager)
    async fn handle_message_internode(
        &self,
        receiver_address: SocketAddr,
        unsafe_sender_address: SocketAddr,
        message: &ShinkaiMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let maybe_db = self.db
            .upgrade()
            .ok_or("Database reference upgrade failed")?;

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!(
                "{} {} > Network Job Got message from {:?}",
                self.node_name.get_node_name_string(), receiver_address, unsafe_sender_address
            ),
        );

        // Extract sender's public keys and verify the signature
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
                        receiver_address, sender_profile_name_string, e
                    ),
                );
                format!("Failed to get sender identity: {:?}", e)
            })?;

        verify_message_signature(sender_identity.node_signature_public_key, message)
            .map_err(|e| format!("Signature verification failed: {:?}", e))?;

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!(
                "{} > Sender Profile Name: {:?}",
                receiver_address, sender_profile_name_string
            ),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Node Sender Identity: {}", receiver_address, sender_identity),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Verified message signature", receiver_address),
        );

        let proxy_connection_info = self.proxy_connection_info
            .upgrade()
            .ok_or("ProxyConnectionInfo upgrade failed")?;

        handle_based_on_message_content_and_encryption(
            message.clone(),
            sender_identity.node_encryption_public_key,
            sender_identity.addr.unwrap(),
            sender_profile_name_string,
            &self.encryption_secret_key,
            &self.signature_secret_key,
            &self.node_name.get_node_name_string(),
            maybe_db,
            self.identity_manager.clone(),
            receiver_address,
            unsafe_sender_address,
            self.my_agent_offerings_manager.clone(),
            self.ext_agent_offerings_manager.clone(),
            proxy_connection_info,
            self.ws_manager.clone(),
        )
        .await
        .map_err(|e| format!("Message processing failed: {:?}", e))?;

        Ok(())
    }
}

/// Convert Multiaddr to SocketAddr (best effort)
pub fn multiaddr_to_socket_addr(multiaddr: &Multiaddr) -> Option<SocketAddr> {
    use libp2p::core::multiaddr::Protocol;
    
    let mut ip = None;
    let mut port = None;
    
    for component in multiaddr.iter() {
        match component {
            Protocol::Ip4(addr) => ip = Some(std::net::IpAddr::V4(addr)),
            Protocol::Ip6(addr) => ip = Some(std::net::IpAddr::V6(addr)),
            Protocol::Tcp(p) => port = Some(p),
            _ => {}
        }
    }
    
    match (ip, port) {
        (Some(ip), Some(port)) => Some(SocketAddr::new(ip, port)),
        _ => None,
    }
} 