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
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_message::shinkai_message::{MessageBody, MessageData, ShinkaiMessage};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::schemas::agent_network_offering::AgentNetworkOfferingRequest;
use shinkai_message_primitives::shinkai_utils::encryption::encryption_public_key_to_string;
use x25519_dalek::{StaticSecret as EncryptionStaticKey};
use std::time::{Duration, Instant, SystemTime};
use dashmap::DashMap;
use shinkai_crypto_identities::ShinkaiRegistry;
use crate::LibP2PRelayError;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use serde_json::Value;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealth {
    #[serde(with = "serde_system_time")]
    pub last_activity: SystemTime,
    pub ping_failures: u32,
    /// Number of bytes transferred (not currently tracked, placeholder for future implementation)
    pub bytes_transferred: u64,
    #[serde(with = "serde_system_time")]
    pub connection_established: SystemTime,
    /// Last ping round-trip time in milliseconds
    #[serde(with = "serde_duration")]
    pub last_ping_rtt: Duration,
}

/// HTTP payload for node status updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatusPayload {
    pub online: bool,
    #[serde(rename = "connectionHealth")]
    pub connection_health: Option<ConnectionHealth>,
    pub identity: Option<String>,
}

/// HTTP payload for node offerings updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOfferingsPayload {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "peerId")]
    pub peer_id: String,
    pub offering: Value,
}

// Custom serialization modules for SystemTime and Duration
mod serde_system_time {
    use super::*;
    use serde::{Serializer, Deserializer};
    
    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let datetime: DateTime<Utc> = (*time).into();
        serializer.serialize_str(&datetime.to_rfc3339())
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let datetime_str: String = String::deserialize(deserializer)?;
        let datetime = DateTime::parse_from_rfc3339(&datetime_str)
            .map_err(serde::de::Error::custom)?;
        Ok(datetime.into())
    }
}

mod serde_duration {
    use super::*;
    use serde::{Serializer, Deserializer};
    
    pub fn serialize<S>(duration_opt: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(duration_opt.as_secs_f64() * 1000.0)
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis: f64 = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(millis / 1000.0))
    }
}

impl ConnectionHealth {
    pub fn new() -> Self {
        let now = SystemTime::now();
        Self {
            last_activity: now,
            ping_failures: 0,
            bytes_transferred: 0,
            connection_established: now,
            last_ping_rtt: Duration::from_secs(0),
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    pub fn record_ping_success(&mut self, rtt: Duration) {
        self.ping_failures = 0;
        self.last_ping_rtt = rtt;
        self.update_activity();
    }

    pub fn record_ping_failure(&mut self) {
        self.ping_failures += 1;
        self.update_activity();
    }

    pub fn is_idle(&self, idle_timeout: Duration) -> bool {
        self.last_activity.elapsed().unwrap_or(Duration::ZERO) > idle_timeout
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
    pub status_endpoint_url: Option<String>,
    pub ping_interval_secs: u64,
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
    // HTTP client for status updates
    http_client: reqwest::Client,
    // Identify event deduplication to reduce excessive processing
    last_identify_events: DashMap<PeerId, Instant>,
    identify_event_cooldown: Duration,
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
        status_endpoint_url: Option<String>,
    ) -> Result<Self, LibP2PRelayError> {
        // Detect external IP address first
        let external_ip = Self::detect_external_ip().await;

        // Read ping interval from environment variable, default to 10 seconds
        let ping_interval_secs: u64 = std::env::var("PING_INTERVAL_SECS")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .expect("Failed to parse PING_INTERVAL_SECS");
        
        println!("📶 Ping interval set to {} seconds", ping_interval_secs);

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
        // Use conservative settings to reduce identify event frequency
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shinkai/1.0.0".to_string(),
            local_key.public(),
        ).with_agent_version(format!(
            "shinkai-relayer/{}/{}",
            relay_node_name,
            env!("CARGO_PKG_VERSION")
        ))
        .with_interval(Duration::from_secs(300)) // Reduced from 60s to 5 minutes
        .with_push_listen_addr_updates(false)    // Disabled to reduce events
        .with_cache_size(100)
        .with_hide_listen_addrs(true));

        // Configure ping protocol with configurable interval for connection monitoring
        let ping = ping::Behaviour::new(
            ping::Config::new()
                .with_interval(Duration::from_secs(ping_interval_secs))
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

        // Create swarm with proper configuration and connection limits
        let swarm_config = Config::with_tokio_executor()
            .with_idle_connection_timeout(Duration::from_secs(180)); // Close idle connections after 3 minutes
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, swarm_config);

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
                status_endpoint_url,
                ping_interval_secs: ping_interval_secs,
            },
            // Initialize connection health monitoring
            connection_health: DashMap::new(),
            idle_timeout: Duration::from_secs(300), // 5 minutes idle timeout
            max_ping_failures: 3,
            // Initialize identity verification caching
            identity_cache: DashMap::new(),
            cache_ttl: Duration::from_secs(600), // 10 minutes cache TTL
            // Initialize HTTP client
            http_client: reqwest::Client::new(),
            // Initialize identify event deduplication
            last_identify_events: DashMap::new(),
            identify_event_cooldown: Duration::from_secs(30), // 30 second cooldown between identify processing
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

    /// Set the status endpoint URL for node health reporting
    pub fn set_status_endpoint_url(&mut self, url: String) {
        self.config.status_endpoint_url = Some(url);
    }

    /// Post node status to the configured HTTP endpoint (non-blocking)
    fn post_node_status(&self, peer_id: PeerId, online: bool, connection_health: Option<ConnectionHealth>) {
        if let Some(ref endpoint_url) = self.config.status_endpoint_url {
            let identity = match self.find_identity_by_peer(&peer_id) {
                Some(identity) => identity,
                None => {
                    println!("⚠️ No identity found for peer {}, using peer ID as identity", peer_id);
                    peer_id.to_string()
                }
            };

            let payload = NodeStatusPayload {
                online,
                connection_health,
                identity: Some(identity),
            };

            let url = format!("{}/dapps/nodes/{}", endpoint_url, peer_id);
            let client = self.http_client.clone();
            
            // Spawn the HTTP request as a separate task to avoid blocking the event loop
            tokio::spawn(async move {
                let mut request_builder = client.post(&url).json(&payload);
                
                // Add Authorization header if STATUS_ENDPOINT_TOKEN is set
                if let Ok(token) = std::env::var("STATUS_ENDPOINT_TOKEN") {
                    request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
                }
                
                match request_builder.send().await {
                    Ok(response) => {
                        if !response.status().is_success() {
                            println!("⚠️ Failed to post node status for peer {}: HTTP {}", peer_id, response.status());
                        }
                    }
                    Err(e) => {
                        println!("❌ Error posting node status for peer {}: {}", peer_id, e);
                    }
                }
            });
        }
    }

    /// Fetch current offerings for a peer and synchronize with local database
    fn sync_peer_offerings(&self, peer_id: PeerId, new_offerings: Vec<Value>) {
        if let Some(endpoint_url) = self.config.status_endpoint_url.clone() {
            Self::spawn_sync_offerings_task(
                self.http_client.clone(),
                endpoint_url,
                peer_id,
                new_offerings,
            );
        }
    }

    /// Static function to handle offerings synchronization in a separate task
    fn spawn_sync_offerings_task(
        client: reqwest::Client,
        endpoint_url: String,
        peer_id: PeerId,
        new_offerings: Vec<Value>,
    ) {
        tokio::spawn(async move {
            let offerings_url = format!("{}/dapps/offerings/peerId/{}", endpoint_url, peer_id);
            let mut request_builder = client.get(&offerings_url);
            
            // Add Authorization header if STATUS_ENDPOINT_TOKEN is set
            if let Ok(token) = std::env::var("STATUS_ENDPOINT_TOKEN") {
                request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
            }
            
            match request_builder.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<Vec<Value>>().await {
                            Ok(existing_offerings) => {
                                println!("✅ Fetched {} existing offerings for peer {}", existing_offerings.len(), peer_id);
                                
                                // Find offerings that exist in database but not in new offerings
                                let mut stale_offering_ids = Vec::new();
                                
                                for existing_offering in &existing_offerings {
                                    if let Some(offering_id) = existing_offering.get("id").and_then(|v| v.as_str()) {
                                        // Check if this offering exists in the new offerings
                                        let found_in_new = new_offerings.iter().any(|new_offering| {
                                            // Compare offerings by some unique identifier (e.g., tool_key or name)
                                            if let (Some(existing_tool_key), Some(new_tool_key)) = (
                                                existing_offering.get("tool_key").and_then(|v| v.as_str()),
                                                new_offering.get("tool_key").and_then(|v| v.as_str())
                                            ) {
                                                existing_tool_key == new_tool_key
                                            } else if let (Some(existing_name), Some(new_name)) = (
                                                existing_offering.get("name").and_then(|v| v.as_str()),
                                                new_offering.get("name").and_then(|v| v.as_str())
                                            ) {
                                                existing_name == new_name
                                            } else {
                                                false
                                            }
                                        });
                                        
                                        if !found_in_new {
                                            stale_offering_ids.push(offering_id.to_string());
                                        }
                                    }
                                }
                                
                                // Delete stale offerings
                                for offering_id in stale_offering_ids {
                                    let delete_url = format!("{}/dapps/offerings/{}", endpoint_url, offering_id);
                                    let mut delete_request_builder = client.delete(&delete_url);
                                    
                                    // Add Authorization header if STATUS_ENDPOINT_TOKEN is set
                                    if let Ok(token) = std::env::var("STATUS_ENDPOINT_TOKEN") {
                                        delete_request_builder = delete_request_builder.header("Authorization", format!("Bearer {}", token));
                                    }
                                    
                                    match delete_request_builder.send().await {
                                        Ok(delete_response) => {
                                            if delete_response.status().is_success() {
                                                println!("✅ Successfully deleted stale offering {} for peer {}", offering_id, peer_id);
                                            } else {
                                                println!("⚠️ Failed to delete stale offering {} for peer {}: HTTP {}", offering_id, peer_id, delete_response.status());
                                            }
                                        }
                                        Err(e) => {
                                            println!("❌ Error deleting stale offering {} for peer {}: {}", offering_id, peer_id, e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                println!("❌ Failed to parse offerings response for peer {}: {}", peer_id, e);
                            }
                        }
                    } else {
                        println!("⚠️ Failed to fetch offerings for peer {}: HTTP {}", peer_id, response.status());
                    }
                }
                Err(e) => {
                    println!("❌ Error fetching offerings for peer {}: {}", peer_id, e);
                }
            }
        });
    }


    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Request tool offerings from all connected non-localhost peers
    async fn request_tool_offerings_from_all_peers(&mut self) {
        let connected_peers: Vec<PeerId> = self.peer_identities.iter()
            .filter_map(|entry| {
                let peer_id = *entry.key();
                // Only include peers that are:
                // 1. Still connected
                // 2. Not localhost peers
                if self.swarm.is_connected(&peer_id) && !self.is_localhost_peer(&peer_id) {
                    Some(peer_id)
                } else {
                    None
                }
            })
            .collect();

        if connected_peers.is_empty() {
            println!("🔧 No connected non-localhost peers to request tool offerings from");
            return;
        }

        println!("🔧 Requesting tool offerings from {} connected non-localhost peers", connected_peers.len());
        
        for peer_id in connected_peers {
            self.request_tool_offerings(peer_id).await;
        }
    }

    /// Request tool offerings from a connected node
    async fn request_tool_offerings(&mut self, peer_id: PeerId) {
        if let Some(identity) = self.find_identity_by_peer(&peer_id) {
            println!("🔧 Requesting tool offerings from peer {} ({})", peer_id, identity);
            
            // Get the node's encryption public key from blockchain registry
            let node_name = if let Ok(parsed_name) = shinkai_message_primitives::schemas::shinkai_name::ShinkaiName::new(identity.clone()) {
                parsed_name.get_node_name_string()
            } else {
                identity.clone()
            };

            let node_encryption_public_key = match self.registry.get_identity_record(node_name.clone(), None).await {
                Ok(identity_record) => {
                    match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&identity_record.encryption_key) {
                        Ok(key) => key,
                        Err(e) => {
                            println!("❌ Failed to parse node's encryption key for {}: {}", node_name, e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to get node's identity from registry for {}: {}", node_name, e);
                    return;
                }
            };
            
            // Create an AgentNetworkOfferingRequest message
            let request = AgentNetworkOfferingRequest {
                agent_identity: identity.clone(),
            };
            
            let request_json = match serde_json::to_string(&request) {
                Ok(json) => json,
                Err(e) => {
                    println!("❌ Failed to serialize tool offerings request: {}", e);
                    return;
                }
            };
            
            // Create the message requesting tool offerings
            // Encrypt it for the node so it can decrypt and process it
            let request_message = match ShinkaiMessageBuilder::new(
                self.config.encryption_secret_key.clone(),
                self.config.identity_secret_key.clone(),
                node_encryption_public_key, // Use node's public key for encryption
            )
            .message_raw_content(request_json)
            .message_schema_type(MessageSchemaType::AgentNetworkOfferingRequest)
            .internal_metadata(
                "main".to_string(),
                "main".to_string(),
                shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod::None,
                None,
            )
            .external_metadata(
                identity.clone(),
                self.config.relay_node_name.clone(),
            )
            .build() {
                Ok(msg) => msg,
                Err(e) => {
                    println!("❌ Failed to build tool offerings request message: {}", e);
                    return;
                }
            };

            // Send the request via the request-response protocol
            let outbound_id = self.swarm.behaviour_mut().request_response.send_request(&peer_id, request_message.clone());
            println!("📤 Sent tool offerings request to peer {} with request ID {:?}", peer_id, outbound_id);
            println!("🔧 Debug - Message sender: '{}', recipient: '{}'", 
                request_message.external_metadata.sender, 
                request_message.external_metadata.recipient);
        } else {
            println!("⚠️ No identity found for peer {}, cannot request tool offerings", peer_id);
        }
    }

    /// Handle received tool offerings response from a peer
    async fn handle_tool_offerings_response(&mut self, peer: PeerId, content: String) {
        println!("🔧 Received AgentNetworkOfferingResponse from peer {}", peer);
        
        // Parse the AgentNetworkOfferingResponse
        match serde_json::from_str::<Value>(&content) {
            Ok(offerings_array) => {
                // First, synchronize offerings (fetch existing and delete stale ones)
                if let Some(offerings_vec) = offerings_array.as_array() {
                    self.sync_peer_offerings(peer, offerings_vec.clone());
                }
                
                // Process offerings in a separate async task
                let peer_id = peer;
                let offerings_clone = offerings_array.clone();
                let endpoint_url = self.config.status_endpoint_url.clone();
                let client = self.http_client.clone();
                
                tokio::spawn(async move {
                    if let Some(ref endpoint_url) = endpoint_url {
                        // Fetch nodeId
                        let url = format!("{}/dapps/nodes/peerId/{}", endpoint_url, peer_id);
                        let mut request_builder = client.get(&url);
                        
                        // Add Authorization header if STATUS_ENDPOINT_TOKEN is set
                        if let Ok(token) = std::env::var("STATUS_ENDPOINT_TOKEN") {
                            request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
                        }
                        
                        match request_builder.send().await {
                            Ok(response) => {
                                if response.status().is_success() {
                                    match response.json::<Value>().await {
                                        Ok(json) => {
                                            if let Some(node_id) = json.get("id").and_then(|v| v.as_str()) {
                                                println!("✅ Successfully fetched nodeId {} for peer {}", node_id, peer_id);
                                                
                                                // Post new offerings
                                                let offerings_url = format!("{}/dapps/offerings", endpoint_url);
                                                if let Some(offerings_array) = offerings_clone.as_array() {
                                                    for offering in offerings_array {
                                                        let payload = NodeOfferingsPayload {
                                                            node_id: node_id.to_string(),
                                                            peer_id: peer_id.to_string(),
                                                            offering: offering.clone(),
                                                        };

                                                        let mut offerings_request_builder = client.post(&offerings_url).json(&payload);
                                                        
                                                        // Add Authorization header if STATUS_ENDPOINT_TOKEN is set
                                                        if let Ok(token) = std::env::var("STATUS_ENDPOINT_TOKEN") {
                                                            offerings_request_builder = offerings_request_builder.header("Authorization", format!("Bearer {}", token));
                                                        }
                                                        
                                                        match offerings_request_builder.send().await {
                                                            Ok(response) => {
                                                                if response.status().is_success() {
                                                                    println!("✅ Successfully posted offering for peer {}", peer_id);
                                                                } else {
                                                                    println!("⚠️ Failed to post offering for peer {}: HTTP {}", peer_id, response.status());
                                                                }
                                                            }
                                                            Err(e) => {
                                                                println!("❌ Error posting offering for peer {}: {}", peer_id, e);
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                println!("⚠️ No nodeId found in response for peer {}", peer_id);
                                            }
                                        }
                                        Err(e) => {
                                            println!("❌ Failed to parse nodeId response for peer {}: {}", peer_id, e);
                                        }
                                    }
                                } else {
                                    println!("⚠️ Failed to fetch nodeId for peer {}: HTTP {}", peer_id, response.status());
                                }
                            }
                            Err(e) => {
                                println!("❌ Error fetching nodeId for peer {}: {}", peer_id, e);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                println!("❌ Failed to parse AgentNetworkOfferingResponse from peer {}: {}", peer, e);
            }
        }
    }

    /// Handle identity registration with conflict resolution
    pub async fn handle_identity_registration(&mut self, mut identity: String, new_peer_id: PeerId, is_localhost: bool) {
        // If the identity is localhost, we need to check if the peer is localhost
        if is_localhost {
            identity = new_peer_id.to_string();
        }

        // Check for peerId conflicts first (same peerId, different identity)
        if let Some(existing_identity) = self.peer_identities.get(&new_peer_id) {
            let existing_identity = existing_identity.value().clone();
            
            if existing_identity != identity {
                println!("⚠️  PeerId conflict detected for peer {}: existing identity '{}' vs new identity '{}'", 
                    new_peer_id, existing_identity, identity);
                println!("🔄 Applying last-registration-wins strategy: replacing '{}' with '{}'", 
                    existing_identity, identity);
                
                // Remove the old identity mapping (last-registration-wins)
                self.registered_peers.remove(&existing_identity);
                
                // The peer_identities entry will be updated below
            }
        }

        // Check for identity conflicts (same identity, different peer)
        if let Some(existing_peer_id) = self.registered_peers.get(&identity) {
            let existing_peer_id = *existing_peer_id.value();
            
            if existing_peer_id != new_peer_id {
                println!("⚠️  Identity conflict detected for '{}': existing peer {} vs new peer {}", 
                    identity, existing_peer_id, new_peer_id);
                
                // Check if the existing peer is still connected
                if self.swarm.is_connected(&existing_peer_id) {
                    println!("🔄 Disconnecting existing peer {} to allow new peer {} for identity '{}' (last-registration-wins)", 
                        existing_peer_id, new_peer_id, identity);
                    
                    // Disconnect the old peer
                    let _ = self.swarm.disconnect_peer_id(existing_peer_id);
                    
                    // Clean up the old mapping
                    self.peer_identities.remove(&existing_peer_id);
                } else {
                    println!("🧹 Cleaning up stale mapping for disconnected peer {} with identity '{}'", 
                        existing_peer_id, identity);
                    
                    // Clean up the stale mapping
                    self.peer_identities.remove(&existing_peer_id);
                }
            }
        }
        
        // Register the new peer with this identity (atomic update of both mappings)
        self.registered_peers.insert(identity.clone(), new_peer_id);
        self.peer_identities.insert(new_peer_id, identity.clone());
        
        println!("✅ Successfully registered peer {} with identity '{}'", new_peer_id, identity);
    }

    pub fn unregister_peer(&mut self, peer_id: &PeerId) {
        if let Some((_, identity)) = self.peer_identities.remove(peer_id) {
            println!("🔄 Peer {} with PeerId: {} unregistered - will update peer discovery information", identity, peer_id);
            self.registered_peers.remove(&identity);
            // Clean up identify event tracking
            self.last_identify_events.remove(peer_id);
        }
    }

    pub fn find_peer_by_identity(&self, identity: &str) -> Option<PeerId> {
        self.registered_peers.get(identity).map(|entry| *entry.value())
    }

    pub fn find_identity_by_peer(&self, peer_id: &PeerId) -> Option<String> {
        self.peer_identities.get(peer_id).map(|entry| entry.value().clone())
    }

    /// Check if a peer is a localhost peer based on its identity
    pub fn is_localhost_peer(&self, peer_id: &PeerId) -> bool {
        if let Some(identity) = self.find_identity_by_peer(peer_id) {
            // Localhost peers either have identities starting with "@@localhost." 
            // or use their peer_id as identity (for unregistered localhost nodes)
            identity.starts_with("@@localhost.") || identity == peer_id.to_string()
        } else {
            false
        }
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
        
        // Tool offerings request timer - runs every 4*PING_DURATION
        let tool_offerings_interval = Duration::from_secs(self.config.ping_interval_secs * 4);
        let mut tool_offerings_timer = tokio::time::interval(tool_offerings_interval);
        println!("🔧 Tool offerings request timer set to {} seconds", tool_offerings_interval.as_secs());
        
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
                // Periodic tool offerings requests
                _ = tool_offerings_timer.tick() => {
                    self.request_tool_offerings_from_all_peers().await;
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
                connection_id,
            })) => {
                // Check if we've recently processed an identify event for this peer (deduplication)
                let should_process = if let Some(last_event) = self.last_identify_events.get(&peer_id) {
                    last_event.elapsed() >= self.identify_event_cooldown
                } else {
                    true // First time, so process it
                };

                if !should_process {
                    println!("🔄 Skipping identify event for peer {} (cooldown: {:?} remaining)", 
                        peer_id, 
                        self.identify_event_cooldown - self.last_identify_events.get(&peer_id).unwrap().elapsed());
                    return Ok(());
                }

                // Record this identify event to prevent excessive processing
                self.last_identify_events.insert(peer_id, Instant::now());
                
                println!("🔄 Identified peer: {} with agent version: {:?} and connection id: {}", peer_id, info.agent_version, connection_id);
                
                // Check if this peer is already registered
                if let Some(existing_identity) = self.find_identity_by_peer(&peer_id) {
                    println!("🔄 Peer {} already registered with identity: {}", peer_id, existing_identity);
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
                            self.handle_identity_registration(verified_identity, peer_id, false).await;
                            
                            // Post node status update after successful identification
                            let health = self.connection_health.get(&peer_id).map(|h| h.clone());
                            if !self.is_localhost_peer(&peer_id) {
                                self.post_node_status(peer_id, true, health);
                                
                                // Request tool offerings from the newly identified non-localhost node
                                self.request_tool_offerings(peer_id).await;
                            }
                        } else {
                            let possible_identity = if info.agent_version.ends_with("shinkai") {
                                if let Some(identity_part) = info.agent_version.split("@@").nth(1) {
                                    Some(format!("@@{}", identity_part))
                                } else { None }
                            } else { None };

                            if let Some(identity) = possible_identity {
                                println!("🔑 Verification failed, registering peer {} with identity: {}, using peer id as identity.", peer_id, identity);
                                self.handle_identity_registration(identity, peer_id, true).await;
                                
                                // Note: No status update or tool offerings request for localhost peers
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
                            
                            // Post updated health status
                            let health_clone = health.clone();
                            drop(health); // Release the mutable reference
                            if !self.is_localhost_peer(&peer) {
                                self.post_node_status(peer, true, Some(health_clone));
                            }
                        }
                    }
                    ping::Event { peer, result: Err(failure), .. } => {
                        println!("📶 Ping failure to {}: {:?}", peer, failure);
                        // Update connection health with ping failure
                        if let Some(mut health) = self.connection_health.get_mut(&peer) {
                            health.record_ping_failure();
                            let health_clone = health.clone();
                            let is_unhealthy = health.is_unhealthy(self.max_ping_failures);
                            let ping_failures = health.ping_failures;
                            
                            // Release the mutable reference before async call
                            drop(health);
                            
                            // Post updated health status
                            if !self.is_localhost_peer(&peer) {
                                self.post_node_status(peer, !is_unhealthy, Some(health_clone));
                            }
                            
                            // Disconnect unhealthy connections
                            if is_unhealthy {
                                println!("❌ Disconnecting unhealthy peer {} after {} ping failures", 
                                    peer, ping_failures);
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
                        eprintln!("🔄 Relay: Received request-response message from peer {}", peer);
                        match message {
                            request_response::Message::Request { mut request, channel, .. } => {
                                println!("🔄 Relay: Received direct message request from peer {}", peer);
                                println!("   Message from: {} to: {}", 
                                    request.external_metadata.sender,
                                    request.external_metadata.recipient);
                                
                                // Check if this message is intended for the relay itself
                                // (intra_sender is empty and recipient contains relay node name)
                                if request.external_metadata.intra_sender.is_empty() && 
                                   request.external_metadata.recipient.contains(&self.config.relay_node_name) {
                                    println!("📩 Relay: Message is intended for the relay itself, processing locally");
                                    
                                    // Decrypt and process the message intended for the relay
                                    if let Some(response) = self.process_relay_message(request, peer).await {
                                        let _ = self.swarm.behaviour_mut().request_response.send_response(channel, response);
                                    } else {
                                        println!("❌ Relay: Failed to process message intended for relay");
                                        // Send an error response or handle failure appropriately
                                    }
                                    return Ok(());
                                }
                                
                                // Try to find the target peer by their identity
                                let target_identity = &request.external_metadata.recipient;
                                let target_node = if let Ok(parsed_name) = shinkai_message_primitives::schemas::shinkai_name::ShinkaiName::new(target_identity.clone()) {
                                    parsed_name.get_node_name_string()
                                } else {
                                    target_identity.clone()
                                };

                                if request.external_metadata.recipient.contains(self.config.relay_node_name.as_str()) && 
                                   !request.external_metadata.intra_sender.is_empty() {
                                    println!("🔑 Relay: We need to re-encrypt the message from relay to {}", request.external_metadata.intra_sender);
                                    let peer_id = request.external_metadata.intra_sender.parse::<PeerId>().unwrap();
                                    request.external_metadata.intra_sender = request.external_metadata.sender.clone();
                                    request.external_metadata.sender = self.config.relay_node_name.clone();
                                    request.external_metadata.recipient = "@@localhost.sep-shinkai".to_string();

                                    // Re-encrypt message for localhost recipient
                                    self.relay_message_encryption(&mut request, &"@@localhost.sep-shinkai".to_string()).await;

                                    // Re-sign outer layer with the relay identity key
                                    if let Ok(resigned) = request.sign_outer_layer(&self.config.identity_secret_key) {
                                        request = resigned;
                                    } else {
                                        println!("❌ Failed to re-sign message from localhost");
                                    }
                                
                                    let outbound_id = self.swarm.behaviour_mut().request_response.send_request(&peer_id, request);
                                    self.request_response_channels.insert(outbound_id, channel);
                                    return Ok(());
                                }
                                
                                if request.external_metadata.sender.starts_with("@@localhost.") {
                                    println!("🔑 Relay: We need to re-sign the outer layer of the message from localhost to {}", target_node);

                                    // Tell the recipient that the message was relayed by the relay node
                                    request.external_metadata.sender = self.config.relay_node_name.clone();
                                    request.external_metadata.intra_sender = peer.to_string();

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
                                    // request.external_metadata.other = "Target not found".to_string();
                                    // let _ = self.swarm.behaviour_mut().request_response.send_response(channel, request);
                                }
                            }
                            request_response::Message::Response { mut response, request_id, .. } => {
                                println!("🔄 Relay: Received direct message response from peer {}", peer);
                                println!("   Message from: {} to: {}", 
                                    response.external_metadata.sender,
                                    response.external_metadata.recipient);

                                // Check if this response is intended for the relay itself
                                // (intra_sender is empty and recipient contains relay node name)
                                if response.external_metadata.intra_sender.is_empty() && 
                                   response.external_metadata.recipient.contains(&self.config.relay_node_name) {
                                    println!("📩 Relay: Response is intended for the relay itself, processing locally");
                                    
                                    // Process the response intended for the relay - no need to forward it
                                    self.process_relay_response(response, peer).await;
                                    return Ok(());
                                }

                                // Check if this is an AgentNetworkOfferingResponse
                                if let Ok(content) = response.get_message_content() {
                                    if let Ok(schema) = response.get_message_content_schema() {
                                        if schema == MessageSchemaType::AgentNetworkOfferingResponse {
                                            self.handle_tool_offerings_response(peer, content).await;
                                            
                                            // Don't relay tool offerings responses to other nodes
                                            return Ok(());
                                        }
                                    }
                                }                 

                                if response.external_metadata.sender.starts_with("@@localhost.") {
                                    println!("🔑 Relay: We need to re-sign the outer layer of the message from localhost to {}", response.external_metadata.recipient);

                                    // Tell the recipient that the message was relayed by the relay node
                                    response.external_metadata.sender = self.config.relay_node_name.clone();
                                    response.external_metadata.intra_sender = peer.to_string();

                                    // Re-sign outer layer with the relay identity key
                                    if let Ok(resigned) = response.sign_outer_layer(&self.config.identity_secret_key) {
                                        response = resigned;
                                    } else {
                                        println!("❌ Failed to re-sign message from localhost");
                                    }
                                }

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
                let health = ConnectionHealth::new();
                self.connection_health.insert(peer_id, health.clone());
                
                // Post initial connection status
                self.post_node_status(peer_id, true, Some(health));
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, num_established, .. } => {
                println!(
                    "❌ Disconnected from peer: {}, reason: {:?}, remaining connections: {}",
                    peer_id, cause, num_established
                );
                if num_established == 0 {
                    // Get final health status before cleanup
                    let final_health = self.connection_health.get(&peer_id).map(|h| h.clone());
                    
                    // Post offline status
                    self.post_node_status(peer_id, false, final_health);
                    
                    self.unregister_peer(&peer_id);
                    // Clean up connection health tracking
                    self.connection_health.remove(&peer_id);
                    // Clean up identify event tracking (redundant but safe)
                    self.last_identify_events.remove(&peer_id);
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
                    peer_id, health.last_activity.elapsed().unwrap_or(Duration::ZERO));
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
        
        // Also clean up stale identify event tracking for disconnected peers
        let mut stale_identify_peer_ids = Vec::new();
        for entry in self.last_identify_events.iter() {
            let peer_id = *entry.key();
            // Check if peer is still connected
            if !self.swarm.is_connected(&peer_id) {
                stale_identify_peer_ids.push(peer_id);
            }
        }
        
        for peer_id in stale_identify_peer_ids {
            self.last_identify_events.remove(&peer_id);
            println!("🧹 Removed stale identify event tracking for disconnected peer: {}", peer_id);
        }
    }

    /// Process a message intended for the relay itself
    async fn process_relay_message(&mut self, request: ShinkaiMessage, sender_peer: PeerId) -> Option<ShinkaiMessage> {
        println!("🔓 Relay: Processing message intended for relay from peer {}", sender_peer);
        
        // Get sender's identity to get their encryption key
        let sender_identity = if let Some(identity) = self.find_identity_by_peer(&sender_peer) {
            identity
        } else {
            println!("❌ Relay: Could not find sender identity for peer {}", sender_peer);
            return None;
        };

        // Get sender's encryption key from registry or use other field for localhost
        let sender_enc_key = if sender_identity.contains("localhost") {
            println!("🔑 Relay: Using other field for localhost sender");
            match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&request.external_metadata.other) {
                Ok(key) => key,
                Err(e) => {
                    println!("❌ Relay: Failed to parse sender's encryption key from other field: {}", e);
                    return None;
                }
            }
        } else {
            // For registered nodes, get from blockchain registry
            match self.registry.get_identity_record(sender_identity.clone(), None).await {
                Ok(identity_record) => {
                    match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&identity_record.encryption_key) {
                        Ok(key) => key,
                        Err(e) => {
                            println!("❌ Relay: Failed to parse sender's encryption key: {}", e);
                            return None;
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Relay: Failed to get sender's identity from registry: {}", e);
                    return None;
                }
            }
        };

        // Decrypt the message using relay's private key and sender's public key
        let decrypted_message = match request.decrypt_outer_layer(&self.config.encryption_secret_key, &sender_enc_key) {
            Ok(message) => message,
            Err(e) => {
                println!("❌ Relay: Failed to decrypt message intended for relay: {}", e);
                return None;
            }
        };

        // Try to decrypt inner layer as well if possible
        let fully_decrypted = match decrypted_message.decrypt_inner_layer(&self.config.encryption_secret_key, &sender_enc_key) {
            Ok(inner_message) => inner_message,
            Err(_) => {
                println!("⚠️  Relay: Could not decrypt inner layer, using outer layer only");
                decrypted_message
            }
        };

        // Check if this is an AgentNetworkOfferingResponse
        let message_schema = match &fully_decrypted.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => &data.message_content_schema,
                _ => {
                    println!("⚠️  Relay: Message data is encrypted, cannot determine schema");
                    return None;
                }
            },
            _ => {
                println!("⚠️  Relay: Message body is encrypted, cannot determine schema");
                return None;
            }
        };

        if *message_schema == MessageSchemaType::AgentNetworkOfferingResponse {
            println!("🔧 Relay: Received AgentNetworkOfferingResponse, processing tool offerings");
            
            // Get the message content
            let content = match &fully_decrypted.body {
                MessageBody::Unencrypted(body) => match &body.message_data {
                    MessageData::Unencrypted(data) => data.message_raw_content.clone(),
                    _ => {
                        println!("❌ Relay: Cannot extract content from encrypted message data");
                        return self.create_ack_response(&request, &sender_enc_key, &fully_decrypted);
                    }
                },
                _ => {
                    println!("❌ Relay: Cannot extract content from encrypted message body");
                    return self.create_ack_response(&request, &sender_enc_key, &fully_decrypted);
                }
            };
            
            self.handle_tool_offerings_response(sender_peer, content).await;
            
            // Send back an ACK after processing the offerings
            return self.create_ack_response(&request, &sender_enc_key, &fully_decrypted);
        }

        // For other message types, create a simple ACK response
        self.create_ack_response(&request, &sender_enc_key, &fully_decrypted)
    }

    /// Create an ACK response for a processed message
    fn create_ack_response(
        &self,
        original_request: &ShinkaiMessage,
        sender_encryption_key: &x25519_dalek::PublicKey,
        _processed_message: &ShinkaiMessage,
    ) -> Option<ShinkaiMessage> {
        // Create a simple ACK message using the ShinkaiMessageBuilder
        match ShinkaiMessageBuilder::ack_message(
            self.config.encryption_secret_key.clone(),
            self.config.identity_secret_key.clone(),
            *sender_encryption_key,
            self.config.relay_node_name.clone(),
            original_request.external_metadata.sender.clone(),
        ) {
            Ok(ack_message) => {
                println!("✅ Relay: Successfully created ACK response");
                Some(ack_message)
            }
            Err(e) => {
                println!("❌ Relay: Failed to create ACK response: {}", e);
                None
            }
        }
    }

    /// Process a response message intended for the relay itself
    async fn process_relay_response(&mut self, response: ShinkaiMessage, sender_peer: PeerId) {
        println!("🔓 Relay: Processing response intended for relay from peer {}", sender_peer);
        
        // Get sender's identity to get their encryption key for decryption (if needed)
        let sender_identity = if let Some(identity) = self.find_identity_by_peer(&sender_peer) {
            identity
        } else {
            println!("❌ Relay: Could not find sender identity for peer {}", sender_peer);
            return;
        };

        // Check if the response is encrypted and needs decryption
        let decrypted_response = if response.encryption != shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod::None {
            // Get sender's encryption key for decryption
            let sender_enc_key = if sender_identity.contains("localhost") {
                println!("🔑 Relay: Using other field for localhost sender");
                match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&response.external_metadata.other) {
                    Ok(key) => key,
                    Err(e) => {
                        println!("❌ Relay: Failed to parse sender's encryption key from other field: {}", e);
                        return;
                    }
                }
            } else {
                // For registered nodes, get from blockchain registry
                match self.registry.get_identity_record(sender_identity.clone(), None).await {
                    Ok(identity_record) => {
                        match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&identity_record.encryption_key) {
                            Ok(key) => key,
                            Err(e) => {
                                println!("❌ Relay: Failed to parse sender's encryption key: {}", e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ Relay: Failed to get sender's identity from registry: {}", e);
                        return;
                    }
                }
            };

            // Decrypt the response using relay's private key and sender's public key
            match response.decrypt_outer_layer(&self.config.encryption_secret_key, &sender_enc_key) {
                Ok(message) => message,
                Err(e) => {
                    println!("❌ Relay: Failed to decrypt response intended for relay: {}", e);
                    return;
                }
            }
        } else {
            response
        };

        // Check the message content and schema to determine how to process it
        let message_content = match decrypted_response.get_message_content() {
            Ok(content) => content,
            Err(_) => {
                println!("⚠️  Relay: Could not extract message content from response");
                return;
            }
        };

        let message_schema = match decrypted_response.get_message_content_schema() {
            Ok(schema) => schema,
            Err(_) => {
                println!("⚠️  Relay: Could not extract message schema from response");
                return;
            }
        };

        // Handle different types of responses
        match message_schema {
            MessageSchemaType::AgentNetworkOfferingResponse => {
                println!("🔧 Relay: Received AgentNetworkOfferingResponse in response, processing tool offerings");
                self.handle_tool_offerings_response(sender_peer, message_content).await;
            }
            MessageSchemaType::TextContent => {
                if message_content == "ACK" {
                    println!("✅ Relay: Received ACK response from peer {}", sender_peer);
                } else {
                    // TODO: Handle other text responses as needed in the future.
                }
            }
            _ => {
                // TODO: Handle other response types as needed in the future.
            }
        }
        
        println!("✅ Relay: Finished processing response from peer {}", sender_peer);
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

        // For localhost nodes (unregistered), use deterministic key generation
        let recipient_enc_key = if target_node.contains("localhost") {
            println!("🔑 Relay: Using other field for localhost node");
            // Parse recipient's encryption key
            match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&request.external_metadata.other) {
                Ok(key) => key,
                Err(_) => {
                    println!("❌ Relay: Failed to parse recipient's encryption key");
                    return;
                }
            }
        } else {
            // For registered nodes, try to get from blockchain registry
            let recipient_node_name = recipient_name.get_node_name_string();
            let recipient_identity = match self.registry.get_identity_record(recipient_node_name.clone(), None).await {
                Ok(identity) => identity,
                Err(e) => {
                    println!("❌ Relay: Failed to get recipient's identity from registry: {}", e);
                    return;
                }
            };
    
            // Parse recipient's encryption key
            match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&recipient_identity.encryption_key) {
                Ok(key) => key,
                Err(_) => {
                    println!("❌ Relay: Failed to parse recipient's encryption key");
                    return;
                }
            }
        };

        let original_sender_node_name = request.external_metadata.intra_sender.clone();
        let original_sender_identity = match self.registry.get_identity_record(original_sender_node_name.clone(), None).await {
            Ok(identity) => identity,
            Err(e) => {
                println!("❌ Relay: Failed to get recipient's identity from registry: {}", e);
                return;
            }
        };

        // Parse original sender's encryption key
        let original_sender_enc_key = match shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_public_key(&original_sender_identity.encryption_key) {
            Ok(key) => key,
            Err(_) => {
                println!("❌ Relay: Failed to parse original sender's encryption key");
                return;
            }
        };        

        // Decrypt the message using relay's private key and original sender's public key
        let mut decrypted_message = match request.decrypt_outer_layer(&self.config.encryption_secret_key, &original_sender_enc_key) {
            Ok(message) => {
                println!("✅ Relay: Successfully decrypted outer layer.");
                message
            }
            Err(e) => {
                println!("❌ Relay: Failed to decrypt outer layer: {}", e);
                return;
            }
        };

        // Also decrypt and re-encrypt the inner layer for end-to-end encryption between profiles
        println!("🔑 Relay: Re-encrypting inner layer for final recipient");
        if let Ok(inner_decrypted) = decrypted_message.decrypt_inner_layer(&self.config.encryption_secret_key, &original_sender_enc_key) {
            println!("✅ Relay: Successfully decrypted inner layer.");
            // Re-encrypt inner layer with relay's key + recipient's profile key  
            if let Ok(inner_re_encrypted) = inner_decrypted.encrypt_inner_layer(&self.config.encryption_secret_key, &recipient_enc_key) {
                println!("✅ Relay: Successfully re-encrypted inner layer");
                decrypted_message = inner_re_encrypted;
            } else {
                println!("❌ Relay: Failed to re-encrypt inner layer");
            }
        } else {
            println!("⚠️  Relay: Could not decrypt inner layer, proceeding with outer layer only");
        }

        // Re-encrypt outer layer with recipient's key
        match decrypted_message.encrypt_outer_layer(&self.config.encryption_secret_key, &recipient_enc_key) {
            Ok(mut re_encrypted_message) => {
                println!("✅ Relay: Successfully decrypted and re-encrypted message for recipient");
                
                // Update the 'other' field to contain relay's public key for final decryption
                let relay_public_key = x25519_dalek::PublicKey::from(&self.config.encryption_secret_key);
                re_encrypted_message.external_metadata.other = encryption_public_key_to_string(relay_public_key);

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