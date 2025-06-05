use ed25519_dalek::SigningKey;
use futures::prelude::*;
use libp2p::{
    dcutr, gossipsub, identify, kad, noise, ping, quic, request_response,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use std::{
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc;

/// The libp2p network behavior combining all protocols
/// Kademlia is always enabled for better peer discovery and protocol compatibility
#[derive(NetworkBehaviour)]
pub struct ShinkaiNetworkBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub request_response: request_response::json::Behaviour<ShinkaiMessage, ShinkaiMessage>,
}

/// Events that can be sent through the network
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// A message to be sent directly to a specific peer using request-response
    SendDirectMessage {
        peer_id: PeerId,
        message: ShinkaiMessage,
    },
    /// A message to be broadcast to all peers in a topic using gossipsub
    #[allow(dead_code)]
    BroadcastMessage {
        topic: String,
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
}

/// The main libp2p network manager
pub struct LibP2PManager {
    swarm: Swarm<ShinkaiNetworkBehaviour>,
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
    event_receiver: mpsc::UnboundedReceiver<NetworkEvent>,
    message_handler: Arc<ShinkaiMessageHandler>,
}

use crate::network::libp2p_message_handler::ShinkaiMessageHandler;

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
            &format!("Local peer id: {}", local_peer_id),
        );

        // Create transport with QUIC and TCP fallback
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .timeout(Duration::from_secs(20))
            .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

        let quic_transport = quic::tokio::Transport::new(quic::Config::new(&local_key))
            .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

        // Combine QUIC and TCP transports - QUIC will be preferred, TCP as fallback
        let transport = quic_transport
            .or_transport(tcp_transport)
            .map(|either_output, _| either_output.into_inner())
            .boxed();

        // Create GossipSub behavior with simple default configuration
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Permissive)
            .mesh_outbound_min(0)  // Allow zero outbound connections during startup
            .mesh_n_low(1)         // Minimum peers in mesh
            .mesh_n(3)             // Target mesh size
            .mesh_n_high(5)        // Higher maximum for mesh
            .gossip_lazy(3)        // Gossip settings
            .fanout_ttl(Duration::from_secs(60))  // TTL for fanout
            .gossip_retransimission(3)  // Retransmit important messages
            .duplicate_cache_time(Duration::from_secs(60))  // Cache for deduplication
            .max_transmit_size(262144) // 256KB max message size
            .build()
            .expect("Valid config");

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;

        // Subscribe only to discovery topic - no longer using gossipsub for direct messaging
        let discovery_topic = gossipsub::IdentTopic::new("shinkai-network");
        gossipsub.subscribe(&discovery_topic)?;

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            "Subscribed to broadcasting topic: shinkai-network",
        );

        // Create Identify behavior with compatible protocol and include node identity
        let mut identify_config = identify::Config::new(
            "/shinkai/1.0.0".to_string(),
            local_key.public(),
        );
        // Include the node identity in the agent version for relay identification
        identify_config = identify_config.with_agent_version(format!("shinkai-node-{}", node_name));
        let identify = identify::Behaviour::new(identify_config);

        // Create ping behavior
        let ping = ping::Behaviour::new(ping::Config::new());

        // Create DCUtR behavior for hole punching
        let dcutr = dcutr::Behaviour::new(local_peer_id);

        // Create Kademlia behavior with proper protocol configuration for relay compatibility
        let mut kademlia_config = kad::Config::default();
        kademlia_config.set_protocol_names(vec![
            libp2p::StreamProtocol::new("/kad/1.0.0"),
            libp2p::StreamProtocol::new("/kademlia/1.0.0"),
            libp2p::StreamProtocol::new("/ipfs/kad/1.0.0"),
        ]);
        
        let mut kademlia = kad::Behaviour::with_config(
            local_peer_id,
            kad::store::MemoryStore::new(local_peer_id),
            kademlia_config,
        );
        
        // Start as server mode to participate fully in DHT
        kademlia.set_mode(Some(kad::Mode::Server));

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            "Kademlia DHT enabled with multiple protocol versions for relay compatibility",
        );

        // Create request-response behavior for direct messaging using JSON codec
        let request_response = request_response::json::Behaviour::new(
            std::iter::once((libp2p::StreamProtocol::new("/shinkai/message/1.0.0"), request_response::ProtocolSupport::Full)),
            request_response::Config::default(),
        );

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            "Request-Response protocol enabled for direct peer messaging",
        );

        let behaviour = ShinkaiNetworkBehaviour {
            gossipsub,
            identify,
            ping,
            dcutr,
            kademlia,
            request_response,
        };

        // Create swarm
        let swarm_config = libp2p::swarm::Config::with_tokio_executor();
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, swarm_config);

        // Listen on both QUIC and TCP ports - relay networking still requires listening to connect to/from relay
        let (tcp_listen_addr, quic_listen_addr) = if let Some(port) = listen_port {
            (
                format!("/ip4/0.0.0.0/tcp/{}", port),
                format!("/ip4/0.0.0.0/udp/{}/quic-v1", port),
            )
        } else {
            (
                "/ip4/0.0.0.0/tcp/0".to_string(),
                "/ip4/0.0.0.0/udp/0/quic-v1".to_string(),
            )
        };

        // Listen on both TCP and QUIC
        swarm.listen_on(tcp_listen_addr.parse()?)?;
        swarm.listen_on(quic_listen_addr.parse()?)?;
        
        if relay_address.is_some() {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Listening on {} and {} (relay mode - for relay connections)", tcp_listen_addr, quic_listen_addr),
            );
        } else {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Listening on {} and {} (direct mode)", tcp_listen_addr, quic_listen_addr),
            );
        }

        // Connect to relay if provided
        if let Some(relay_addr) = relay_address {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Connecting to relay at: {}", relay_addr),
            );
            swarm.dial(relay_addr)?;
        }

        // Create event channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Ok(LibP2PManager {
            swarm,
            event_sender,
            event_receiver,
            message_handler: Arc::new(message_handler),
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
        eprintln!(">> DEBUG: Sending direct message to peer {} using request-response", peer_id);

        // Send the message using request-response protocol
        let _request_id = self.swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer_id, message);

        eprintln!(">> DEBUG: Direct message request sent to peer {}", peer_id);

        Ok(())
    }

    /// Broadcast a message to all peers in a topic
    pub async fn broadcast_message(
        &mut self,
        topic: &str,
        message: ShinkaiMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = serde_json::to_string(&message)?;
        
        // Always subscribe to the general shinkai-network topic for peer discovery
        let general_topic = gossipsub::IdentTopic::new("shinkai-network");
        let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&general_topic);
        
        // Also subscribe to the specific topic
        let specific_topic = gossipsub::IdentTopic::new(topic);
        let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&specific_topic);
        
        // Publish to both topics to ensure message delivery
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(general_topic, serialized.as_bytes())?;
            
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(specific_topic, serialized.as_bytes())?;

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Broadcasted message to topics: shinkai-network"),
        );

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
        let mut ping_timer = tokio::time::interval(Duration::from_secs(30));
        let mut discovery_timer = tokio::time::interval(Duration::from_secs(60));
        let mut kademlia_bootstrap_timer = tokio::time::interval(Duration::from_secs(120)); // Every 2 minutes
        
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
                _ = ping_timer.tick() => {
                    self.send_ping().await?;
                }
                _ = discovery_timer.tick() => {
                    self.send_discovery_message().await?;
                }
                _ = kademlia_bootstrap_timer.tick() => {
                    // Bootstrap Kademlia for peer discovery
                    if let Err(e) = self.swarm.behaviour_mut().bootstrap_kademlia() {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Debug,
                            &format!("Kademlia bootstrap failed: {}", e),
                        );
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Debug,
                            "Initiated Kademlia bootstrap for peer discovery",
                        );
                    }
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
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            })) => {
                // Handle incoming GossipSub messages
                if let Ok(message_str) = String::from_utf8(message.data) {
                    // Check if it's a discovery message
                    if message_str.contains("\"type\":\"discovery\"") || message_str.contains("\"type\":\"peer_joined\"") {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("Received discovery message from peer {}", 
                                message.source.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string())),
                        );
                        
                        // If we have a source peer, make sure they're in our Kademlia table
                        if let Some(source_peer) = message.source {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Debug,
                                &format!("Processing discovery from peer: {}", source_peer),
                            );
                        }
                        return Ok(()); // Don't process discovery messages as regular Shinkai messages
                    }
                    
                    // Try to parse as a regular Shinkai message
                    if let Ok(shinkai_message) = serde_json::from_str::<ShinkaiMessage>(&message_str) {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("Received Shinkai message from peer {}", 
                                message.source.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string())),
                        );
                        
                        // Handle the message using the message handler
                        if let Some(source) = message.source {
                            self.message_handler.handle_message(source, shinkai_message).await;
                        }
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Debug,
                            &format!("Received non-Shinkai message: {}", message_str),
                        );
                    }
                }
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic })) => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Peer {} subscribed to topic {}", peer_id, topic),
                );
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed { peer_id, topic })) => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Peer {} unsubscribed from topic {}", peer_id, topic),
                );
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Gossipsub(gossipsub::Event::GossipsubNotSupported { peer_id })) => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Error,
                    &format!("Peer {} does not support Gossipsub", peer_id),
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
                
                // Add peer to Kademlia for better peer discovery
                for addr in &info.listen_addrs {
                    self.swarm.behaviour_mut().add_peer_to_kademlia(peer_id, addr.clone());
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!("Added peer {} with address {} to Kademlia DHT", peer_id, addr),
                    );
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

                                // Handle the incoming request message
                                self.message_handler.handle_message(peer, request.clone()).await;

                                // Send acknowledgment response
                                let ack_response = request.clone(); // For now, echo back the message as acknowledgment
                                if let Err(e) = self.swarm.behaviour_mut().request_response.send_response(channel, ack_response) {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!("Failed to send response to peer {}: {:?}", peer, e),
                                    );
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Info,
                                    &format!("Received direct message response from peer {}", peer),
                                );
                                
                                // Handle the response (acknowledgment)
                                self.message_handler.handle_message(peer, response).await;
                            }
                        }
                    }
                    request_response::Event::OutboundFailure { peer, error, .. } => {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to send direct message to peer {}: {:?}", peer, error),
                        );
                    }
                    request_response::Event::InboundFailure { peer, error, .. } => {
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
                    }
                }
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Kademlia(kad_event)) => {
                // Handle Kademlia events for peer discovery
                match kad_event {
                        kad::Event::OutboundQueryProgressed { id: _, result, .. } => {
                            match result {
                                kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { peer, num_remaining })) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Info,
                                        &format!("Kademlia bootstrap progress: peer={}, remaining={}", peer, num_remaining),
                                    );
                                }
                                kad::QueryResult::Bootstrap(Err(e)) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!("Kademlia bootstrap error: {:?}", e),
                                    );
                                }
                                kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk { peers, .. })) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Debug,
                                        &format!("Found {} close peers via Kademlia", peers.len()),
                                    );
                                }
                                kad::QueryResult::GetClosestPeers(Err(e)) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Debug,
                                        &format!("Kademlia get closest peers error: {:?}", e),
                                    );
                                }
                                _ => {}
                            }
                        }
                        kad::Event::RoutingUpdated { peer, is_new_peer, addresses, .. } => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Debug,
                                &format!("Kademlia routing updated: peer={}, new={}, addresses={:?}", peer, is_new_peer, addresses),
                            );
                        }
                        _ => {}
                    }
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Connected to peer {}", peer_id),
                );
                
                // When a new peer connects, try to add them to gossipsub
                let topic = gossipsub::IdentTopic::new("shinkai-network");
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!("Already subscribed to topic: {}", e),
                    );
                }
                
                // Announce our presence to the new peer
                let discovery_message = format!("{{\"type\":\"peer_joined\",\"peer_id\":\"{}\"}}",
                    self.swarm.local_peer_id());
                
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, discovery_message.as_bytes()) {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!("Failed to announce presence: {}", e),
                    );
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Disconnected from peer {}: {:?}", peer_id, cause),
                );
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Debug,
                    &format!("Incoming connection error: {}", error),
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
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

    /// Check if a peer is connected (libp2p ping is automatic)
    async fn ensure_peer_connected(&mut self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Checking connection to peer {}", peer_id),
        );
        
        // Check if we're already connected to this peer
        if self.swarm.is_connected(&peer_id) {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Already connected to peer {}", peer_id),
            );
            return Ok(());
        }
        
        // libp2p ping behavior is automatic - we just need to ensure connection
        // The ping events will be automatically generated when connected
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Not currently connected to peer {} - ping results will be available once connected", peer_id),
        );
        
        Ok(())
    }

    /// Send a discovery message to help with peer discovery  
    async fn send_discovery_message(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let discovery_message = format!("{{\"type\":\"discovery\",\"peer_id\":\"{}\"}}",
            self.swarm.local_peer_id());
        
        let topic = gossipsub::IdentTopic::new("shinkai-network");
        
        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, discovery_message.as_bytes()) {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("Failed to send discovery message: {}", e),
            );
        } else {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                "Sent discovery message to network",
            );
        }
        
        Ok(())
    }

    /// Send ping to all connected peers (handled automatically by libp2p)
    async fn send_ping(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let known_peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();
        
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("Ping status check for {} connected peers (handled automatically by libp2p)", known_peers.len()),
        );
        
        // libp2p ping behavior handles pinging automatically
        // We just need to log that we have connections
        for peer in &known_peers {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("Connected to peer: {}", peer),
            );
        }
        
        Ok(())
    }

    /// Handle network events from the event channel
    async fn handle_network_event(&mut self, event: NetworkEvent) -> Result<(), Box<dyn std::error::Error>> {
        eprintln!(">> DEBUG: LibP2P Manager received network event: {:?}", event);
        
        match event {
            NetworkEvent::SendDirectMessage { peer_id, message } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Sending direct message to peer {}", peer_id),
                );
                self.send_direct_message_to_peer(peer_id, message).await?;
            }
            NetworkEvent::BroadcastMessage { topic, message } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Broadcasting message to topic {}", topic),
                );
                self.broadcast_message(&topic, message).await?;
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
                    &format!("Ping request for peer {} - libp2p ping is automatic when connected", peer_id),
                );
                self.ensure_peer_connected(peer_id).await?;
            }
        }
        Ok(())
    }
}

impl ShinkaiNetworkBehaviour {
    /// Bootstrap Kademlia
    pub fn bootstrap_kademlia(&mut self) -> Result<(), String> {
        self.kademlia.bootstrap()
            .map(|_query_id| ()) // Ignore the query ID, just return success
            .map_err(|e| format!("Kademlia bootstrap failed: {:?}", e))
    }
    
    /// Add a peer to Kademlia
    pub fn add_peer_to_kademlia(&mut self, peer_id: PeerId, address: Multiaddr) {
        self.kademlia.add_address(&peer_id, address);
    }
}

/// Convert an ed25519 verifying key to a libp2p PeerId
pub fn verifying_key_to_peer_id(verifying_key: ed25519_dalek::VerifyingKey) -> Result<PeerId, Box<dyn std::error::Error>> {
    // Convert ed25519_dalek::VerifyingKey to libp2p::identity::PublicKey using the ed25519 module
    let ed25519_public_key = libp2p::identity::ed25519::PublicKey::try_from_bytes(verifying_key.as_bytes())?;
    let libp2p_public_key = libp2p::identity::PublicKey::from(ed25519_public_key);
    Ok(PeerId::from(libp2p_public_key))
}
