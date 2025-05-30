use super::{
    network_manager::network_job_manager::{NetworkJobManager, NetworkJobQueue},
};
use chrono::Utc;
use libp2p::{Multiaddr, PeerId};
use shinkai_message_primitives::{
    schemas::shinkai_network::NetworkMessageType,
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

/// Message handler that integrates libp2p messages with the existing Shinkai network logic
pub struct ShinkaiMessageHandler {
    network_job_manager: Arc<Mutex<NetworkJobManager>>,
    // We'll store a mapping of PeerId to SocketAddr for compatibility
    peer_addr_map: Arc<Mutex<std::collections::HashMap<PeerId, SocketAddr>>>,
    local_addr: SocketAddr,
}

impl ShinkaiMessageHandler {
    pub fn new(
        network_job_manager: Arc<Mutex<NetworkJobManager>>,
        local_addr: SocketAddr,
    ) -> Self {
        Self {
            network_job_manager,
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

    /// Handle a message from a peer (non-async version)
    pub async fn handle_message(&self, peer_id: PeerId, message: ShinkaiMessage) {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Handling message from peer {} via libp2p", peer_id),
        );

        // Get or create a SocketAddr for this peer
        let peer_addr = self.get_peer_addr(peer_id).await;

        // Encode the message to bytes for compatibility with existing network job queue
        let encoded_message = match message.encode_message() {
            Ok(bytes) => bytes,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to encode message from peer {}: {}", peer_id, e),
                );
                return;
            }
        };

        // Create a network job that's compatible with the existing system
        let network_job = NetworkJobQueue {
            receiver_address: self.local_addr,
            unsafe_sender_address: peer_addr,
            message_type: NetworkMessageType::ShinkaiMessage,
            content: encoded_message,
            date_created: Utc::now(),
        };

        // Add the job to the existing network job manager
        let mut job_manager = self.network_job_manager.lock().await;
        if let Err(e) = job_manager.add_network_job_to_queue(&network_job).await {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!("Failed to add network job from peer {}: {}", peer_id, e),
            );
        }
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