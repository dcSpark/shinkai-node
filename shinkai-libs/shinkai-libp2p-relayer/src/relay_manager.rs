use libp2p::{
    futures::StreamExt,
    gossipsub::{self, Event as GossipsubEvent, MessageAuthenticity, ValidationMode, MessageId},
    identify::{self, Event as IdentifyEvent},
    noise, ping,
    relay::{self, Event as RelayEvent},
    swarm::{NetworkBehaviour, SwarmEvent, Config},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
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
}

pub struct RelayManager {
    swarm: Swarm<RelayBehaviour>,
    registered_peers: HashMap<String, PeerId>, // identity -> peer_id
    peer_identities: HashMap<PeerId, String>,  // peer_id -> identity
    message_sender: mpsc::UnboundedSender<RelayMessage>,
    message_receiver: mpsc::UnboundedReceiver<RelayMessage>,
}

impl RelayManager {
    pub async fn new(
        listen_port: u16,
        relay_node_name: String,
    ) -> Result<Self, LibP2PRelayError> {
        // Generate deterministic PeerId from relay name
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // Configure transport with relay support
        let transport = tcp::tokio::Transport::new(tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();

        // Configure gossipsub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(|message| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                message.data.hash(&mut hasher);
                MessageId::from(hasher.finish().to_string())
            })
            .build()
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Gossipsub config error: {}", e)))?;

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .map_err(|e| LibP2PRelayError::LibP2PError(format!("Gossipsub creation error: {}", e)))?;

        // Configure identify protocol
        let identify = identify::Behaviour::new(identify::Config::new(
            "/shinkai-relay/1.0.0".to_string(),
            local_key.public(),
        ));

        // Configure ping protocol
        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(30)));

        // Configure relay protocol
        let relay = relay::Behaviour::new(local_peer_id, Default::default());

        // Create the behaviour
        let behaviour = RelayBehaviour {
            gossipsub,
            identify,
            ping,
            relay,
        };

        // Create swarm with proper configuration
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, Config::with_tokio_executor());

        // Listen on the specified port
        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
            .parse()
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid listen address: {}", e)))?;

        swarm
            .listen_on(listen_addr)
            .map_err(|e| LibP2PRelayError::LibP2PError(format!("Failed to listen: {}", e)))?;

        // Create message channel
        let (message_sender, message_receiver) = mpsc::unbounded_channel();

        println!("LibP2P Relay initialized with PeerId: {}", local_peer_id);
        println!("Relay node name: {}", relay_node_name);

        Ok(RelayManager {
            swarm,
            registered_peers: HashMap::new(),
            peer_identities: HashMap::new(),
            message_sender,
            message_receiver,
        })
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
        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    if let Err(e) = self.handle_swarm_event(event).await {
                        eprintln!("Error handling swarm event: {}", e);
                    }
                }
                // Handle outgoing messages
                Some(message) = self.message_receiver.recv() => {
                    if let Err(e) = self.handle_outgoing_message(message).await {
                        eprintln!("Error handling outgoing message: {}", e);
                    }
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
                println!("Relay listening on {}", address);
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
                // We could use this to auto-register peers based on their identify info
            }
            SwarmEvent::Behaviour(RelayBehaviourEvent::Relay(RelayEvent::ReservationReqAccepted {
                src_peer_id,
                ..
            })) => {
                println!("Accepted relay reservation from: {}", src_peer_id);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("Connection established with peer: {}", peer_id);
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
        match RelayMessage::from_bytes(&data) {
            Ok(relay_message) => {
                self.route_message(relay_message).await?;
            }
            Err(e) => {
                eprintln!("Failed to parse relay message: {}", e);
            }
        }
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