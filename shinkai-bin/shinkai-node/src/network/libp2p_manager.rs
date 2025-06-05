use ed25519_dalek::SigningKey;
use futures::prelude::*;
use libp2p::{
    dcutr, identify, noise, ping, quic, request_response,
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
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub dcutr: dcutr::Behaviour,
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
}

/// The main libp2p network manager
pub struct LibP2PManager {
    swarm: Swarm<ShinkaiNetworkBehaviour>,
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
    event_receiver: mpsc::UnboundedReceiver<NetworkEvent>,
    message_handler: Arc<ShinkaiMessageHandler>,
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
        external_address: Option<Multiaddr>,
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
            identify,
            ping,
            dcutr,
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
        
        // If external address is provided, add it as an external address
        // This prevents private IP advertisement in Kademlia DHT
        if let Some(ext_addr) = external_address {
            swarm.add_external_address(ext_addr.clone());
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Added external address for DHT advertisement: {}", ext_addr),
            );
        }
        
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
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("Sending direct message to peer {} using request-response", peer_id),
        );

        // Send the message using request-response protocol
        let _request_id = self.swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer_id, message);

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("Direct message request sent to peer {}", peer_id),
        );

        Ok(())
    }

    /// Attempt to upgrade a relayed connection to a direct connection using DCUtR
    pub fn try_direct_connection_upgrade(&mut self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("🔄 Attempting direct connection upgrade to peer {} via DCUtR", peer_id),
        );
        
        // Check if we're connected to this peer through a relay
        if self.swarm.is_connected(&peer_id) {
            // The DCUtR behaviour automatically handles the upgrade when both peers support it
            // We just need to log that we're attempting it
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("   DCUtR will attempt hole punching for direct connection to {}", peer_id),
            );
        } else {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("   Not connected to peer {} - cannot attempt direct upgrade", peer_id),
            );
        }
        
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
            SwarmEvent::Behaviour(ShinkaiNetworkBehaviourEvent::Dcutr(dcutr_event)) => {
                // Handle DCUtR events for direct connection upgrades
                // Enhanced DCUtR event handling for direct connection upgrades
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("🔄 DCUtR: Direct connection upgrade event: {:?}", dcutr_event),
                );
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    "   Attempting to establish direct peer-to-peer connection",
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
                
                // Check if this peer is connected through a relay and attempt direct connection upgrade
                let mut circuit_addrs = Vec::new();
                let mut external_addrs = Vec::new();
                
                for addr in &info.listen_addrs {
                    if Self::is_circuit_address(addr) {
                        circuit_addrs.push(addr.clone());
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            &format!("Peer {} has relay circuit address: {}", peer_id, addr),
                        );
                    } else if Self::is_external_address(addr) {
                        external_addrs.push(addr.clone());
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Debug,
                            &format!("Peer {} has external address: {}", peer_id, addr),
                        );
                    }
                }
                
                // If peer is connected through relay and has external addresses, try direct connection upgrade
                if !circuit_addrs.is_empty() && !external_addrs.is_empty() {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Peer {} connected via relay but has external addresses - attempting DCUtR upgrade", peer_id),
                    );
                    if let Err(e) = self.try_direct_connection_upgrade(peer_id) {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Debug,
                            &format!("Failed to initiate DCUtR upgrade for peer {}: {}", peer_id, e),
                        );
                    }
                } else if !circuit_addrs.is_empty() {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Peer {} using relay circuit addressing - {} circuit addresses", 
                            peer_id, circuit_addrs.len()),
                    );
                } else if !external_addrs.is_empty() {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Peer {} using direct external addressing - {} external addresses", 
                            peer_id, external_addrs.len()),
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

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Connected to peer {}", peer_id),
                );
                
                // Check if this connection is through a relay and create circuit address
                if let Some(circuit_addr) = Self::extract_circuit_address(&endpoint.get_remote_address(), &peer_id) {
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Connected through relay, advertising circuit address: {}", circuit_addr),
                    );
                    
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
            NetworkEvent::TryDirectConnectionUpgrade { peer_id } => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Info,
                    &format!("Attempting direct connection upgrade to peer {}", peer_id),
                );
                self.try_direct_connection_upgrade(peer_id)?;
            }
        }
        Ok(())
    }
}

impl ShinkaiNetworkBehaviour {
    /// Attempt to initiate a direct connection upgrade using DCUtR
    pub fn initiate_dcutr_upgrade(&mut self, peer_id: PeerId) {
        // DCUtR automatically handles the upgrade when both peers support it
        // This is just a placeholder for any future manual triggering if needed
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("DCUtR upgrade for peer {} will be handled automatically", peer_id),
        );
    }
}

/// Convert an ed25519 verifying key to a libp2p PeerId
pub fn verifying_key_to_peer_id(verifying_key: ed25519_dalek::VerifyingKey) -> Result<PeerId, Box<dyn std::error::Error>> {
    // Convert ed25519_dalek::VerifyingKey to libp2p::identity::PublicKey using the ed25519 module
    let ed25519_public_key = libp2p::identity::ed25519::PublicKey::try_from_bytes(verifying_key.as_bytes())?;
    let libp2p_public_key = libp2p::identity::PublicKey::from(ed25519_public_key);
    Ok(PeerId::from(libp2p_public_key))
}
