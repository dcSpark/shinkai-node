use ed25519_dalek::SigningKey;
use futures::prelude::*;
use libp2p::{
    dcutr, gossipsub, identify, noise, ping, quic,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc;

/// The libp2p network behavior combining all protocols
#[derive(NetworkBehaviour)]
pub struct ShinkaiNetworkBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub dcutr: dcutr::Behaviour,
}

/// Events that can be sent through the network
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// A message to be sent to a specific peer
    SendMessage {
        peer_id: PeerId,
        message: ShinkaiMessage,
    },
    /// A message to be broadcast to all peers in a topic
    BroadcastMessage {
        topic: String,
        message: ShinkaiMessage,
    },
    /// Add a peer to connect to
    AddPeer {
        peer_id: PeerId,
        address: Multiaddr,
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

        // Subscribe to default shinkai topic
        let shinkai_topic = gossipsub::IdentTopic::new("shinkai-network");
        gossipsub.subscribe(&shinkai_topic)?;
        #[cfg(feature = "debug")]
        eprintln!(">> DEBUG: LibP2P {} subscribed to topic: 'shinkai-network'", node_name);

        // Also subscribe to node-specific topics to receive messages addressed to this node
        let node_topic = gossipsub::IdentTopic::new(format!("shinkai-{}", node_name));
        gossipsub.subscribe(&node_topic)?;
        #[cfg(feature = "debug")]
        eprintln!(">> DEBUG: LibP2P {} subscribed to topic: 'shinkai-{}'", node_name, node_name);
        
        // Subscribe to the base node name (without subidentity) for broader message reception
        if let Ok(parsed_name) = shinkai_message_primitives::schemas::shinkai_name::ShinkaiName::new(node_name.clone()) {
            let base_node_name = parsed_name.get_node_name_string();
            let base_topic = gossipsub::IdentTopic::new(format!("shinkai-{}", base_node_name));
            gossipsub.subscribe(&base_topic)?;
            #[cfg(feature = "debug")]
            eprintln!(">> DEBUG: LibP2P {} subscribed to base topic: 'shinkai-{}'", node_name, base_node_name);
        }

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Subscribed to topics: shinkai-network, shinkai-{}", node_name),
        );

        // Create Identify behavior with compatible protocol
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shinkai/1.0.0".to_string(),
            local_key.public(),
        ));

        // Create ping behavior
        let ping = ping::Behaviour::new(ping::Config::new());

        // Create DCUtR behavior for hole punching
        let dcutr = dcutr::Behaviour::new(local_peer_id);

        // Combine all behaviors
        let behaviour = ShinkaiNetworkBehaviour {
            gossipsub,
            identify,
            ping,
            dcutr,
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

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Get listening addresses
    pub fn listeners(&self) -> impl Iterator<Item = &Multiaddr> {
        self.swarm.listeners()
    }

    /// Send a message to a specific peer
    pub async fn send_message_to_peer(
        &mut self,
        peer_id: PeerId,
        message: ShinkaiMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // For now, we'll use GossipSub to send messages
        // In a more sophisticated implementation, you might use request-response
        let serialized = serde_json::to_string(&message)?;
        let topic = gossipsub::IdentTopic::new(format!("shinkai-direct-{}", peer_id));
        
        // Subscribe to the topic first if not already subscribed
        let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&topic);
        
        // Publish the message
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, serialized.as_bytes())?;

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
            &format!("Broadcasted message to topics: shinkai-network and {}", topic),
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

    /// Dial a peer directly by address
    pub fn dial_peer(&mut self, address: Multiaddr) -> Result<(), Box<dyn std::error::Error>> {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Dialing peer at {}", address),
        );
        
        if let Err(e) = self.swarm.dial(address.clone()) {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!("Failed to dial peer at {}: {}", address, e),
            );
            return Err(Box::new(e));
        }
        
        Ok(())
    }

    /// Force discovery of peers and mesh building
    pub fn force_peer_discovery(&mut self) {
        // For relay scenarios, we rely on gossipsub discovery through the relay
        // instead of Kademlia DHT to avoid direct connection attempts
        
        let known_peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();
        
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Force discovery: found {} connected peers", known_peers.len()),
        );
        
        // For each connected peer, ensure they're part of gossipsub
        for peer_id in &known_peers {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("Adding peer {} to mesh consideration", peer_id),
            );
            
            // Ensure we're subscribed to the main topic
            let topic = gossipsub::IdentTopic::new("shinkai-network");
            if let Err(e) = self.swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Debug,
                    &format!("Already subscribed to topic: {}", e),
                );
            }
        }
        
        // Publish a discovery message to announce our presence
        let discovery_message = format!("{{\"type\":\"discovery\",\"peer_id\":\"{}\"}}",
            self.swarm.local_peer_id());
        let topic = gossipsub::IdentTopic::new("shinkai-network");
        
        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, discovery_message.as_bytes()) {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("Failed to publish discovery message: {}", e),
            );
        }
    }

    /// Main event loop for processing libp2p events
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Set up a timer for frequent peer discovery (every 10 seconds)
        let mut discovery_interval = tokio::time::interval(Duration::from_secs(10));
        
        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await?;
                }
                
                // Handle network events from other parts of the application
                event = self.event_receiver.recv() => {
                    match event {
                        Some(NetworkEvent::SendMessage { peer_id, message }) => {
                            if let Err(e) = self.send_message_to_peer(peer_id, message).await {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to send message to peer {}: {}", peer_id, e),
                                );
                            }
                        }
                        Some(NetworkEvent::BroadcastMessage { topic, message }) => {
                            if let Err(e) = self.broadcast_message(&topic, message).await {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to broadcast message to topic {}: {}", topic, e),
                                );
                            }
                        }
                        Some(NetworkEvent::AddPeer { peer_id, address }) => {
                            if let Err(e) = self.add_peer(peer_id, address.clone()) {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to add peer {} at {}: {}", peer_id, address, e),
                                );
                            }
                        }
                        None => break, // Channel closed
                    }
                }
                
                // Periodic peer discovery
                _ = discovery_interval.tick() => {
                    self.force_peer_discovery();
                }
            }
        }
        Ok(())
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
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Identify(identify::Event::Received { peer_id, info })) => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Identified peer {} with protocol version {}", peer_id, info.protocol_version),
                );
                
                // For relay scenarios, we don't add peers to Kademlia since it causes direct connections
                // All peer discovery should happen through gossipsub via the relay
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
            _ => {}
        }
        Ok(())
    }
}

/// Convert SocketAddr to Multiaddr (TCP)
pub fn socket_addr_to_multiaddr(addr: SocketAddr) -> Multiaddr {
    match addr {
        SocketAddr::V4(addr) => format!("/ip4/{}/tcp/{}", addr.ip(), addr.port()).parse().unwrap(),
        SocketAddr::V6(addr) => format!("/ip6/{}/tcp/{}", addr.ip(), addr.port()).parse().unwrap(),
    }
}

/// Convert SocketAddr to QUIC Multiaddr
pub fn socket_addr_to_quic_multiaddr(addr: SocketAddr) -> Multiaddr {
    match addr {
        SocketAddr::V4(addr) => format!("/ip4/{}/udp/{}/quic-v1", addr.ip(), addr.port()).parse().unwrap(),
        SocketAddr::V6(addr) => format!("/ip6/{}/udp/{}/quic-v1", addr.ip(), addr.port()).parse().unwrap(),
    }
}

/// Convert PeerId and SocketAddr to a format similar to the current peer tuple
pub fn peer_id_to_profile_name(peer_id: PeerId) -> String {
    format!("peer_{}", peer_id.to_string()[..8].to_lowercase())
} 