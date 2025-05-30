use futures::prelude::*;
use libp2p::{
    dcutr, gossipsub, identify, kad, mdns, noise, ping,
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
    pub mdns: mdns::tokio::Behaviour,
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

        // Create GossipSub behavior
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .build()
            .expect("Valid config");

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;

        // Subscribe to default shinkai topic
        let shinkai_topic = gossipsub::IdentTopic::new("shinkai-network");
        gossipsub.subscribe(&shinkai_topic)?;

        // Create mDNS behavior
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

        // Create Identify behavior
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
            mdns,
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
        let gossip_topic = gossipsub::IdentTopic::new(topic);
        
        // Subscribe to the topic first if not already subscribed
        let _ = self.swarm.behaviour_mut().gossipsub.subscribe(&gossip_topic);
        
        // Publish the message
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(gossip_topic, serialized.as_bytes())?;

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

    /// Main event loop for processing libp2p events
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
                for (peer_id, multiaddr) in peers {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Discovered peer {} at {}", peer_id, multiaddr),
                    );
                    
                    // Add discovered peer to Kademlia
                    self.swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                }
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
                for (peer_id, multiaddr) in peers {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Peer {} at {} expired", peer_id, multiaddr),
                    );
                }
            }
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            })) => {
                // Handle incoming GossipSub messages
                if let Ok(message_str) = String::from_utf8(message.data) {
                    if let Ok(shinkai_message) = serde_json::from_str::<ShinkaiMessage>(&message_str) {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("Received message from peer {}", message.source.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string())),
                        );
                        
                        // Handle the message using the message handler
                        if let Some(source) = message.source {
                            self.message_handler.handle_message(source, shinkai_message).await;
                        }
                    }
                }
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