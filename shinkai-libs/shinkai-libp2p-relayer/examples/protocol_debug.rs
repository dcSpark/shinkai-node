use libp2p::{
    futures::StreamExt,
    identify::{self, Event as IdentifyEvent},
    noise, ping, quic, tcp, yamux,
    swarm::{NetworkBehaviour, SwarmEvent, Config},
    Multiaddr, PeerId, Swarm, Transport,
};
use std::time::Duration;

#[derive(NetworkBehaviour)]
struct ProtocolDebugBehaviour {
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 LibP2P Protocol Negotiation Debug Tool");
    println!("==========================================");

    // Get relay address from command line args or use default
    let args: Vec<String> = std::env::args().collect();
    let relay_addr = args.get(1).cloned().expect("Relay IP address must be specified as first argument");

    println!("📡 Connecting to relay: {}", relay_addr);

    // Generate a test keypair
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("🆔 Debug client PeerId: {}", local_peer_id);

    // Create transport with detailed logging
    let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default())
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&local_key)?)
        .multiplex(yamux::Config::default())
        .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

    let quic_transport = quic::tokio::Transport::new(quic::Config::new(&local_key))
        .map(|(peer, muxer), _| (peer, libp2p::core::muxing::StreamMuxerBox::new(muxer)));

    let transport = quic_transport
        .or_transport(tcp_transport)
        .map(|either_output, _| either_output.into_inner())
        .boxed();

    // Configure identify with the same protocol version as Shinkai nodes
    let identify = identify::Behaviour::new(identify::Config::new(
        "/shinkai/1.0.0".to_string(),
        local_key.public(),
    ));

    let ping = ping::Behaviour::new(ping::Config::new());

    let behaviour = ProtocolDebugBehaviour {
        identify,
        ping,
    };

    let mut swarm = Swarm::new(transport, behaviour, local_peer_id, Config::with_tokio_executor());

    // Connect to relay
    let tcp_addr: Multiaddr = format!("/ip4/{}/tcp/{}", 
        relay_addr.split(':').next().unwrap(),
        relay_addr.split(':').nth(1).unwrap())
        .parse()?;

    println!("🔌 Connecting to: {}", tcp_addr);
    swarm.dial(tcp_addr)?;

    println!("\n📊 Monitoring protocol negotiations...");
    println!("Protocol mismatches will be logged here.\n");

    let mut connection_established = false;
    let start_time = std::time::Instant::now();

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                println!("✅ Connected to peer: {}", peer_id);
                println!("   Endpoint: {:?}", endpoint);
                connection_established = true;
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("💔 Connection closed with peer: {} (cause: {:?})", peer_id, cause);
                if connection_established {
                    break;
                }
            }
            SwarmEvent::Behaviour(ProtocolDebugBehaviourEvent::Identify(IdentifyEvent::Received { peer_id, info, .. })) => {
                println!("\n🔍 === PEER IDENTIFICATION ===");
                println!("Peer ID: {}", peer_id);
                println!("Protocol Version: {}", info.protocol_version);
                println!("Agent Version: {}", info.agent_version);
                println!("Listen Addresses: {:?}", info.listen_addrs);
                println!("\n📋 Supported Protocols:");
                
                let mut supported_protocols = Vec::new();
                for protocol in &info.protocols {
                    println!("  ✓ {}", protocol);
                    supported_protocols.push(protocol.to_string());
                }
                
                // Check for protocol compatibility
                println!("\n🔍 === PROTOCOL COMPATIBILITY ANALYSIS ===");
                
                let relay_protocols = ["/libp2p/circuit/relay/0.2.0/hop", "/ipfs/kad/1.0.0"];
                let node_protocols = ["/gossipsub/1.1.0", "/ipfs/id/1.0.0", "/ipfs/ping/1.0.0", "/libp2p/dcutr"];
                
                println!("Relay-specific protocols (may cause 'no protocol agreed' messages):");
                for protocol in &relay_protocols {
                    let supported = supported_protocols.iter().any(|p| p.contains(protocol));
                    if supported {
                        println!("  ✅ {} - SUPPORTED", protocol);
                    } else {
                        println!("  ❌ {} - NOT SUPPORTED (will cause negotiation failures)", protocol);
                    }
                }
                
                println!("\nCommon protocols:");
                for protocol in &node_protocols {
                    let supported = supported_protocols.iter().any(|p| p.contains(protocol));
                    if supported {
                        println!("  ✅ {} - SUPPORTED", protocol);
                    } else {
                        println!("  ❌ {} - NOT SUPPORTED", protocol);
                    }
                }
                
                println!("\n📝 === EXPLANATION ===");
                println!("The 'no protocol could be agreed upon' messages occur when:");
                println!("1. The relay tries to use /libp2p/circuit/relay/0.2.0/hop protocol");
                println!("2. The relay tries to use /libp2p/kad/1.0.0 (Kademlia) protocol");
                println!("3. Your node doesn't support these protocols");
                println!("4. This is NORMAL behavior - your node works fine without them");
                println!("================================\n");
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                println!("⚠️  Incoming connection error: {}", error);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                println!("⚠️  Outgoing connection error to {:?}: {}", peer_id, error);
            }
            _ => {}
        }
        
        // Exit after 30 seconds
        if start_time.elapsed() > Duration::from_secs(30) {
            println!("🏁 Debug session completed after 30 seconds");
            break;
        }
    }

    Ok(())
} 