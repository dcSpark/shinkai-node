use ed25519_dalek::SigningKey;
use libp2p::{
    dcutr::{self},
    futures::StreamExt,
    identify::{self, Event as IdentifyEvent},
    noise, ping::{self, Event as PingEvent}, quic, request_response,
    relay::{self, Event as RelayEvent},
    swarm::{NetworkBehaviour, SwarmEvent, Config},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::{LibP2PRelayError, RelayMessage};
use shinkai_crypto_identities::ShinkaiRegistry;

// Custom behaviour for the relay server
#[derive(NetworkBehaviour)]
pub struct RelayBehaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub relay: relay::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub request_response: request_response::json::Behaviour<ShinkaiMessage, ShinkaiMessage>,
}

pub struct RelayManager {
    swarm: Swarm<RelayBehaviour>,
    registered_peers: HashMap<String, PeerId>, // identity -> peer_id
    peer_identities: HashMap<PeerId, String>,  // peer_id -> identity
    message_sender: mpsc::UnboundedSender<RelayMessage>,
    message_receiver: mpsc::UnboundedReceiver<RelayMessage>,
    external_ip: Option<std::net::IpAddr>, // Store detected external IP
    registry: ShinkaiRegistry, // Blockchain registry for identity verification
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
        registry: ShinkaiRegistry,
    ) -> Result<Self, LibP2PRelayError> {
        // Detect external IP address first
        let external_ip = Self::detect_external_ip().await;

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

        // Configure identify protocol - use same protocol version as Shinkai nodes
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shinkai/1.0.0".to_string(),
            local_key.public(),
        ));

        // Configure ping protocol
        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)));

        // Configure relay protocol
        let relay = relay::Behaviour::new(local_peer_id, Default::default());

        // Configure DCUtR for hole punching through relay
        let dcutr = dcutr::Behaviour::new(local_peer_id);

        // Configure request-response behavior for relaying direct messages between Shinkai nodes
        let request_response = request_response::json::Behaviour::new(
            std::iter::once((libp2p::StreamProtocol::new("/shinkai/message/1.0.0"), request_response::ProtocolSupport::Full)),
            request_response::Config::default(),
        );

        // Create the behaviour
        let behaviour = RelayBehaviour {
            identify,
            ping,
            relay,
            dcutr,
            request_response,
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
        }

        // Create message channel
        let (message_sender, message_receiver) = mpsc::unbounded_channel();

        println!("LibP2P Relay initialized with PeerId: {}", local_peer_id);
        println!("Relay node name: {}", relay_node_name);
        println!("🏠 Local binding addresses:");
        println!("🏠   TCP: {}", tcp_listen_addr);
        println!("🏠   QUIC: {}", quic_listen_addr);
        
        if let Some(external_ip) = external_ip {
            println!("🌐 External connectivity addresses (advertised to peers):");
            println!("🌐   TCP: /ip4/{}/tcp/{}", external_ip, listen_port);
            println!("🌐   QUIC: /ip4/{}/udp/{}/quic-v1", external_ip, listen_port);
            println!("🌐 External peers should connect to: {}", external_ip);
        } else {
            println!("⚠️  No external IP detected - only local connectivity available");
        }

        Ok(RelayManager {
            swarm,
            registered_peers: HashMap::new(),
            peer_identities: HashMap::new(),
            message_sender,
            message_receiver,
            external_ip,
            registry,
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
        println!("🔄 Peer {} registered with PeerId: {} - will update peer discovery information", identity, peer_id);
        self.registered_peers.insert(identity.clone(), peer_id);
        self.peer_identities.insert(peer_id, identity);
    }

    pub fn unregister_peer(&mut self, peer_id: &PeerId) {
        if let Some(identity) = self.peer_identities.remove(peer_id) {
            println!("🔄 Peer {} with PeerId: {} unregistered - will update peer discovery information", identity, peer_id);
            self.registered_peers.remove(&identity);
        }
    }

    pub fn find_peer_by_identity(&self, identity: &str) -> Option<PeerId> {
        self.registered_peers.get(identity).copied()
    }

    pub fn find_identity_by_peer(&self, peer_id: &PeerId) -> Option<String> {
        self.peer_identities.get(peer_id).cloned()
    }

    /// Verify a peer's identity by checking their public key against the blockchain registry
    async fn verify_peer_identity_internal(
        registry: ShinkaiRegistry, 
        peer_public_key: ed25519_dalek::VerifyingKey,
        agent_version: String,
    ) -> Option<String> {
        // Extract the identity from the agent version
        let identity = if agent_version.contains("shinkai") || agent_version.contains("node") {
            if let Some(identity_part) = agent_version.split("@@").nth(1) {
                Some(format!("@@{}", identity_part))
            } else { None }
        } else { None };
        
        // Check if we have an identity to verify
        let identity_string = match identity {
            Some(ref id) => id,
            None => {
                println!("❌ No identity provided for verification");
                return None;
            }
        };
        
        match registry.get_identity_record(identity_string.clone(), None).await {
            Ok(identity_record) => {
                if let Ok(registry_public_key) = identity_record.signature_verifying_key() {
                    if registry_public_key == peer_public_key {
                        println!("✅ Identity verification successful: {} matches public key", identity_string);
                        return Some(identity_string.clone());
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to get identity record for {}: {}", identity_string, e);
            }
        };
        
        println!("❌ No matching identity found for public key");
        None
    }

    pub async fn run(&mut self) -> Result<(), LibP2PRelayError> {
        println!("Starting relay manager...");
        
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
                            println!("📡 Relay received outgoing message from {} to {:?}", 
                                msg.identity, msg.target_peer);
                        }
                        None => break, // Channel closed
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
                println!("📡 Relay listening on: {}", address);
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                println!("✅ External address confirmed and advertised to network: {}", address);
            }
            SwarmEvent::ExternalAddrExpired { address } => {
                println!("⚠️  External address expired and removed: {}", address);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Identify(IdentifyEvent::Received {
                peer_id,
                info,
                ..
            })) => {
                println!("Identified peer: {} with protocol version: {}", peer_id, info.protocol_version);
                
                // Extract the peer's public key from the libp2p identity  
                // Get the raw public key bytes and try to create an ed25519_dalek::VerifyingKey
                let public_key_bytes = info.public_key.encode_protobuf();
                
                // For Ed25519, the protobuf encoding includes a prefix, so we need to extract just the key bytes
                // The public key should be 32 bytes for Ed25519
                if public_key_bytes.len() >= 32 {
                    let key_bytes = &public_key_bytes[public_key_bytes.len() - 32..];
                    if let Ok(verifying_key) = ed25519_dalek::VerifyingKey::from_bytes(&key_bytes.try_into().unwrap_or([0u8; 32])) {
                        // Verify the peer's identity using blockchain registry
                        if let Some(verified_identity) = Self::verify_peer_identity_internal(self.registry.clone(), verifying_key, info.agent_version.clone()).await {
                            println!("🔑 Verified and registering peer {} with identity: {}", peer_id, verified_identity);
                            self.register_peer(verified_identity, peer_id);
                        } else {
                            let possible_identity = if info.agent_version.ends_with("shinkai") {
                                if let Some(identity_part) = info.agent_version.split("@@").nth(1) {
                                    Some(format!("@@{}", identity_part))
                                } else { None }
                            } else { None };
                            
                            if let Some(identity) = possible_identity {
                                let identity = format!("{}.not-verified", identity);
                                println!("❌ Verification failed, registering peer {} with identity: {}", peer_id, identity);
                                self.register_peer(identity, peer_id);
                            } else {
                                println!("❌ Could not parse identity from agent version: {}", info.agent_version);
                            }
                        }
                    } else {
                        println!("❌ Failed to convert peer {} public key to ed25519_dalek::VerifyingKey", peer_id);
                    }
                } else {
                    println!("❌ Peer {} public key too short: {} bytes", peer_id, public_key_bytes.len());
                }

                println!("📋 Peer {} supports protocols: {:?}", peer_id, info.protocols);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Ping(ping_event)) => {
                match ping_event {
                    PingEvent { peer, connection: _, result } => {
                        match result {
                            Ok(rtt) => {
                                println!("📡 Ping to {} successful: RTT = {:?}", peer, rtt);
                            }
                            Err(ping::Failure::Timeout) => {
                                println!("⚠️  Ping to {} timed out", peer);
                            }
                            Err(ping::Failure::Unsupported) => {
                                println!("⚠️  Ping protocol unsupported by peer {}", peer);
                            }
                            Err(ping::Failure::Other { error }) => {
                                println!("⚠️  Ping to {} failed: {}", peer, error);
                            }
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Relay(RelayEvent::ReservationReqAccepted {
                src_peer_id,
                ..
            })) => {
                println!("📡 Accepted relay reservation from: {}", src_peer_id);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Dcutr(dcutr_event)) => {
                println!("🔄 DCUtR: Direct connection upgrade event processed: {:?}", dcutr_event);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::RequestResponse(req_resp_event)) => {
                // Handle request-response events for relaying direct messages between Shinkai nodes
                match req_resp_event {
                    request_response::Event::Message { peer, message, .. } => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                println!("🔄 Relay: Received direct message request from peer {}", peer);
                                println!("   Message from: {} to: {}", 
                                    request.external_metadata.sender,
                                    request.external_metadata.recipient);
                                
                                // Try to find the target peer by their identity
                                let target_identity = &request.external_metadata.recipient;
                                let target_node = if let Ok(parsed_name) = shinkai_message_primitives::schemas::shinkai_name::ShinkaiName::new(target_identity.clone()) {
                                    parsed_name.get_node_name_string()
                                } else {
                                    target_identity.clone()
                                };
                                
                                if let Some(target_peer_id) = self.find_peer_by_identity(&target_node) {
                                    println!("   Forwarding to target peer: {}", target_peer_id);
                                    
                                    // Forward the request to the target peer
                                    let _request_id = self.swarm
                                        .behaviour_mut()
                                        .request_response
                                        .send_request(&target_peer_id, request.clone());
                                    
                                    // Send acknowledgment back to sender
                                    let ack_response = request.clone();
                                    if let Err(e) = self.swarm.behaviour_mut().request_response.send_response(channel, ack_response) {
                                        println!("   Failed to send ack to sender: {:?}", e);
                                    } else {
                                        println!("   Sent acknowledgment to sender");
                                    }
                                } else {
                                    println!("   Target peer {} not found", target_node);
                                    
                                    // Send the original message back as "not found" response
                                    let not_found_response = request.clone();
                                    if let Err(e) = self.swarm.behaviour_mut().request_response.send_response(channel, not_found_response) {
                                        println!("   Failed to send not found response: {:?}", e);
                                    }
                                }
                            }
                            request_response::Message::Response { response: _, .. } => {
                                println!("🔄 Relay: Received direct message response from peer {}", peer);
                            }
                        }
                    }
                    request_response::Event::OutboundFailure { peer, error, .. } => {
                        println!("🔄 Relay: Failed to send direct message to peer {}: {:?}", peer, error);
                    }
                    request_response::Event::InboundFailure { peer, error, .. } => {
                        println!("🔄 Relay: Failed to receive direct message from peer {}: {:?}", peer, error);
                    }
                    request_response::Event::ResponseSent { peer, .. } => {
                        println!("🔄 Relay: Successfully sent response to peer {}", peer);
                    }
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("📡 Connection established with peer: {}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("📡 Connection closed with peer: {} (cause: {:?})", peer_id, cause);
                self.unregister_peer(&peer_id);
            }
            _ => {}
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