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
use x25519_dalek::{StaticSecret as EncryptionStaticKey};
use std::time::{Duration, Instant};
use dashmap::DashMap;
use shinkai_crypto_identities::ShinkaiRegistry;
use crate::LibP2PRelayError;

/// A queued message waiting to be delivered
#[derive(Debug)]
pub struct QueuedRelayMessage {
    pub source_peer: PeerId,
    pub target_identity: String,
    pub message: ShinkaiMessage,
    pub retry_count: u32,
    pub last_attempt: std::time::Instant,
}

/// Connection health tracking for monitoring connection quality
#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    pub last_activity: Instant,
    pub ping_failures: u32,
    pub bytes_transferred: u64,
    pub connection_established: Instant,
    pub last_ping_rtt: Option<Duration>,
}

impl ConnectionHealth {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_activity: now,
            ping_failures: 0,
            bytes_transferred: 0,
            connection_established: now,
            last_ping_rtt: None,
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn record_ping_success(&mut self, rtt: Duration) {
        self.ping_failures = 0;
        self.last_ping_rtt = Some(rtt);
        self.update_activity();
    }

    pub fn record_ping_failure(&mut self) {
        self.ping_failures += 1;
        self.update_activity();
    }

    pub fn is_idle(&self, idle_timeout: Duration) -> bool {
        self.last_activity.elapsed() > idle_timeout
    }

    pub fn is_unhealthy(&self, max_ping_failures: u32) -> bool {
        self.ping_failures >= max_ping_failures
    }
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

pub struct RelayManagerConfig {
    pub listen_port: u16,
    pub relay_node_name: String,
    pub identity_secret_key: SigningKey,
    pub encryption_secret_key: EncryptionStaticKey,
}

pub struct RelayManager {
    swarm: Swarm<RelayBehaviour>,
    registered_peers: DashMap<String, PeerId>, // identity -> peer_id
    peer_identities: DashMap<PeerId, String>,  // peer_id -> identity
    request_response_channels: DashMap<request_response::OutboundRequestId, request_response::ResponseChannel<ShinkaiMessage>>, // request_id -> channel
    external_ip: Option<std::net::IpAddr>, // Store detected external IP
    registry: ShinkaiRegistry, // Blockchain registry for identity verification
    config: RelayManagerConfig,
    // Connection health monitoring
    connection_health: DashMap<PeerId, ConnectionHealth>,
    idle_timeout: Duration,
    max_ping_failures: u32,
    // Identity verification caching  
    identity_cache: DashMap<String, (shinkai_crypto_identities::OnchainIdentity, Instant)>,
    cache_ttl: Duration,
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
        encryption_secret_key: EncryptionStaticKey,
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
            relay_node_name,
            env!("CARGO_PKG_VERSION")
        ))
        .with_interval(Duration::from_secs(60))
        .with_push_listen_addr_updates(true)
        .with_cache_size(100)
        .with_hide_listen_addrs(true));

        // Configure ping protocol with faster intervals for better connection monitoring
        let ping = ping::Behaviour::new(
            ping::Config::new()
                .with_interval(Duration::from_secs(5))  // Reduced from 10s to 5s
                .with_timeout(Duration::from_secs(10))  // Add explicit timeout
        );

        // Configure relay protocol with increased limits
        let relay_config = relay::Config {
            reservation_duration: Duration::from_secs(1800), // 30 minutes
            reservation_rate_limiters: Vec::new(), // No rate limiting for now
            circuit_src_rate_limiters: Vec::new(), // No rate limiting for now  
            max_reservations: 1024, // Allow up to 1024 concurrent reservations
            max_reservations_per_peer: 16, // Allow 16 reservations per peer
            max_circuits: 1024, // Allow up to 1024 concurrent circuits
            max_circuits_per_peer: 16, // Allow 16 circuits per peer
            max_circuit_duration: Duration::from_secs(3600), // 1 hour max circuit duration
            max_circuit_bytes: 1024 * 1024 * 1024, // 1GB max circuit data transfer
        };
        let relay = relay::Behaviour::new(local_peer_id, relay_config);

        // Configure DCUtR for hole punching through relay
        let dcutr = dcutr::Behaviour::new(local_peer_id);

        // Configure request-response behavior with reduced timeout for faster failure detection
        let request_response = request_response::json::Behaviour::new(
            std::iter::once((libp2p::StreamProtocol::new("/shinkai/message/1.0.0"), request_response::ProtocolSupport::Full)),
            request_response::Config::default()
                .with_request_timeout(Duration::from_secs(30))
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
            request_response_channels: DashMap::new(),
            external_ip,
            registry,
            config: RelayManagerConfig {
                listen_port,
                relay_node_name,
                identity_secret_key,
                encryption_secret_key,
            },
            // Initialize connection health monitoring
            connection_health: DashMap::new(),
            idle_timeout: Duration::from_secs(300), // 5 minutes idle timeout
            max_ping_failures: 3,
            // Initialize identity verification caching
            identity_cache: DashMap::new(),
            cache_ttl: Duration::from_secs(600), // 10 minutes cache TTL
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

    /// Verify a peer's identity by checking their public key against the blockchain registry
    /// Uses caching to prevent blocking on repeated lookups
    async fn verify_peer_identity_internal(
        &mut self,
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

        // Check cache first
        if let Some(cache_entry) = self.identity_cache.get(identity_string) {
            let (cached_record, cached_time) = cache_entry.value();
            if cached_time.elapsed() < self.cache_ttl {
                println!("🔍 Using cached identity record for {}", identity_string);
                if let Ok(registry_public_key) = cached_record.signature_verifying_key() {
                    if registry_public_key == peer_public_key {
                        println!("✅ Identity verification successful (cached): {} matches public key", identity_string);
                        return Some(identity_string.clone());
                    }
                }
                println!("❌ Cached identity verification failed for {}", identity_string);
                return None;
            } else {
                // Drop the reference before removing
                drop(cache_entry);
                // Remove expired cache entry
                self.identity_cache.remove(identity_string);
            }
        }
        
        // Fetch from registry and cache the result
        match self.registry.get_identity_record(identity_string.clone(), None).await {
            Ok(identity_record) => {
                // Cache the successful lookup
                self.identity_cache.insert(identity_string.clone(), (identity_record.clone(), Instant::now()));
                
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
        let mut cleanup_timer = tokio::time::interval(Duration::from_secs(30)); // Clean up every 30 seconds
        let mut cache_cleanup_timer = tokio::time::interval(Duration::from_secs(300)); // Clean cache every 5 minutes
        
        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await?;
                }
                // Periodic cleanup of idle connections
                _ = cleanup_timer.tick() => {
                    self.cleanup_idle_connections().await;
                }
                // Periodic cleanup of identity cache
                _ = cache_cleanup_timer.tick() => {
                    self.cleanup_identity_cache();
                }
            }
        }
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
                
                // Check if this peer is already registered to avoid unnecessary re-processing
                if let Some(existing_identity) = self.find_identity_by_peer(&peer_id) {
                    println!("🔄 Peer {} already registered with identity: {} - skipping re-identification", peer_id, existing_identity);
                    return Ok(());
                }
                
                // Extract the peer's public key from the libp2p identity  
                // Get the raw public key bytes and try to create an ed25519_dalek::VerifyingKey
                let public_key_bytes = info.public_key.encode_protobuf();
                
                // For Ed25519, the protobuf encoding includes a prefix, so we need to extract just the key bytes
                // The public key should be 32 bytes for Ed25519
                if public_key_bytes.len() >= 32 {
                    let key_bytes = &public_key_bytes[public_key_bytes.len() - 32..];
                    if let Ok(verifying_key) = ed25519_dalek::VerifyingKey::from_bytes(&key_bytes.try_into().unwrap_or([0u8; 32])) {
                        // Verify the peer's identity using blockchain registry (with caching)
                        if let Some(verified_identity) = self.verify_peer_identity_internal(verifying_key, info.agent_version.clone()).await {
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
            }      
            SwarmEvent::Behaviour(RelayBehaviourEvent::Ping(ping_event)) => {
                match ping_event {
                    ping::Event { peer, result: Ok(rtt), .. } => {
                        println!("📶 Ping success to {}: {:?}", peer, rtt);
                        // Update connection health with successful ping
                        if let Some(mut health) = self.connection_health.get_mut(&peer) {
                            health.record_ping_success(rtt);
                        }
                    }
                    ping::Event { peer, result: Err(failure), .. } => {
                        println!("📶 Ping failure to {}: {:?}", peer, failure);
                        // Update connection health with ping failure
                        if let Some(mut health) = self.connection_health.get_mut(&peer) {
                            health.record_ping_failure();
                            
                            // Disconnect unhealthy connections
                            if health.is_unhealthy(self.max_ping_failures) {
                                println!("❌ Disconnecting unhealthy peer {} after {} ping failures", 
                                    peer, health.ping_failures);
                                let _ = self.swarm.disconnect_peer_id(peer);
                            }
                        }
                    }
                }
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
                            request_response::Message::Request { mut request, channel, .. } => {
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
                                
                                if request.external_metadata.sender.starts_with("@@localhost.") {
                                    println!("🔄 Relay: We need to re-encrypt message from localhost to {}", target_node);
                                    let original_sender = request.external_metadata.sender.clone();

                                    // Check if the message is body-encrypted (likely encrypted with relay's key due to proxy logic)
                                    let is_body_encrypted = matches!(request.body, shinkai_message_primitives::shinkai_message::shinkai_message::MessageBody::Encrypted(_));
                                    println!("🔍 Debug: is_body_encrypted = {}", is_body_encrypted);
                                    if is_body_encrypted {
                                        println!("🔓 Relay: Message from localhost is body-encrypted, attempting to decrypt and re-encrypt for recipient");
                                        self.relay_message_encryption(&mut request, &target_node).await;
                                    }

                                    // Preserve original sender in intra_sender so the recipient knows who originated the request
                                    request.external_metadata.intra_sender = original_sender;

                                    // Replace outer sender with the relay identity so signature verification succeeds
                                    request.external_metadata.sender = self.config.relay_node_name.clone();

                                    // Re-sign outer layer with the relay identity key
                                    if let Ok(resigned) = request.sign_outer_layer(&self.config.identity_secret_key) {
                                        request = resigned;
                                    } else {
                                        println!("❌ Failed to re-sign message from localhost");
                                    }
                                }

                                // Forward to target and store channel for response
                                if let Some(target_peer_id) = self.find_peer_by_identity(&target_node) {
                                    let outbound_id = self.swarm.behaviour_mut().request_response.send_request(&target_peer_id, request);
                                    self.request_response_channels.insert(outbound_id, channel);
                                } else {
                                    // Target not found, send error response
                                    println!("❌ Target not found for request from {} to {}", request.external_metadata.sender, request.external_metadata.recipient);
                                    request.external_metadata.other = "Target not found".to_string();
                                    let _ = self.swarm.behaviour_mut().request_response.send_response(channel, request);
                                }
                            }
                            request_response::Message::Response { response, request_id, .. } => {
                                println!("🔄 Relay: Received direct message response from peer {}", peer);
                                println!("   Message from: {} to: {}", 
                                    response.external_metadata.sender,
                                    response.external_metadata.recipient);                                

                                if let Some((_, channel)) = self.request_response_channels.remove(&request_id) {
                                    let _ = self.swarm.behaviour_mut().request_response.send_response(channel, response);
                                }
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
                // Initialize connection health tracking
                self.connection_health.insert(peer_id, ConnectionHealth::new());
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, num_established, .. } => {
                println!(
                    "❌ Disconnected from peer: {}, reason: {:?}, remaining connections: {}",
                    peer_id, cause, num_established
                );
                if num_established == 0 {
                    self.unregister_peer(&peer_id);
                    // Clean up connection health tracking
                    self.connection_health.remove(&peer_id);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Clean up idle and unhealthy connections
    async fn cleanup_idle_connections(&mut self) {
        let mut peers_to_disconnect = Vec::new();
        
        // Check each tracked connection for health issues
        for entry in self.connection_health.iter() {
            let peer_id = *entry.key();
            let health = entry.value();
            
            if health.is_idle(self.idle_timeout) {
                println!("🧹 Marking idle peer {} for disconnection (idle for {:?})", 
                    peer_id, health.last_activity.elapsed());
                peers_to_disconnect.push(peer_id);
            } else if health.is_unhealthy(self.max_ping_failures) {
                println!("🧹 Marking unhealthy peer {} for disconnection ({} ping failures)", 
                    peer_id, health.ping_failures);
                peers_to_disconnect.push(peer_id);
            }
        }
        
        // Disconnect identified peers
        for peer_id in peers_to_disconnect {
            println!("🧹 Disconnecting peer {} due to health issues", peer_id);
            let _ = self.swarm.disconnect_peer_id(peer_id);
            self.connection_health.remove(&peer_id);
        }
    }
    
    /// Clean up expired identity cache entries
    fn cleanup_identity_cache(&mut self) {
        let now = Instant::now();
        let mut expired_keys = Vec::new();
        
        for entry in self.identity_cache.iter() {
            let key = entry.key().clone();
            let (_, cached_time) = entry.value();
            
            if now.duration_since(*cached_time) > self.cache_ttl {
                expired_keys.push(key);
            }
        }
        
        for key in expired_keys {
            self.identity_cache.remove(&key);
            println!("🧹 Removed expired identity cache entry for: {}", key);
        }
        
        if !self.identity_cache.is_empty() {
            println!("🧹 Identity cache cleanup complete. {} entries remaining", self.identity_cache.len());
        }
    }

async fn relay_message_encryption(&mut self, request: &mut ShinkaiMessage, target_node: &String) {
        // Parse recipient name
        let recipient_name = match shinkai_message_primitives::schemas::shinkai_name::ShinkaiName::new(target_node.clone()) {
            Ok(name) => name,
            Err(_) => {
                println!("❌ Relay: Failed to parse recipient name");
                return;
            }
        };

        // Get recipient's identity from registry
        let recipient_node_name = recipient_name.get_node_name_string();
        let recipient_identity = match self.registry.get_identity_record(recipient_node_name.clone(), None).await {
            Ok(identity) => identity,
            Err(e) => {
                println!("❌ Relay: Failed to get recipient's identity from registry: {}", e);
                return;
            }
        };

        // Parse recipient's encryption key
        let recipient_enc_key = match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&recipient_identity.encryption_key) {
            Ok(key) => key,
            Err(_) => {
                println!("❌ Relay: Failed to parse recipient's encryption key");
                return;
            }
        };

        // Check if 'other' field has original sender's encryption key
        if request.external_metadata.other.is_empty() {
            println!("❌ Relay: 'other' field is empty, cannot get original sender's encryption key");
            return;
        }

        // Parse original sender's encryption key
        let original_sender_enc_key = match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&request.external_metadata.other) {
            Ok(key) => key,
            Err(_) => {
                println!("❌ Relay: Failed to parse original sender's encryption key from 'other' field");
                return;
            }
        };

        // Decrypt the message using relay's private key and original sender's public key
        let decrypted_message = match request.decrypt_outer_layer(&self.config.encryption_secret_key, &original_sender_enc_key) {
            Ok(message) => message,
            Err(e) => {
                println!("❌ Relay: Failed to decrypt message: {}", e);
                return;
            }
        };

        // Re-encrypt with recipient's key
        match decrypted_message.encrypt_outer_layer(&self.config.encryption_secret_key, &recipient_enc_key) {
            Ok(re_encrypted_message) => {
                println!("✅ Relay: Successfully decrypted and re-encrypted message for recipient");
                *request = re_encrypted_message;
            },
            Err(e) => {
                println!("❌ Relay: Failed to re-encrypt message: {}", e);
            }
        }
    }

}

// This allows the noise configuration to work
impl From<noise::Error> for LibP2PRelayError {
    fn from(e: noise::Error) -> Self {
        LibP2PRelayError::LibP2PError(format!("Noise error: {}", e))
    }
} 