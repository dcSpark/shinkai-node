use ed25519_dalek::SigningKey;
use libp2p::{
    dcutr::{self},
    futures::StreamExt,
    identify::{self, Event as IdentifyEvent},
    noise, ping::{self}, request_response,
    relay::{self},
    swarm::{NetworkBehaviour, SwarmEvent, Config},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use std::{time::Duration, collections::VecDeque};
use tokio::sync::mpsc;
use dashmap::DashMap;
use shinkai_message_primitives::shinkai_message::shinkai_message::{
    ExternalMetadata, InternalMetadata, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiVersion,
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::shinkai_utils::shinkai_time::ShinkaiStringTime;

use crate::{LibP2PRelayError, RelayMessage};
use shinkai_crypto_identities::ShinkaiRegistry;

/// A queued message waiting to be delivered
#[derive(Debug)]
pub struct QueuedRelayMessage {
    pub source_peer: PeerId,
    pub target_identity: String,
    pub message: ShinkaiMessage,
    pub retry_count: u32,
    pub last_attempt: std::time::Instant,
}

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
    registered_peers: DashMap<String, PeerId>, // identity -> peer_id
    peer_identities: DashMap<PeerId, String>,  // peer_id -> identity
    message_sender: mpsc::UnboundedSender<RelayMessage>,
    message_receiver: mpsc::UnboundedReceiver<RelayMessage>,
    external_ip: Option<std::net::IpAddr>, // Store detected external IP
    registry: ShinkaiRegistry, // Blockchain registry for identity verification
    // Message queue fields
    message_queue: VecDeque<QueuedRelayMessage>, // Queue for messages that failed to deliver
    max_retry_attempts: u32, // Maximum retry attempts per message
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

        // Configure transport with TCP
        let transport = tcp::tokio::Transport::new(tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)))
            .boxed();

        // Configure identify protocol - use same protocol version as Shinkai nodes
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shinkai/1.0.0".to_string(),
            local_key.public(),
        ).with_agent_version(format!(
            "shinkai-relayer/{}/{}",
            std::env::var("NODE_NAME").unwrap_or_else(|_| "@@relayer_pub_01.sep-shinkai".to_string()),
            env!("CARGO_PKG_VERSION")
        ))
        .with_interval(Duration::from_secs(60))
        .with_push_listen_addr_updates(true)
        .with_cache_size(100)
        .with_hide_listen_addrs(true));

        // Configure ping protocol
        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(10)));

        // Configure relay protocol
        let relay = relay::Behaviour::new(local_peer_id, Default::default());

        // Configure DCUtR for hole punching through relay
        let dcutr = dcutr::Behaviour::new(local_peer_id);

        // Configure request-response behavior for relaying direct messages between Shinkai nodes
        let request_response = request_response::json::Behaviour::new(
            std::iter::once((libp2p::StreamProtocol::new("/shinkai/message/1.0.0"), request_response::ProtocolSupport::Full)),
            request_response::Config::default().with_request_timeout(Duration::from_secs(300)),
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

        // Listen on TCP - bind to all interfaces
        let tcp_listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
            .parse()
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid TCP listen address: {}", e)))?;

        swarm
            .listen_on(tcp_listen_addr.clone())
            .map_err(|e| LibP2PRelayError::LibP2PError(format!("Failed to listen on TCP: {}", e)))?;

        // If we detected an external IP, also add external addresses to help with connectivity
        if let Some(external_ip) = external_ip {
            let external_tcp_addr: Multiaddr = format!("/ip4/{}/tcp/{}", external_ip, listen_port)
                .parse()
                .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid external TCP address: {}", e)))?;
            
            // Add external addresses for advertisement
            swarm.add_external_address(external_tcp_addr.clone());
        }

        // Create message channel
        let (message_sender, message_receiver) = mpsc::unbounded_channel();

        println!("LibP2P Relay initialized with PeerId: {}", local_peer_id);
        println!("Relay node name: {}", relay_node_name);
        println!("🏠 Local binding addresses:");
        println!("🏠   TCP: {}", tcp_listen_addr);
        
        if let Some(external_ip) = external_ip {
            println!("🌐 External connectivity addresses (advertised to peers):");
            println!("🌐   TCP: /ip4/{}/tcp/{}", external_ip, listen_port);
            println!("🌐 External peers should connect to: {}", external_ip);
        } else {
            println!("⚠️  No external IP detected - only local connectivity available");
        }

        Ok(RelayManager {
            swarm,
            registered_peers: DashMap::new(),
            peer_identities: DashMap::new(),
            message_sender,
            message_receiver,
            external_ip,
            registry,
            // Message queue fields
            message_queue: VecDeque::new(),
            max_retry_attempts: 5,
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

    /// Handle identity registration with conflict resolution
    pub async fn handle_identity_registration(&mut self, identity: String, new_peer_id: PeerId) {
        // Check if this identity is already registered to a different peer
        if let Some(existing_peer_id) = self.registered_peers.get(&identity) {
            let existing_peer_id = *existing_peer_id.value();
            
            if existing_peer_id != new_peer_id {
                println!("⚠️  Identity conflict detected for {}: existing peer {} vs new peer {}", 
                    identity, existing_peer_id, new_peer_id);
                
                // Check if the existing peer is still connected
                if self.swarm.is_connected(&existing_peer_id) {
                    println!("🔄 Disconnecting stale peer {} to allow new peer {} for identity {}", 
                        existing_peer_id, new_peer_id, identity);
                    
                    // Disconnect the old peer
                    let _ = self.swarm.disconnect_peer_id(existing_peer_id);
                    
                    // Clean up the old mapping
                    self.peer_identities.remove(&existing_peer_id);
                } else {
                    println!("🧹 Cleaning up stale mapping for disconnected peer {} with identity {}", 
                        existing_peer_id, identity);
                    
                    // Clean up the stale mapping
                    self.peer_identities.remove(&existing_peer_id);
                }
            }
        }
        
        // Register the new peer with this identity
        self.register_peer(identity, new_peer_id);
    }

    pub fn unregister_peer(&mut self, peer_id: &PeerId) {
        if let Some((_, identity)) = self.peer_identities.remove(peer_id) {
            println!("🔄 Peer {} with PeerId: {} unregistered - will update peer discovery information", identity, peer_id);
            self.registered_peers.remove(&identity);
        }
    }

    pub fn find_peer_by_identity(&self, identity: &str) -> Option<PeerId> {
        self.registered_peers.get(identity).map(|entry| *entry.value())
    }

    pub fn find_identity_by_peer(&self, peer_id: &PeerId) -> Option<String> {
        self.peer_identities.get(peer_id).map(|entry| entry.value().clone())
    }

    /// Create a simple unencrypted acknowledgment message in response to a request
    fn create_simple_ack_message(original_request: &ShinkaiMessage) -> ShinkaiMessage {
        // Create the acknowledgment message data
        let ack_data = ShinkaiData {
            message_raw_content: "ACK".to_string(),
            message_content_schema: MessageSchemaType::TextContent,
        };

        // Create internal metadata with no encryption
        let internal_metadata = InternalMetadata {
            sender_subidentity: String::new(),
            recipient_subidentity: String::new(),
            inbox: String::new(),
            signature: String::new(),
            encryption: EncryptionMethod::None,
            node_api_data: None,
        };

        // Create the message body
        let body = ShinkaiBody {
            message_data: MessageData::Unencrypted(ack_data),
            internal_metadata,
        };

        // Create external metadata (swap sender and recipient from original)
        let external_metadata = ExternalMetadata {
            sender: original_request.external_metadata.recipient.clone(),
            recipient: original_request.external_metadata.sender.clone(),
            scheduled_time: ShinkaiStringTime::generate_time_now(),
            signature: String::new(),
            intra_sender: String::new(),
            other: String::new(),
        };

        // Create the complete message
        ShinkaiMessage {
            body: MessageBody::Unencrypted(body),
            external_metadata,
            encryption: EncryptionMethod::None,
            version: ShinkaiVersion::V1_0,
        }
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
        let mut message_retry_timer = tokio::time::interval(Duration::from_secs(2));
        
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
                
                // Process message queue for retry
                _ = message_retry_timer.tick() => {
                    self.process_message_queue().await?;
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
                println!("📡 Listening on {}", address);
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                println!("🌐 External address confirmed: {}", address);
            }
            SwarmEvent::ExternalAddrExpired { address } => {
                println!("⚠️ External address expired: {}", address);
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
                             self.handle_identity_registration(verified_identity, peer_id).await;
                         } else {
                             let possible_identity = if info.agent_version.ends_with("shinkai") {
                                 if let Some(identity_part) = info.agent_version.split("@@").nth(1) {
                                     Some(format!("@@{}", identity_part))
                                 } else { None }
                             } else { None };
                             
                             if let Some(identity) = possible_identity {
                                 println!("❌ Verification failed, registering peer {} with identity: {}", peer_id, identity);
                                 self.handle_identity_registration(identity, peer_id).await;
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
                println!("📶 Ping event: {:?}", ping_event);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Relay(relay_event)) => {
                println!("📦 Relay event: {:?}", relay_event);
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Dcutr(dcutr_event)) => {
                println!("🔄 DCUtR event: {:?}", dcutr_event);
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
                                
                                // Use the new delivery method that handles both relay and direct connections
                                if let Err(e) = self.deliver_message_to_target(peer, &target_node, request, channel).await {
                                    println!("   Failed to deliver message to {}: {:?}", target_node, e);
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                println!("🔄 Relay: Received direct message response from peer {}: {:?}", peer, response);

                                // TODO: Handle response from target peer.
                                // let _ = self.swarm.behaviour_mut().request_response.send_response(channel, response);
                            }
                        }
                    }
                    request_response::Event::OutboundFailure { peer, error, .. } => {
                        println!("❌ Outbound failure to {}: {:?}", peer, error);
                    }
                    request_response::Event::InboundFailure { peer, error, .. } => {
                        println!("❌ Inbound failure from {}: {:?}", peer, error);
                    }
                    request_response::Event::ResponseSent { peer, .. } => {
                        println!("✅ Response sent to {}", peer);
                    }
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("🔗 Connected to peer: {}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, num_established, .. } => {
                println!(
                    "❌ Disconnected from peer: {}, reason: {:?}, remaining connections: {}",
                    peer_id, cause, num_established
                );
                if num_established == 0 {
                    self.unregister_peer(&peer_id);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Check if a target identity is configured for direct connection vs relay
    async fn check_target_connection_type(&mut self, target_identity: &str) -> Result<(bool, Vec<String>), LibP2PRelayError> {
        match self.registry.get_identity_record(target_identity.to_string(), None).await {
            Ok(identity_record) => {
                println!("🔄 Identity record for {}: {:?}", target_identity, identity_record);
                let is_direct = !identity_record.routing;
                let addresses = identity_record.address_or_proxy_nodes;
                Ok((is_direct, addresses))
            }
            Err(e) => {
                println!("❌ Failed to get identity record for {}: {}", target_identity, e);
                // Default to assuming it's a relay connection
                Ok((false, vec![]))
            }
        }
    }

    /// Attempt to deliver a message to a target identity
    async fn deliver_message_to_target(
        &mut self,
        source_peer: PeerId,
        target_identity: &str,
        message: ShinkaiMessage,
        channel: request_response::ResponseChannel<ShinkaiMessage>,
    ) -> Result<(), LibP2PRelayError> {
        // First check if target is connected as a relay peer
        if let Some(target_peer_id) = self.find_peer_by_identity(target_identity) {
            println!("   Forwarding to connected relay peer: {}", target_peer_id);
            
            // Forward the request to the target peer
            let _ = self.swarm
                .behaviour_mut()
                .request_response
                .send_request(&target_peer_id, message.clone());
            
            // Send acknowledgment back to sender
            let ack_message = Self::create_simple_ack_message(&message);
            let _ = self.swarm.behaviour_mut().request_response.send_response(channel, ack_message);
            return Ok(());
        }

        // Check if target is configured for direct connection
        match self.check_target_connection_type(target_identity).await {
            Ok((is_direct, addresses)) => {
                if is_direct && !addresses.is_empty() {
                    println!("   Target {} is configured for direct connection, attempting to dial: {:?}", target_identity, addresses);
                    
                    // Try to dial the direct address
                    for addr_str in &addresses {
                        if let Ok(multiaddr) = format!("/ip4/{}/tcp/{}", 
                            addr_str.split(':').next().unwrap_or(""), 
                            addr_str.split(':').nth(1).unwrap_or("8080")
                        ).parse::<libp2p::Multiaddr>() {
                            
                            println!("   Dialing direct address: {}", multiaddr);
                            if let Err(e) = self.swarm.dial(multiaddr.clone()) {
                                println!("   Failed to dial {}: {}", multiaddr, e);
                            } else {
                                // Queue the message for retry once connection is established
                                let queued_message = QueuedRelayMessage {
                                    source_peer,
                                    target_identity: target_identity.to_string(),
                                    message: message.clone(),
                                    retry_count: 0,
                                    last_attempt: std::time::Instant::now(),
                                };
                                self.message_queue.push_back(queued_message);
                                
                                // Send acknowledgment that we're attempting delivery
                                let ack_message = Self::create_simple_ack_message(&message);
                                let _ = self.swarm.behaviour_mut().request_response.send_response(channel, ack_message);
                                return Ok(());
                            }
                        }
                    }
                } else {
                    println!("   Target {} not configured for direct connection", target_identity);
                }
            }
            Err(e) => {
                println!("   Failed to check connection type for {}: {:?}", target_identity, e);
            }
        }

        // If we get here, we couldn't deliver the message
        println!("   Target peer {} not found and not reachable", target_identity);
        // TODO: Send not found response to sender
        Ok(())
    }

    /// Process the message queue and retry delivering failed messages
    async fn process_message_queue(&mut self) -> Result<(), LibP2PRelayError> {
        if self.message_queue.is_empty() {
            return Ok(());
        }

        println!("📦 Processing relay message queue with {} messages", self.message_queue.len());

        let mut messages_to_retry = Vec::new();
        let mut messages_to_remove = Vec::new();
        let now = std::time::Instant::now();

        // Check each message in the queue
        for (index, queued_message) in self.message_queue.iter().enumerate() {
            // Wait at least 1 second between retry attempts
            if now.duration_since(queued_message.last_attempt) < Duration::from_secs(1) {
                continue;
            }

            // Check if we've exceeded max retry attempts
            if queued_message.retry_count >= self.max_retry_attempts {
                println!("   Message to {} exceeded max retry attempts ({}), dropping", 
                    queued_message.target_identity, self.max_retry_attempts);
                messages_to_remove.push(index);
                continue;
            }

            // Check if target peer is now connected
            if let Some(target_peer_id) = self.find_peer_by_identity(&queued_message.target_identity) {
                println!("   Target peer {} now connected, will retry delivery", queued_message.target_identity);
                messages_to_retry.push(index);
            }
        }

        // Remove messages that exceeded retry limits (in reverse order to maintain indices)
        for &index in messages_to_remove.iter().rev() {
            self.message_queue.remove(index);
        }

        // Retry messages for connected peers (in reverse order to maintain indices)
        for &index in messages_to_retry.iter().rev() {
            if let Some(queued_message) = self.message_queue.remove(index) {
                if let Some(target_peer_id) = self.find_peer_by_identity(&queued_message.target_identity) {
                    println!("   🔄 Retrying message delivery to {} (attempt {})", 
                        queued_message.target_identity, queued_message.retry_count + 1);
                    
                    // Attempt to send the message
                    let _ = self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_request(&target_peer_id, queued_message.message);
                }
            }
        }

        if !self.message_queue.is_empty() {
            println!("📦 Message queue still has {} messages pending", self.message_queue.len());
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