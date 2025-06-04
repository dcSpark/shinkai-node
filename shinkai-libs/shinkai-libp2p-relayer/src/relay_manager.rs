use ed25519_dalek::SigningKey;
use libp2p::{
    futures::StreamExt,
    gossipsub::{self, Event as GossipsubEvent, MessageAuthenticity, ValidationMode, MessageId},
    identify::{self, Event as IdentifyEvent},
    kad,
    noise, ping, quic,
    relay::{self, Event as RelayEvent},
    swarm::{NetworkBehaviour, SwarmEvent, Config},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_message_primitives::{
    schemas::shinkai_network::NetworkMessageType,
    shinkai_message::shinkai_message::ShinkaiMessage,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::{LibP2PRelayError, RelayMessage};

// Custom behaviour for the relay server
#[derive(NetworkBehaviour)]
pub struct RelayBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub relay: relay::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

pub struct RelayManager {
    swarm: Swarm<RelayBehaviour>,
    registered_peers: HashMap<String, PeerId>, // identity -> peer_id
    peer_identities: HashMap<PeerId, String>,  // peer_id -> identity
    message_sender: mpsc::UnboundedSender<RelayMessage>,
    message_receiver: mpsc::UnboundedReceiver<RelayMessage>,
    external_ip: Option<std::net::IpAddr>, // Store detected external IP
}

impl RelayManager {
    /// Detect the external IP address using multiple services as fallback
    async fn detect_external_ip() -> Option<std::net::IpAddr> {
        // List of external IP detection services
        let services = [
            "https://httpbin.org/ip",
            "https://api.ipify.org",
            "https://ifconfig.me/ip",
            "https://icanhazip.com",
        ];

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .ok()?;

        for service in &services {
            println!("Attempting to detect external IP using: {}", service);
            
            match tokio::time::timeout(Duration::from_secs(5), client.get(*service).send()).await {
                Ok(Ok(response)) => {
                    if let Ok(body) = response.text().await {
                        let ip_str = if service.contains("httpbin.org") {
                            // httpbin.org returns JSON: {"origin": "IP"}
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                                if let Some(origin) = json.get("origin").and_then(|v| v.as_str()) {
                                    origin.split(',').next().unwrap_or("").trim().to_string()
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        } else {
                            // Other services return plain text IP
                            body.trim().to_string()
                        };

                        if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
                            println!("Successfully detected external IP: {} using {}", ip, service);
                            return Some(ip);
                        }
                    }
                }
                Ok(Err(e)) => {
                    println!("HTTP error from {}: {}", service, e);
                }
                Err(_) => {
                    println!("Timeout detecting IP from: {}", service);
                }
            }
        }

        println!("Failed to detect external IP from all services");
        None
    }

    pub async fn new(
        listen_port: u16,
        relay_node_name: String,
        identity_secret_key: SigningKey,
    ) -> Result<Self, LibP2PRelayError> {
        // Detect external IP address first
        let external_ip = Self::detect_external_ip().await;
        if let Some(ip) = external_ip {
            println!("Detected external IP address: {}", ip);
        } else {
            println!("Warning: Could not detect external IP address. External connectivity may be limited.");
        }

        // Generate deterministic PeerId from relay name
        let local_key = libp2p::identity::Keypair::ed25519_from_bytes(identity_secret_key.to_bytes())
            .map_err(|e| LibP2PRelayError::LibP2PError(format!("Failed to create keypair: {}", e)))?;
        let local_peer_id = PeerId::from(local_key.public());

        // Configure transport with QUIC and TCP fallback support
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

        let quic_transport = quic::tokio::Transport::new(quic::Config::new(&local_key))
            .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

        // Combine QUIC and TCP transports - QUIC will be preferred, TCP as fallback
        let transport = quic_transport
            .or_transport(tcp_transport)
            .map(|either_output, _| either_output.into_inner())
            .boxed();

        // Configure gossipsub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Permissive)
            .mesh_outbound_min(0)      // Allow zero outbound connections
            .mesh_n_low(1)             // Allow single node mesh
            .mesh_n(8)                 // Higher target for relay (hub for multiple nodes)
            .mesh_n_high(16)           // High maximum to handle many nodes
            .gossip_lazy(6)            // More gossip for better propagation as hub
            .fanout_ttl(Duration::from_secs(60))
            .gossip_retransimission(3)  // Retransmit messages for reliability
            .duplicate_cache_time(Duration::from_secs(120))  // Longer dedup cache
            .max_transmit_size(262144) // 256KB max message size
            .message_id_fn(|message| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                message.data.hash(&mut hasher);
                MessageId::from(hasher.finish().to_string())
            })
            .build()
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Gossipsub config error: {}", e)))?;

        let mut gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .map_err(|e| LibP2PRelayError::LibP2PError(format!("Gossipsub creation error: {}", e)))?;

        // Subscribe to common topics that nodes will use
        let shinkai_topic = gossipsub::IdentTopic::new("shinkai-network");
        gossipsub.subscribe(&shinkai_topic)
            .map_err(|e| LibP2PRelayError::LibP2PError(format!("Failed to subscribe to shinkai-network: {}", e)))?;

        // Configure identify protocol
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shinkai-relay/1.0.0".to_string(),
            local_key.public(),
        ));

        // Configure ping protocol
        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)));

        // Configure relay protocol
        let relay = relay::Behaviour::new(local_peer_id, Default::default());

        // Configure Kademlia DHT
        let mut kademlia = kad::Behaviour::new(
            local_peer_id,
            kad::store::MemoryStore::new(local_peer_id),
        );
        kademlia.set_mode(Some(kad::Mode::Server));

        // Create the behaviour
        let behaviour = RelayBehaviour {
            gossipsub,
            identify,
            ping,
            relay,
            kademlia,
        };

        // Create swarm with proper configuration
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, Config::with_tokio_executor());

        // Listen on both TCP and QUIC ports - bind to all interfaces
        let tcp_listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
            .parse()
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid TCP listen address: {}", e)))?;

        let quic_listen_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", listen_port)
            .parse()
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid QUIC listen address: {}", e)))?;

        swarm
            .listen_on(tcp_listen_addr.clone())
            .map_err(|e| LibP2PRelayError::LibP2PError(format!("Failed to listen on TCP: {}", e)))?;

        swarm
            .listen_on(quic_listen_addr.clone())
            .map_err(|e| LibP2PRelayError::LibP2PError(format!("Failed to listen on QUIC: {}", e)))?;

        // If we detected an external IP, also add external addresses to help with connectivity
        if let Some(external_ip) = external_ip {
            let external_tcp_addr: Multiaddr = format!("/ip4/{}/tcp/{}", external_ip, listen_port)
                .parse()
                .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid external TCP address: {}", e)))?;
            
            let external_quic_addr: Multiaddr = format!("/ip4/{}/udp/{}/quic-v1", external_ip, listen_port)
                .parse()
                .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid external QUIC address: {}", e)))?;
            
            // Add external addresses for advertisement
            swarm.add_external_address(external_tcp_addr.clone());
            swarm.add_external_address(external_quic_addr.clone());
            
            println!("Added external addresses for advertisement:");
            println!("  TCP: {}", external_tcp_addr);
            println!("  QUIC: {}", external_quic_addr);
        }

        // Create message channel
        let (message_sender, message_receiver) = mpsc::unbounded_channel();

        println!("LibP2P Relay initialized with PeerId: {}", local_peer_id);
        println!("Relay node name: {}", relay_node_name);
        println!("Listening on TCP: {} and QUIC: {}", tcp_listen_addr, quic_listen_addr);
        
        if let Some(external_ip) = external_ip {
            println!("External IP detected: {} - peers can connect via /ip4/{}/tcp/{} or /ip4/{}/udp/{}/quic-v1", 
                external_ip, external_ip, listen_port, external_ip, listen_port);
        }

        Ok(RelayManager {
            swarm,
            registered_peers: HashMap::new(),
            peer_identities: HashMap::new(),
            message_sender,
            message_receiver,
            external_ip,
        })
    }

    /// Get the external IP address if detected
    pub fn get_external_ip(&self) -> Option<std::net::IpAddr> {
        self.external_ip
    }

    /// Get external addresses for this relay
    pub fn get_external_addresses(&self, listen_port: u16) -> Vec<Multiaddr> {
        let mut addresses = Vec::new();
        
        if let Some(external_ip) = self.external_ip {
            if let Ok(tcp_addr) = format!("/ip4/{}/tcp/{}", external_ip, listen_port).parse::<Multiaddr>() {
                addresses.push(tcp_addr);
            }
            if let Ok(quic_addr) = format!("/ip4/{}/udp/{}/quic-v1", external_ip, listen_port).parse::<Multiaddr>() {
                addresses.push(quic_addr);
            }
        }
        
        addresses
    }

    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn get_message_sender(&self) -> mpsc::UnboundedSender<RelayMessage> {
        self.message_sender.clone()
    }

    pub fn register_peer(&mut self, identity: String, peer_id: PeerId) {
        println!("Registering peer: {} with PeerId: {}", identity, peer_id);
        self.registered_peers.insert(identity.clone(), peer_id);
        self.peer_identities.insert(peer_id, identity);
    }

    pub fn unregister_peer(&mut self, peer_id: &PeerId) {
        if let Some(identity) = self.peer_identities.remove(peer_id) {
            self.registered_peers.remove(&identity);
            println!("Unregistered peer: {} with PeerId: {}", identity, peer_id);
        }
    }

    pub fn find_peer_by_identity(&self, identity: &str) -> Option<PeerId> {
        self.registered_peers.get(identity).copied()
    }

    pub fn find_identity_by_peer(&self, peer_id: &PeerId) -> Option<String> {
        self.peer_identities.get(peer_id).cloned()
    }

    pub async fn run(&mut self) -> Result<(), LibP2PRelayError> {
        println!("Starting relay manager...");
        
        // Set up a timer for periodic Kademlia bootstrap (every 30 seconds)
        let mut bootstrap_interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await?;
                }
                
                // Handle outgoing messages
                message = self.message_receiver.recv() => {
                    match message {
                        Some(msg) => {
                            self.handle_outgoing_message(msg).await?;
                        }
                        None => break, // Channel closed
                    }
                }
                
                // Periodic Kademlia bootstrap
                _ = bootstrap_interval.tick() => {
                    if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
                        println!("Kademlia bootstrap failed: {:?}", e);
                    } else {
                        println!("Initiated Kademlia bootstrap");
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<RelayBehaviourEvent>,
    ) -> Result<(), LibP2PRelayError> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Relay listening on {}", address);
                
                // If this is an external address, log it prominently
                if let Some(external_ip) = self.external_ip {
                    let addr_str = address.to_string();
                    if addr_str.contains(&external_ip.to_string()) {
                        println!("🌐 External address ready for peer connections: {}", address);
                    }
                }
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                println!("🌐 External address confirmed and advertised: {}", address);
            }
            SwarmEvent::ExternalAddrExpired { address } => {
                println!("⚠️  External address expired: {}", address);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Gossipsub(GossipsubEvent::Message {
                propagation_source,
                message,
                ..
            })) => {
                self.handle_gossipsub_message(propagation_source, message.data).await?;
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Identify(IdentifyEvent::Received {
                peer_id,
                info,
                ..
            })) => {
                println!("Identified peer: {} with protocol version: {}", peer_id, info.protocol_version);
                // Add peer addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
                // We could use this to auto-register peers based on their identify info
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Relay(RelayEvent::ReservationReqAccepted {
                src_peer_id,
                ..
            })) => {
                println!("Accepted relay reservation from: {}", src_peer_id);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                id: _,
                result,
                ..
            })) => {
                match result {
                    kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk {
                        peer,
                        num_remaining,
                    })) => {
                        println!("Kademlia bootstrap progress: peer={}, remaining={}", peer, num_remaining);
                    }
                    kad::QueryResult::Bootstrap(Err(e)) => {
                        println!("Kademlia bootstrap error: {:?}", e);
                    }
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { providers, .. })) => {
                        println!("Found {} providers", providers.len());
                    }
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. })) => {
                        println!("Provider search finished with no additional records");
                    }
                    kad::QueryResult::GetProviders(Err(e)) => {
                        println!("Get providers error: {:?}", e);
                    }
                    _ => {}
                }
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
                peer,
                is_new_peer,
                addresses,
                ..
            })) => {
                println!("Kademlia routing updated: peer={}, new={}, addresses={:?}", 
                    peer, is_new_peer, addresses);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("Connection established with peer: {}", peer_id);
                
                // Subscribe to main shinkai network topic to help with mesh formation
                let topic = gossipsub::IdentTopic::new("shinkai-network");
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                    println!("Already subscribed to shinkai-network: {}", e);
                }
                
                // Publish a peer announcement to help other nodes discover this peer
                let announcement = format!("{{\"type\":\"peer_connected\",\"peer_id\":\"{}\"}}", peer_id);
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, announcement.as_bytes()) {
                    println!("Failed to announce peer connection: {:?}", e);
                } else {
                    println!("Announced connection of peer: {}", peer_id);
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("Connection closed with peer: {} (cause: {:?})", peer_id, cause);
                self.unregister_peer(&peer_id);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_gossipsub_message(
        &mut self,
        _propagation_source: PeerId,
        data: Vec<u8>,
    ) -> Result<(), LibP2PRelayError> {
        // First try to parse as a simple discovery message
        if let Ok(message_str) = String::from_utf8(data.clone()) {
            // Check if it's a discovery message
            if message_str.contains("\"type\":\"discovery\"") || 
               message_str.contains("\"type\":\"peer_joined\"") || 
               message_str.contains("\"type\":\"peer_connected\"") {
                println!("Received discovery message: {}", message_str);
                // Discovery messages are handled automatically by gossipsub propagation
                return Ok(());
            }
        }
        
        // Try to parse as ShinkaiMessage directly
        match serde_json::from_slice::<ShinkaiMessage>(&data) {
            Ok(shinkai_message) => {
                println!("Received ShinkaiMessage from: {} to: {}", 
                    shinkai_message.external_metadata.sender,
                    shinkai_message.external_metadata.recipient);
                self.handle_shinkai_message_direct(shinkai_message).await?;
            }
            Err(e) => {
                // Log but don't fail - could be other types of messages
                println!("Received non-Shinkai message ({}): {:?}", e, 
                    String::from_utf8_lossy(&data[..std::cmp::min(100, data.len())]));
            }
        }
        Ok(())
    }

    async fn handle_shinkai_message_direct(&mut self, shinkai_message: ShinkaiMessage) -> Result<(), LibP2PRelayError> {
        let recipient = &shinkai_message.external_metadata.recipient;
        let sender = &shinkai_message.external_metadata.sender;
        
        println!("Routing ShinkaiMessage from {} to {}", sender, recipient);
        
        // Extract the node name from the recipient (remove subidentity parts)
        let target_node = if let Ok(parsed_name) = shinkai_message_primitives::schemas::shinkai_name::ShinkaiName::new(recipient.clone()) {
            parsed_name.get_node_name_string()
        } else {
            recipient.clone()
        };
        
        // Create topic based on recipient node name
        let topic_name = format!("shinkai-{}", target_node);
        let topic = gossipsub::IdentTopic::new(topic_name.clone());
        
        // Subscribe to the topic if not already subscribed (this allows us to relay messages)
        if let Err(e) = self.swarm.behaviour_mut().gossipsub.subscribe(&topic) {
            println!("Already subscribed to topic {}: {}", topic_name, e);
        }
        
        // Don't republish the message - just ensure we're subscribed to relay it
        // The gossipsub protocol will automatically relay messages to subscribed peers
        println!("Relay is now subscribing to topic: {} to relay messages for {}", topic_name, target_node);
        
        Ok(())
    }

    async fn handle_outgoing_message(&mut self, message: RelayMessage) -> Result<(), LibP2PRelayError> {
        // Convert message to bytes and publish via gossipsub
        let data = message.to_bytes()?;
        
        // Use a topic based on the target peer or a general relay topic
        let topic_name = if let Some(target) = &message.target_peer {
            format!("shinkai-relay-{}", target)
        } else {
            "shinkai-relay-general".to_string()
        };

        let topic = gossipsub::IdentTopic::new(topic_name);
        
        // Subscribe to the topic if not already subscribed
        let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
        
        // Publish the message
        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
            return Err(LibP2PRelayError::MessageDeliveryFailed(format!(
                "Failed to publish message: {:?}",
                e
            )));
        }

        Ok(())
    }

    async fn route_message(&mut self, message: RelayMessage) -> Result<(), LibP2PRelayError> {
        match message.message_type {
            NetworkMessageType::ProxyMessage => {
                // Handle registration/connection message
                self.handle_proxy_registration(message).await?;
            }
            NetworkMessageType::ShinkaiMessage => {
                // Route the message to the target peer
                self.handle_shinkai_message_routing(message).await?;
            }
        }
        Ok(())
    }

    async fn handle_proxy_registration(&mut self, message: RelayMessage) -> Result<(), LibP2PRelayError> {
        // For now, we'll implement a simple registration based on the identity
        // In a real implementation, you'd want to validate the identity through cryptographic means
        println!("Received proxy registration from: {}", message.identity);
        
        // The registration would typically include a challenge-response or signature verification
        // For this example, we'll assume the peer is already connected and identified
        
        Ok(())
    }

    async fn handle_shinkai_message_routing(&mut self, message: RelayMessage) -> Result<(), LibP2PRelayError> {
        if let Some(target_identity) = &message.target_peer {
            if let Some(target_peer_id) = self.find_peer_by_identity(target_identity) {
                // Route message to specific peer via gossipsub topic
                let topic_name = format!("shinkai-direct-{}", target_peer_id);
                let topic = gossipsub::IdentTopic::new(topic_name);
                
                let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
                
                let data = message.to_bytes()?;
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                    return Err(LibP2PRelayError::MessageDeliveryFailed(format!(
                        "Failed to route message to {}: {:?}",
                        target_identity, e
                    )));
                }
                
                println!("Routed message from {} to {}", message.identity, target_identity);
            } else {
                return Err(LibP2PRelayError::PeerNotFound(format!(
                    "Target peer not found: {}",
                    target_identity
                )));
            }
        } else {
            // Broadcast message to all peers
            let topic = gossipsub::IdentTopic::new("shinkai-broadcast");
            let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
            
            let data = message.to_bytes()?;
            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                return Err(LibP2PRelayError::MessageDeliveryFailed(format!(
                    "Failed to broadcast message: {:?}",
                    e
                )));
            }
            
            println!("Broadcasted message from {}", message.identity);
        }
        
        Ok(())
    }
}

// This allows the noise configuration to work
impl From<noise::Error> for LibP2PRelayError {
    fn from(e: noise::Error) -> Self {
        LibP2PRelayError::LibP2PError(format!("Noise error: {}", e))
    }
} 