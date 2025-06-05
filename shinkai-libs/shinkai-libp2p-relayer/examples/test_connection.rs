use libp2p::{
    futures::StreamExt,
    identify::{self, Event as IdentifyEvent},
    noise, ping, quic, tcp, yamux,
    swarm::{NetworkBehaviour, SwarmEvent, Config},
    Multiaddr, PeerId, Swarm, Transport,
};
use std::time::Duration;

#[derive(NetworkBehaviour)]
struct TestClientBehaviour {
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 LibP2P Relay Connection Diagnostic Tool");
    println!("=========================================");

    // Get relay address from command line args or use default
    let args: Vec<String> = std::env::args().collect();
    let relay_addr = args.get(1).cloned().expect("Relay IP address must be specified as first argument");

    println!("📡 Testing connection to relay: {}", relay_addr);

    // Generate a test keypair
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("🆔 Test client PeerId: {}", local_peer_id);

    // Create transport with QUIC and TCP support (same as relay)
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

    // Configure identify
    let identify = identify::Behaviour::new(identify::Config::new(
        "/shinkai-test-client/1.0.0".to_string(),
        local_key.public(),
    ));

    // Configure ping
    let ping = ping::Behaviour::new(ping::Config::new());

    // Create behaviour
    let behaviour = TestClientBehaviour {
        identify,
        ping,
    };

    // Create swarm
    let mut swarm = Swarm::new(transport, behaviour, local_peer_id, Config::with_tokio_executor());

    // Try both TCP and QUIC connections to the relay
    let tcp_addr: Multiaddr = format!("/ip4/{}/tcp/{}", 
        relay_addr.split(':').next().unwrap(),
        relay_addr.split(':').nth(1).unwrap())
        .parse()?;
    
    let quic_addr: Multiaddr = format!("/ip4/{}/udp/{}/quic-v1", 
        relay_addr.split(':').next().unwrap(),
        relay_addr.split(':').nth(1).unwrap())
        .parse()?;

    println!("🔌 Attempting to connect to relay...");
    println!("   TCP address:  {}", tcp_addr);
    println!("   QUIC address: {}", quic_addr);

    // Try QUIC first
    println!("\n🚀 Trying QUIC connection...");
    match swarm.dial(quic_addr.clone()) {
        Ok(_) => println!("✅ QUIC dial initiated"),
        Err(e) => println!("❌ QUIC dial failed: {}", e),
    }

    // Wait a moment, then try TCP as fallback
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("🚀 Trying TCP connection...");
    match swarm.dial(tcp_addr.clone()) {
        Ok(_) => println!("✅ TCP dial initiated"),
        Err(e) => println!("❌ TCP dial failed: {}", e),
    }

    println!("\n⏱️  Waiting for connection events (30 seconds)...");
    
    let mut connected = false;
    let mut peer_count: u32 = 0;
    let timeout = tokio::time::sleep(Duration::from_secs(30));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        println!("🎉 Connected to peer: {}", peer_id);
                        println!("   Endpoint: {:?}", endpoint);
                        connected = true;
                        peer_count += 1;
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        println!("💔 Connection closed with peer: {} (cause: {:?})", peer_id, cause);
                        peer_count = peer_count.saturating_sub(1);
                    }
                    SwarmEvent::Behaviour(TestClientBehaviourEvent::Identify(IdentifyEvent::Received { peer_id, info, .. })) => {
                        println!("🔍 Identified peer: {}", peer_id);
                        println!("   Protocol version: {}", info.protocol_version);
                        println!("   Agent version: {}", info.agent_version);
                        println!("   Listen addresses: {:?}", info.listen_addrs);
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        println!("❌ Outgoing connection error to {:?}: {}", peer_id, error);
                    }
                    SwarmEvent::IncomingConnectionError { error, .. } => {
                        println!("❌ Incoming connection error: {}", error);
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                println!("\n⏰ Timeout reached");
                break;
            }
        }
    }

    // Summary
    println!("\n📊 Connection Test Summary");
    println!("========================");
    println!("🔗 Connected: {}", if connected { "✅ Yes" } else { "❌ No" });
    println!("👥 Active peers: {}", peer_count);
    
    if connected {
        println!("✅ SUCCESS: Successfully connected to the relay!");
        println!("💡 The relay is reachable and functioning.");
        if peer_count == 0 {
            println!("⚠️  WARNING: Connected to relay but no other peers found.");
            println!("   This might be expected if no other nodes are currently connected.");
        }
    } else {
        println!("❌ FAILURE: Could not connect to the relay.");
        println!("💡 Possible issues:");
        println!("   1. Relay is not running on the specified address");
        println!("   2. Firewall blocking the connection");
        println!("   3. Wrong IP address or port");
        println!("   4. Network connectivity issues");
    }

    Ok(())
} 