use libp2p::{
    futures::StreamExt,
    identify::{self, Event as IdentifyEvent},
    noise, ping,
    swarm::{NetworkBehaviour, SwarmEvent, Config},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use shinkai_libp2p_relayer::RelayMessage;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use std::time::Duration;
use tokio::io::{self, AsyncBufReadExt};

#[derive(NetworkBehaviour)]
struct ClientBehaviour {
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting LibP2P Relay Client Example");

    // Generate a keypair for this client
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    // Create transport
    let transport = tcp::tokio::Transport::new(tcp::Config::default())
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&local_key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Configure identify
    let identify = identify::Behaviour::new(identify::Config::new(
        "/shinkai-client/1.0.0".to_string(),
        local_key.public(),
    ));

    // Configure ping
    let ping = ping::Behaviour::new(ping::Config::new());

    // Create behaviour
    let behaviour = ClientBehaviour {
        identify,
        ping,
    };

    // Create swarm with proper configuration
    let mut swarm = Swarm::new(transport, behaviour, local_peer_id, Config::with_tokio_executor());

    // Connect to relay server
    let relay_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080"
        .parse()
        .expect("Valid multiaddr");

    println!("Connecting to relay at: {}", relay_addr);
    swarm.dial(relay_addr)?;

    println!("Client PeerId: {}", local_peer_id);

    // Start interactive session
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin).lines();

    println!("\nCommands:");
    println!("  register <identity> - Register with the relay");
    println!("  send <target> <message> - Send a message to target identity");
    println!("  quit - Exit the client");
    print!("> ");

    loop {
        tokio::select! {
            // Handle swarm events
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {}", address);
                    }
                    SwarmEvent::Behaviour(ClientBehaviourEvent::Identify(IdentifyEvent::Received {
                        peer_id,
                        info,
                        ..
                    })) => {
                        println!("Identified peer: {} with protocol: {}", peer_id, info.protocol_version);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        println!("Connected to peer: {}", peer_id);
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        println!("Disconnected from peer: {} (cause: {:?})", peer_id, cause);
                    }
                    _ => {}
                }
                print!("> ");
            }
        }
    }
} 