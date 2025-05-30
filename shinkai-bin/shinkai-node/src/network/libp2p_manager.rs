use futures::prelude::*;
use libp2p::{
    dcutr, gossipsub, identify, kad, noise, ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
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
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
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
        listen_port: Option<u16>,
        message_handler: ShinkaiMessageHandler,
        relay_address: Option<Multiaddr>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Generate a deterministic keypair based on node name
        let local_key = {
            let mut hasher = DefaultHasher::new();
            node_name.hash(&mut hasher);
            let hash = hasher.finish();
            // Use a simple seed-based approach
            let seed: [u8; 32] = {
                let mut seed = [0u8; 32];
                let hash_bytes = hash.to_be_bytes();
                for i in 0..4 {
                    seed[i * 8..(i + 1) * 8].copy_from_slice(&hash_bytes);
                }
                seed
            };
            libp2p::identity::Keypair::ed25519_from_bytes(seed).unwrap()
        };
        let local_peer_id = PeerId::from(local_key.public());

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Local peer id: {}", local_peer_id),
        );

        // Create transport
        let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .timeout(Duration::from_secs(20))
            .boxed();

        // Create GossipSub behavior with configuration for relay networking
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))  // Match relay's heartbeat interval
            .validation_mode(gossipsub::ValidationMode::Permissive)
            .mesh_outbound_min(1)  // Allow smaller meshes for relay scenarios
            .mesh_n_low(1)         // Lower minimum mesh size for relay
            .mesh_n(2)             // Target mesh size (relay + maybe 1 peer)
            .mesh_n_high(4)        // Lower maximum for relay scenarios
            .gossip_lazy(2)        // Reduce gossip for relay scenarios
            .fanout_ttl(Duration::from_secs(30))  // Shorter TTL for relay
            .build()
            .expect("Valid config");

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;

        // Subscribe to default shinkai topic
        let shinkai_topic = gossipsub::IdentTopic::new("shinkai-network");
        gossipsub.subscribe(&shinkai_topic)?;

        // Create Identify behavior with compatible protocol
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shinkai/1.0.0".to_string(),
            local_key.public(),
        ));

        // Create Kademlia behavior
        let mut kademlia = kad::Behaviour::new(
            local_peer_id,
            kad::store::MemoryStore::new(local_peer_id),
        );
        kademlia.set_mode(Some(kad::Mode::Server));

        // Create ping behavior
        let ping = ping::Behaviour::new(ping::Config::new());

        // Create DCUtR behavior for hole punching
        let dcutr = dcutr::Behaviour::new(local_peer_id);

        // Combine all behaviors
        let behaviour = ShinkaiNetworkBehaviour {
            gossipsub,
            identify,
            kademlia,
            ping,
            dcutr,
        };

        // Create swarm
        let swarm_config = libp2p::swarm::Config::with_tokio_executor();
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, swarm_config);

        // Configure listening
        let listen_addr = if let Some(port) = listen_port {
            format!("/ip4/0.0.0.0/tcp/{}", port)
        } else {
            "/ip4/0.0.0.0/tcp/0".to_string()
        };

        swarm.listen_on(listen_addr.parse()?)?;

        // Connect to relay if provided
        if let Some(relay_addr) = relay_address {
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
        // Add to Kademlia routing table
        self.swarm.behaviour_mut().kademlia.add_address(&peer_id, address.clone());
        
        // Dial the peer
        self.swarm.dial(address)?;
        
        Ok(())
    }

    /// Force discovery of peers and mesh building
    pub fn force_peer_discovery(&mut self) {
        // Trigger Kademlia bootstrap to find more peers
        let _ = self.swarm.behaviour_mut().kademlia.bootstrap();
        
        // Get all known peers and try to add them to gossipsub mesh
        let known_peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();
        
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Force discovery: found {} connected peers", known_peers.len()),
        );
        
        // For each connected peer, try to make them gossipsub-aware
        for peer_id in &known_peers {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("Adding peer {} to mesh consideration", peer_id),
            );
            
            // Force gossipsub to consider this peer for mesh
            // We'll try to send a subscribe message to make the peer aware of our topics
            let topic = gossipsub::IdentTopic::new("shinkai-network");
            if let Err(e) = self.swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Debug,
                    &format!("Already subscribed to topic: {}", e),
                );
            }
        }
        
        // Also publish a discovery message to announce our presence
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
                
                // Add peer addresses to Kademlia
                for addr in info.listen_addrs {
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
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
            _ => {}
        }
        Ok(())
    }
}

/// Convert SocketAddr to Multiaddr
pub fn socket_addr_to_multiaddr(addr: SocketAddr) -> Multiaddr {
    match addr {
        SocketAddr::V4(addr) => format!("/ip4/{}/tcp/{}", addr.ip(), addr.port()).parse().unwrap(),
        SocketAddr::V6(addr) => format!("/ip6/{}/tcp/{}", addr.ip(), addr.port()).parse().unwrap(),
    }
}

/// Convert PeerId and SocketAddr to a format similar to the current peer tuple
pub fn peer_id_to_profile_name(peer_id: PeerId) -> String {
    format!("peer_{}", peer_id.to_string()[..8].to_lowercase())
} 