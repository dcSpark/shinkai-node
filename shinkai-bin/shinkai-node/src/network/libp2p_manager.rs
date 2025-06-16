use ed25519_dalek::SigningKey;
use futures::prelude::*;
use libp2p::{
    dcutr, identify, noise, ping, relay, request_response::{self, ResponseChannel},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm,
    multiaddr::Protocol,
};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use std::{
    sync::Arc,
    time::Duration,
    collections::{HashMap, VecDeque},
};
use tokio::sync::mpsc;

/// The libp2p network behavior combining all protocols
/// Includes relay client support for connecting through relay servers
#[derive(NetworkBehaviour)]
pub struct ShinkaiNetworkBehaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub relay_client: relay::client::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub request_response: request_response::json::Behaviour<ShinkaiMessage, ShinkaiMessage>,
}

/// Events that can be sent through the network
#[derive(Debug)]
pub enum NetworkEvent {
    /// A message to be sent directly to a specific peer using request-response
    SendDirectMessage {
        peer_id: PeerId,
        message: ShinkaiMessage,
    },
    /// A response to be sent to a specific peer using request-response
    SendResponse {
        channel: ResponseChannel<ShinkaiMessage>,
        message: ShinkaiMessage,
    },
    /// Add a peer to connect to
    #[allow(dead_code)]
    AddPeer {
        peer_id: PeerId,
        address: Multiaddr,
    },    
    /// Ping a specific peer using libp2p ping
    PingPeer {
        peer_id: PeerId,
    },
    /// Attempt to upgrade a relayed connection to direct using DCUtR
    TryDirectConnectionUpgrade {
        peer_id: PeerId,
    },
    /// Request peer discovery from relay
    DiscoverPeers,
    /// Connect to a specific discovered peer by identity
    ConnectToDiscoveredPeer {
        identity: String,
    },
}

/// A queued message waiting to be sent
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub peer_id: PeerId,
    pub message: ShinkaiMessage,
    pub retry_count: u32,
    pub last_attempt: std::time::Instant,
}

/// The main libp2p network manager
pub struct LibP2PManager {
    swarm: Swarm<ShinkaiNetworkBehaviour>,
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
    event_receiver: mpsc::UnboundedReceiver<NetworkEvent>,
    message_handler: Arc<ShinkaiMessageHandler>,
    relay_address: Option<Multiaddr>, // Store relay address for circuit listening
    // Reconnection mechanism fields
    relay_peer_id: Option<PeerId>, // Track the relay peer ID for reconnection
    is_connected_to_relay: bool, // Track relay connection state
    reconnection_attempts: u32, // Count reconnection attempts for backoff
    last_disconnection_time: Option<std::time::Instant>, // Track when we disconnected
    // Peer discovery fields
    discovered_peers: HashMap<String, (PeerId, Multiaddr)>, // identity -> (peer_id, circuit_addr)
    // Message queue fields
    message_queue: VecDeque<QueuedMessage>, // Queue for messages that failed to send
    max_retry_attempts: u32, // Maximum retry attempts per message
    pending_outbound_requests: HashMap<request_response::OutboundRequestId, QueuedMessage>,
}

use crate::network::network_manager::libp2p_message_handler::ShinkaiMessageHandler;

impl LibP2PManager {
    /// Create a new libp2p manager
    pub async fn new(
        node_name: String,
        identity_secret_key: SigningKey,
        listen_port: Option<u16>,
        message_handler: ShinkaiMessageHandler,
        relay_address: Option<Multiaddr>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let local_key = libp2p::identity::Keypair::ed25519_from_bytes(identity_secret_key.to_bytes())?;
        let local_peer_id = PeerId::from(local_key.public());

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("LIBP2P Local peer id: {}", local_peer_id),
        );

        // Create swarm
        let mut swarm =
        libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_relay_client(noise::Config::new, yamux::Config::default)?
            .with_behaviour(|keypair, relay_behaviour| ShinkaiNetworkBehaviour {
                relay_client: relay_behaviour,
                ping: ping::Behaviour::new(
                    ping::Config::new()
                        .with_interval(Duration::from_secs(5))  // Reduced from 10s to 5s
                        .with_timeout(Duration::from_secs(10))  // Add explicit timeout
                ),
                identify: identify::Behaviour::new(identify::Config::new(
                    "/shinkai/1.0.0".to_string(),
                    keypair.public(),
                ).with_agent_version(format!("shinkai-node-{}", node_name))
                .with_interval(Duration::from_secs(60))
                .with_push_listen_addr_updates(true)
                .with_cache_size(100)),
                dcutr: dcutr::Behaviour::new(keypair.public().to_peer_id()),
                request_response: request_response::json::Behaviour::new(
                    std::iter::once((libp2p::StreamProtocol::new("/shinkai/message/1.0.0"), request_response::ProtocolSupport::Full)),
                    request_response::Config::default()
                        .with_request_timeout(Duration::from_secs(30))
                ),
            })?
            .build();

        // Listen on TCP
        let tcp_listen_addr= if let Some(port) = listen_port {
            format!("/ip4/0.0.0.0/tcp/{}", port)
        } else {
            "/ip4/0.0.0.0/tcp/0".to_string()
        };
        swarm.listen_on(tcp_listen_addr.parse()?)?;
        
        if relay_address.is_some() {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Listening on {} (relay mode - for relay connections)", tcp_listen_addr),
            );
        } else {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Listening on {} (direct mode)", tcp_listen_addr),
            );
        }

        // Connect to relay if provided
        if let Some(ref relay_addr) = relay_address {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Connecting to relay at: {}", relay_addr),
            );
            swarm.dial(relay_addr.clone())?;
        }

        // Create event channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Ok(LibP2PManager {
            swarm,
            event_sender,
            event_receiver,
            message_handler: Arc::new(message_handler),
            relay_address: relay_address.clone(),
            // Reconnection mechanism fields
            relay_peer_id: None, // Track the relay peer ID for reconnection
            is_connected_to_relay: false, // Track relay connection state
            reconnection_attempts: 0, // Count reconnection attempts for backoff
            last_disconnection_time: None, // Track when we disconnected
            // Peer discovery fields
            discovered_peers: HashMap::new(), // identity -> (peer_id, circuit_addr)
            // Message queue fields
            message_queue: VecDeque::new(), // Queue for messages that failed to send
            max_retry_attempts: 5, // Maximum retry attempts per message
            pending_outbound_requests: HashMap::new(),
        })
    }

    /// Get the event sender for sending network events
    pub fn event_sender(&self) -> mpsc::UnboundedSender<NetworkEvent> {
        self.event_sender.clone()
    }

    /// Get all connected peers
    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.swarm.connected_peers().cloned().collect()
    }

    /// Send a direct message to a specific peer using request-response
    pub async fn send_direct_message_to_peer(
        &mut self,
        peer_id: PeerId,
        message: ShinkaiMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check if the peer is already connected
        let is_connected = self.swarm.is_connected(&peer_id);
        
        if !is_connected {
            eprintln!("Peer {} not connected, queueing message for retry", peer_id);
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Peer {} not connected, queueing message for retry", peer_id),
            );
            
            // Queue the message for retry
            let queued_message = QueuedMessage {
                peer_id,
                message,
                retry_count: 0,
                last_attempt: std::time::Instant::now(),
            };
            self.message_queue.push_back(queued_message);
            
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("Message queued for peer {}, queue size: {}", peer_id, self.message_queue.len()),
            );

            if let Err(e) = self.swarm.dial(peer_id.clone()) {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to dial peer {}: {}", peer_id, e),
                );
            }
            
            return Ok(());
        }

        // Send the message using request-response protocol
        let queued_message = QueuedMessage {
            peer_id,
            message: message.clone(),
            retry_count: 0,
            last_attempt: std::time::Instant::now(),
        };

        let _request_id = self.swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer_id, message);

        self.pending_outbound_requests.insert(_request_id, queued_message);

        eprintln!("Direct message request sent to peer {} {:?}", peer_id, _request_id);
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("Direct message request sent to peer {}", peer_id),
        );

        Ok(())
    }

    pub async fn send_response_to_peer(
        &mut self,
        channel: ResponseChannel<ShinkaiMessage>,
        message: ShinkaiMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.swarm.behaviour_mut().request_response.send_response(channel, message.clone())
            .map_err(|_| Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to send response")) as Box<dyn std::error::Error>)?;
        eprintln!("Response sent to identity {:?} from identity {:?}", message.external_metadata.recipient, message.external_metadata.sender);
        Ok(())
    }

    /// Add a peer to connect to
    pub fn add_peer(&mut self, peer_id: PeerId, address: Multiaddr) -> Result<(), Box<dyn std::error::Error>> {
        // For direct networking without relay, attempt to connect to the peer
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Attempting to connect to peer {} at {}", peer_id, address),
        );
        
        // Dial the peer directly
        if let Err(e) = self.swarm.dial(address.clone()) {
            eprintln!("Failed to dial peer {} at {}: {}", peer_id, address, e);
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!("Failed to dial peer {} at {}: {}", peer_id, address, e),
            );
            return Err(Box::new(e));
        }
        
        Ok(())
    }

    /// Run the network manager
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut reconnection_timer = tokio::time::interval(Duration::from_secs(3)); // Check reconnection every 3 seconds
        let mut message_retry_timer = tokio::time::interval(Duration::from_secs(1)); // Process message queue every 1 second
        
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await?;
                }
                network_event = self.event_receiver.recv() => {
                    if let Some(event) = network_event {
                        self.handle_network_event(event).await?;
                    }
                }
                _ = reconnection_timer.tick() => {
                    self.check_and_reconnect_to_relay().await?;
                }
                _ = message_retry_timer.tick() => {
                    self.process_message_queue().await?;
                }
            }
        }
    }

    /// Handle individual swarm events
    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<ShinkaiNetworkBehaviourEvent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                                        &format!("Listening on {}", address),
                );
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::RelayClient(relay_event)) => {
                // Handle relay client events for maintaining relay connections
                use libp2p::relay::client::Event as RelayClientEvent;
                match relay_event {
                    RelayClientEvent::ReservationReqAccepted { 
                        relay_peer_id, 
                        renewal, 
                        limit 
                    } => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("🎉 Relay reservation ACCEPTED by {} (renewal: {}, limit: {:?})", 
                                relay_peer_id, renewal, limit),
                        );
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            "   ✅ Relay connection established successfully - can now receive connections through relay",
                        );
                        
                        // Now we have a reservation, create and advertise our circuit address
                        if let Some(circuit_addr) = Self::create_circuit_address_for_relay(self, &relay_peer_id) {
                            self.swarm.add_external_address(circuit_addr.clone());
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Info,
                                &format!("   📍 Added relay circuit address for discovery: {}", circuit_addr),
                            );
                        }
                    }
                    RelayClientEvent::OutboundCircuitEstablished { 
                        relay_peer_id, 
                        limit 
                    } => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("🔄 Outbound circuit established through {} (limit: {:?})", 
                                relay_peer_id, limit),
                        );
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            "   ✅ Can now connect to other peers through this relay",
                        );
                    }
                    RelayClientEvent::InboundCircuitEstablished { 
                        src_peer_id, 
                        limit 
                    } => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("🔄 Inbound circuit established from {} (limit: {:?})", 
                                src_peer_id, limit),
                        );
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            "   ✅ Peer connected to us through relay circuit",
                        );
                    }
                }
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Dcutr(dcutr_event)) => {
                // Handle DCUtR events for direct connection upgrades
                // Enhanced DCUtR event handling for direct connection upgrades
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("🔄 DCUtR: Direct connection upgrade event: {:?}", dcutr_event),
                );
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Ping(ping_event)) => {
                let ping::Event { peer, connection: _, result } = ping_event;
                match result {
                    Ok(rtt) => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("Successfully pinged peer {} in {:?}", peer, rtt),
                        );
                    }
                    Err(ping::Failure::Timeout) => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            &format!("Ping timeout to peer {}", peer),
                        );
                    }
                    Err(ping::Failure::Unsupported) => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            &format!("Ping unsupported by peer {}", peer),
                        );
                    }
                    Err(ping::Failure::Other { error }) => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            &format!("Ping error to peer {}: {}", peer, error),
                        );
                    }
                }
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Identify(identify::Event::Received { peer_id, info })) => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Identified peer {} with protocol version {}", peer_id, info.protocol_version),
                );
                
                // Check if this peer supports the relay protocol
                let supports_relay = info.protocols.iter().any(|protocol| {
                    protocol.to_string().contains("/libp2p/circuit/relay/")
                });
                
                if supports_relay {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Peer {} supports relay protocol - ready to use as relay", peer_id),
                    );
                    
                    // CRITICAL: Now that we've identified the relay, listen on the relay circuit
                    // This is what actually establishes the relay reservation
                    if let Some(relay_addr) = self.relay_address.clone() {
                        if let Some(relay_peer_from_addr) = Self::extract_peer_id_from_address(&relay_addr) {
                            if relay_peer_from_addr == peer_id {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Info,
                                    "🔄 This is our configured relay - establishing circuit reservation",
                                );
                                
                                // Mark as connected if not already marked
                                if !self.is_connected_to_relay {
                                    self.mark_relay_connected(peer_id);
                                }
                                
                                // Create the circuit address with the correct format:
                                // /ip4/{relay-ip}/tcp/{relay-port}/p2p/{relay-peer-id}/p2p-circuit
                                let circuit_addr = relay_addr.clone()
                                    .with(libp2p::multiaddr::Protocol::P2p(peer_id))
                                    .with(libp2p::multiaddr::Protocol::P2pCircuit);
                                
                                if let Err(e) = self.swarm.listen_on(circuit_addr.clone()) {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!("Failed to listen on relay circuit {}: {}", circuit_addr, e),
                                    );
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Info,
                                        &format!("📡 Successfully requested relay reservation: {}", circuit_addr),
                                    );
                                }
                            }
                        } else {
                            // If we can't extract peer ID from address, assume this might be our relay
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Info,
                                "🔄 Identified relay server - attempting to establish circuit reservation",
                            );
                            
                            // Mark as connected (this confirms our potential relay connection)
                            if !self.is_connected_to_relay {
                                self.mark_relay_connected(peer_id);
                            }
                            
                            // Create circuit address with the identified peer ID
                            let circuit_addr = relay_addr.clone()
                                .with(libp2p::multiaddr::Protocol::P2p(peer_id))
                                .with(libp2p::multiaddr::Protocol::P2pCircuit);
                            
                            if let Err(e) = self.swarm.listen_on(circuit_addr.clone()) {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to listen on relay circuit {}: {}", circuit_addr, e),
                                );
                            } else {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Info,
                                    &format!("📡 Successfully requested relay reservation: {}", circuit_addr),
                                );
                            }
                        }
                    }
                } else {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Peer {} identified as direct connection peer (no relay support)", peer_id),
                    );
                    
                    // For direct connections, ensure the connection is properly established
                    eprintln!("Direct peer {} identified, connection should now be stable", peer_id);
                    
                    // Check if this peer supports our Shinkai message protocol
                    let supports_shinkai = info.protocols.iter().any(|protocol| {
                        protocol.to_string().contains("/shinkai/message/")
                    });
                    
                    if supports_shinkai {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("✅ Peer {} supports Shinkai message protocol - ready for communication", peer_id),
                        );
                        eprintln!("✅ Peer {} supports Shinkai protocol and is ready for messages", peer_id);
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("⚠️  Peer {} does not support Shinkai message protocol", peer_id),
                        );
                    }
                }
                
                // Add all listen addresses from the peer to the swarm
                for addr in &info.listen_addrs {
                    self.swarm.add_peer_address(peer_id, addr.clone());
                }
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::RequestResponse(req_resp_event)) => {
                match req_resp_event {
                    request_response::Event::Message { peer, message } => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Info,
                                    &format!("Received direct message request from peer {}", peer),
                                );
                                eprintln!("Received direct message request from peer {} {:?}", peer, request);

                                // Handle the incoming request message
                                let _ = self.message_handler.handle_message_internode(peer, &request, Some(channel), Some(self.event_sender.clone())).await;
                            }
                            request_response::Message::Response { response, request_id, .. } => {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Info,
                                    &format!("Received direct message response from peer {}", peer),
                                );
                                eprintln!("Received direct message response from peer {} {:?}", peer, response);
                                // Handle the response (acknowledgment)
                                let _ = self.message_handler.handle_message_internode(peer, &response, None, Some(self.event_sender.clone())).await;
                                self.pending_outbound_requests.remove(&request_id);
                            }
                        }
                    }
                    request_response::Event::OutboundFailure { peer, request_id, error, .. } => {
                        match error {
                            request_response::OutboundFailure::DialFailure => {
                                eprintln!("Dial failure to peer {} - message will be queued for retry", peer);
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Info,
                                    &format!("Dial failure to peer {} - will retry when connection is established", peer),
                                );

                                if let Some(mut queued_message) = self.pending_outbound_requests.remove(&request_id) {
                                    if !self.message_queue.iter().any(|m| m.peer_id == queued_message.peer_id && m.message == queued_message.message) {
                                        queued_message.last_attempt = std::time::Instant::now();
                                        self.message_queue.push_back(queued_message);
                                    }
                                }
                            }
                            request_response::OutboundFailure::ConnectionClosed => {
                                eprintln!("Connection closed to peer {} for request {:?}.", peer, request_id);
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Debug,
                                    &format!("Connection closed to peer {} for request {:?}.", peer, request_id),
                                );
                            }
                            _ => {
                                eprintln!("Failed to send direct message {:?} to peer {}: {:?}", request_id, peer, error);
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to send direct message to peer {}: {:?}", peer, error),
                                );
                                self.pending_outbound_requests.remove(&request_id);
                            }
                        }
                    }
                    request_response::Event::InboundFailure { peer, error, .. } => {
                        eprintln!("Failed to receive direct message from peer {}: {:?}", peer, error);
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to receive direct message from peer {}: {:?}", peer, error),
                        );
                    }
                    request_response::Event::ResponseSent { peer, .. } => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Debug,
                            &format!("Successfully sent response to peer {}", peer),
                        );
                        eprintln!("Successfully sent response to peer {}", peer);
                    }
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("✅ Connection established with {} at {:?}", peer_id, endpoint),
                );

                eprintln!("Connection established with {} at {:?}", peer_id, endpoint);
                
                // Check if this is a direct connection to a relay server (not through a circuit)
                let is_direct_to_relay = Self::is_external_address(&endpoint.get_remote_address()) 
                    && !Self::is_circuit_address(&endpoint.get_remote_address());
                
                if is_direct_to_relay {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Connected directly to potential relay server: {}", peer_id),
                    );

                    // Check if this might be our configured relay
                    if let Some(relay_addr) = self.relay_address.clone() {
                        if let Some(relay_peer_from_addr) = Self::extract_peer_id_from_address(&relay_addr) {
                            if relay_peer_from_addr == peer_id {
                                // This is our configured relay - mark as connected
                                self.mark_relay_connected(peer_id);
                            }
                        } else {
                            // If we can't extract peer ID from address, assume this might be our relay
                            // We'll confirm this during the identify protocol
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Info,
                                "Potential relay connection - will confirm during identification",
                            );
                        }
                    }
                }
                
                // Check if this connection is through a relay and create circuit address
                if let Some(circuit_addr) = Self::extract_circuit_address(&endpoint.get_remote_address(), &peer_id) {
                    // Add the circuit address as an external address so other peers know how to reach us
                    self.swarm.add_external_address(circuit_addr.clone());
                    
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Added relay circuit address for peer discovery: {}", circuit_addr),
                    );
                }
                
                // When a new peer connects through relay, attempt direct connection upgrade
                if Self::is_circuit_address(&endpoint.get_remote_address()) {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Peer {} connected via relay - will attempt DCUtR upgrade after identification", peer_id),
                    );
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Disconnected from peer {}: {:?}", peer_id, cause),
                );
                
                // Check if this was our relay connection and trigger reconnection
                self.mark_relay_disconnected(peer_id);
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                eprintln!("Incoming connection error: {}", error);
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Debug,
                    &format!("Incoming connection error: {}", error),
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                eprintln!("Outgoing connection error to {:?}: {}", peer_id, error);
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Debug,
                    &format!("Outgoing connection error to {:?}: {}", peer_id, error),
                );
            }
            _ => {}
        }
        Ok(())
    }

    /// Check if a multiaddr represents an external/public address
    /// Returns false for localhost, private networks, and other non-routable addresses
    fn is_external_address(addr: &Multiaddr) -> bool {
        use libp2p::multiaddr::Protocol;
        
        for protocol in addr.iter() {
            match protocol {
                Protocol::Ip4(ip) => {
                    // Filter out private/local IP ranges
                    if ip.is_loopback() ||        // 127.0.0.0/8
                       ip.is_private() ||         // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                       ip.is_link_local() ||      // 169.254.0.0/16
                       ip.is_documentation() ||   // Documentation IPs
                       ip.is_multicast() ||       // Multicast
                       ip.is_broadcast() ||       // Broadcast
                       ip.is_unspecified() {      // 0.0.0.0
                        return false;
                    }
                    
                    // Additional private ranges not covered by is_private()
                    let octets = ip.octets();
                    match octets[0] {
                        // Docker default bridge: 172.17.0.0/16
                        172 if octets[1] == 17 => return false,
                        // Additional private ranges
                        100 if octets[1] >= 64 && octets[1] <= 127 => return false, // 100.64.0.0/10 (CGN)
                        _ => {}
                    }
                }
                Protocol::Ip6(ip) => {
                    // Filter out IPv6 private/local ranges
                    if ip.is_loopback() ||        // ::1
                       ip.is_multicast() ||       // ff00::/8
                       ip.is_unspecified() {      // ::
                        return false;
                    }
                    
                    // IPv6 link-local: fe80::/10
                    if ip.segments()[0] & 0xffc0 == 0xfe80 {
                        return false;
                    }
                    
                    // IPv6 unique local: fc00::/7 (fd00::/8)
                    if ip.segments()[0] & 0xfe00 == 0xfc00 {
                        return false;
                    }
                }
                _ => continue,
            }
        }
        
        true
    }

    /// Extract a relay circuit address from a connection to a relay peer
    /// Returns the circuit address that other peers can use to reach this node through the relay
    fn extract_circuit_address(remote_addr: &Multiaddr, relay_peer_id: &PeerId) -> Option<Multiaddr> {
        use libp2p::multiaddr::Protocol;
        
        // Check if this looks like a connection to a relay server
        // We identify relay servers by checking if the remote address is external/public
        if !Self::is_external_address(remote_addr) {
            return None;
        }
        
        // Build the circuit address that other peers can use to reach us through this relay
        // Format: /ip4/{relay-ip}/tcp/{relay-port}/p2p/{relay-peer-id}/p2p-circuit
        let mut circuit_addr = Multiaddr::empty();
        
        // Extract the network portion (IP and port) from the remote address
        for protocol in remote_addr.iter() {
            match protocol {
                Protocol::Ip4(ip) => {
                    circuit_addr.push(Protocol::Ip4(ip));
                }
                Protocol::Ip6(ip) => {
                    circuit_addr.push(Protocol::Ip6(ip));
                }
                Protocol::Tcp(port) => {
                    circuit_addr.push(Protocol::Tcp(port));
                }
                Protocol::Udp(port) => {
                    circuit_addr.push(Protocol::Udp(port));
                }
                Protocol::QuicV1 => {
                    circuit_addr.push(Protocol::QuicV1);
                }
                _ => continue,
            }
        }
        
        // Add the relay peer ID and circuit protocol
        circuit_addr.push(Protocol::P2p(relay_peer_id.clone()));
        circuit_addr.push(Protocol::P2pCircuit);
        
        Some(circuit_addr)
    }

    /// Check if an address is a relay circuit address
    fn is_circuit_address(addr: &Multiaddr) -> bool {
        use libp2p::multiaddr::Protocol;
        
        addr.iter().any(|protocol| matches!(protocol, Protocol::P2pCircuit))
    }

    /// Create a circuit address for a specific relay peer
    /// This creates the address that other peers can use to reach us through the relay
    fn create_circuit_address_for_relay(&self, relay_peer_id: &PeerId) -> Option<Multiaddr> {
        if let Some(mut addr) = self.relay_address.clone() {
            addr.push(Protocol::P2p(relay_peer_id.clone()));
            addr.push(Protocol::P2pCircuit);
            Some(addr)
        } else {
            None
        }
    }

    /// Handle network events from the event channel
    async fn handle_network_event(&mut self, event: NetworkEvent) -> Result<(), Box<dyn std::error::Error>> {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("LibP2P Manager received network event: {:?}", event),
        );
        
        match event {
            NetworkEvent::SendDirectMessage { peer_id, message } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Sending direct message to peer {}", peer_id),
                );
                self.send_direct_message_to_peer(peer_id, message).await?;
            }
            NetworkEvent::SendResponse { channel, message } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Sending response to peer."),
                );
                self.send_response_to_peer(channel, message).await?;
            }
            NetworkEvent::AddPeer { peer_id, address } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Adding peer {} at address {}", peer_id, address),
                );
                self.add_peer(peer_id, address)?;
            }
            NetworkEvent::PingPeer { peer_id } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Ping request for peer {}", peer_id),
                );
            }
            NetworkEvent::TryDirectConnectionUpgrade { peer_id } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Attempting direct connection upgrade to peer {}", peer_id),
                );
            }
            NetworkEvent::DiscoverPeers => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Discovering peers on the network."),
                );
            }
            NetworkEvent::ConnectToDiscoveredPeer { identity } => {
                self.connect_to_discovered_peer(&identity).await?;
            }
        }
        Ok(())
    }

    /// Extract the peer ID from a multiaddr if present
    fn extract_peer_id_from_address(addr: &Multiaddr) -> Option<PeerId> {
        use libp2p::multiaddr::Protocol;
        
        for protocol in addr.iter() {
            if let Protocol::P2p(peer_id) = protocol {
                return Some(peer_id);
            }
        }
        None
    }

    /// Check if we need to reconnect to relay and attempt reconnection with exponential backoff
    async fn check_and_reconnect_to_relay(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Only attempt reconnection if we have a relay address configured but are not connected
        if let Some(ref relay_addr) = self.relay_address.clone() {
            if !self.is_connected_to_relay {
                // Check if enough time has passed since last disconnection for backoff
                if let Some(last_disconnect) = self.last_disconnection_time {
                    let backoff_duration = self.calculate_backoff_duration();
                    if last_disconnect.elapsed() < backoff_duration {
                        // Still in backoff period, don't reconnect yet
                        return Ok(());
                    }
                }
                
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("🔄 Attempting to reconnect to relay (attempt {}) at: {}", 
                        self.reconnection_attempts + 1, relay_addr),
                );
                
                // Attempt to reconnect
                if let Err(e) = self.swarm.dial(relay_addr.clone()) {
                    self.reconnection_attempts += 1;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to reconnect to relay: {}", e),
                    );
                } else {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        "📡 Reconnection attempt initiated - waiting for connection establishment",
                    );
                    self.reconnection_attempts += 1;
                }
            }
        }
        Ok(())
    }

    /// Calculate exponential backoff duration for reconnection attempts with jitter
    fn calculate_backoff_duration(&self) -> Duration {
        // Exponential backoff: 2s, 4s, 8s, 16s, then max out at 30s
        let base_delay = 2;
        let max_delay = 30;
        let delay_seconds = std::cmp::min(base_delay * (2_u32.saturating_pow(self.reconnection_attempts)), max_delay);
        
        // Add jitter (0-1000ms) to prevent thundering herd
        let jitter_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() % 1_000_000_000) / 1_000_000; // Convert to milliseconds
        
        Duration::from_millis((delay_seconds * 1000) as u64 + (jitter_ms % 1000) as u64)
    }

    /// Mark relay as connected and reset reconnection state
    fn mark_relay_connected(&mut self, peer_id: PeerId) {
        self.is_connected_to_relay = true;
        self.relay_peer_id = Some(peer_id);
        self.reconnection_attempts = 0;
        self.last_disconnection_time = None;
        
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("✅ Relay connection established with {} - reconnection state reset", peer_id),
        );
    }

    /// Mark relay as disconnected and start reconnection process
    fn mark_relay_disconnected(&mut self, peer_id: PeerId) {
        // Only mark as disconnected if this was our relay
        if let Some(relay_peer) = self.relay_peer_id {
            if relay_peer == peer_id {
                self.is_connected_to_relay = false;
                self.last_disconnection_time = Some(std::time::Instant::now());
                
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Error,
                    &format!("❌ Relay connection lost with {} - will attempt reconnection", peer_id),
                );
                
                let next_attempt_in = self.calculate_backoff_duration();
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("🔄 Next reconnection attempt in {:?}", next_attempt_in),
                );
            }
        }
    }

    /// Connect to a discovered peer using their circuit address
    pub async fn connect_to_discovered_peer(&mut self, identity: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some((_peer_id, circuit_addr)) = self.discovered_peers.get(identity).cloned() {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("🔗 Connecting to discovered peer {} via circuit: {}", identity, circuit_addr),
            );
            
            // Dial the peer through the circuit address
            if let Err(e) = self.swarm.dial(circuit_addr.clone()) {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to connect to peer {} at {}: {}", identity, circuit_addr, e),
                );
                return Err(Box::new(e));
            }
            
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("✅ Initiated connection to peer {} - waiting for connection establishment", identity),
            );
        } else {
            return Err(format!("Peer {} not found in discovered peers", identity).into());
        }
        
        Ok(())
    }

    /// Process the message queue and retry sending failed messages
    async fn process_message_queue(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.message_queue.is_empty() {
            return Ok(());
        }

        eprintln!("Processing message queue with {} messages", self.message_queue.len());
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("Processing message queue with {} messages", self.message_queue.len()),
        );

        let mut messages_to_retry = Vec::new();
        let mut messages_to_remove = Vec::new();
        let now = std::time::Instant::now();

        // Check each message in the queue
        for (index, queued_message) in self.message_queue.iter().enumerate() {
            eprintln!("Checking message {} to peer {}, retry_count: {}, last_attempt: {:?} ago", 
                index, queued_message.peer_id, queued_message.retry_count, 
                now.duration_since(queued_message.last_attempt));

            // Wait at least 1 second between retry attempts
            if now.duration_since(queued_message.last_attempt) < Duration::from_secs(1) {
                eprintln!("Message {} too recent, skipping", index);
                continue;
            }

            // Check if we've exceeded max retry attempts
            if queued_message.retry_count >= self.max_retry_attempts {
                eprintln!("Message {} exceeded max retries, removing", index);
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Error,
                    &format!("Message to peer {} exceeded max retry attempts ({}), dropping", 
                        queued_message.peer_id, self.max_retry_attempts),
                );
                messages_to_remove.push(index);
                continue;
            }

            // Check if peer is now connected
            let is_connected = self.swarm.is_connected(&queued_message.peer_id);
            let connected_peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();
            eprintln!("Peer {} connected status: {}", queued_message.peer_id, is_connected);
            eprintln!("All connected peers: {:?}", connected_peers);
            
            if is_connected {
                eprintln!("Adding message {} to retry list", index);
                messages_to_retry.push(index);
            } else {
                // Try to send anyway even if not showing as connected
                eprintln!("Peer not showing as connected, but attempting to send anyway");
                messages_to_retry.push(index);
            }
        }

        // Remove messages that exceeded retry limits (in reverse order to maintain indices)
        for &index in messages_to_remove.iter().rev() {
            eprintln!("Removing message at index {}", index);
            self.message_queue.remove(index);
        }

        // Retry messages for connected peers (in reverse order to maintain indices)
        for &index in messages_to_retry.iter().rev() {
            if let Some(mut queued_message) = self.message_queue.remove(index) {
                eprintln!("Retrying message to peer {} (attempt {})",
                    queued_message.peer_id, queued_message.retry_count + 1);
                
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Retrying message to peer {} (attempt {})", 
                        queued_message.peer_id, queued_message.retry_count + 1),
                );

                queued_message.retry_count += 1;
                queued_message.last_attempt = std::time::Instant::now();

                // Attempt to send the message
                let _request_id = self.swarm
                    .behaviour_mut()
                    .request_response
                    .send_request(&queued_message.peer_id, queued_message.message.clone());

                self.pending_outbound_requests.insert(_request_id, queued_message.clone());

                eprintln!("Message retry sent to peer {} {:?}", queued_message.peer_id, _request_id);
                
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Debug,
                    &format!("Message retry sent to peer {}", queued_message.peer_id),
                );

                // Update retry metadata
                queued_message.retry_count += 1;
                queued_message.last_attempt = now;

                if queued_message.retry_count < self.max_retry_attempts {
                    let peer_id = queued_message.peer_id;
                    let retry_count = queued_message.retry_count;
                    self.message_queue.push_back(queued_message);
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!("Queued message for peer {} for retry {}", peer_id, retry_count),
                    );
                } else {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Error,
                        &format!(
                            "Dropping message to peer {} after {} attempts",
                            queued_message.peer_id, queued_message.retry_count
                        ),
                    );
                }                
            }
        }

        if !self.message_queue.is_empty() {
            eprintln!("Message queue still has {} messages pending", self.message_queue.len());
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("Message queue status: {} messages pending", self.message_queue.len()),
            );
        }

        Ok(())
    }
}

/// Convert an ed25519 verifying key to a libp2p PeerId
pub fn verifying_key_to_peer_id(verifying_key: ed25519_dalek::VerifyingKey) -> Result<PeerId, Box<dyn std::error::Error>> {
    // Convert ed25519_dalek::VerifyingKey to libp2p::identity::PublicKey using the ed25519 module
    let ed25519_public_key = libp2p::identity::ed25519::PublicKey::try_from_bytes(verifying_key.as_bytes())?;
    let libp2p_public_key = libp2p::identity::PublicKey::from(ed25519_public_key);
    Ok(PeerId::from(libp2p_public_key))
}
