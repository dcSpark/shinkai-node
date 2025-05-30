use libp2p::{
    futures::StreamExt,
    gossipsub::{self, Event as GossipsubEvent, MessageAuthenticity, ValidationMode},
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
    gossipsub: gossipsub::Behaviour,
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

    // Configure gossipsub
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(ValidationMode::Strict)
        .build()?;

    let gossipsub = gossipsub::Behaviour::new(
        MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )?;

    // Configure identify
    let identify = identify::Behaviour::new(identify::Config::new(
        "/shinkai-client/1.0.0".to_string(),
        local_key.public(),
    ));

    // Configure ping
    let ping = ping::Behaviour::new(ping::Config::new());

    // Create behaviour
    let behaviour = ClientBehaviour {
        gossipsub,
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

    // Subscribe to relevant topics
    let registration_topic = gossipsub::IdentTopic::new("shinkai-relay-general");
    let direct_topic = gossipsub::IdentTopic::new(format!("shinkai-direct-{}", local_peer_id));
    
    swarm.behaviour_mut().gossipsub.subscribe(&registration_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&direct_topic)?;

    println!("Client PeerId: {}", local_peer_id);
    println!("Subscribed to topics: shinkai-relay-general, shinkai-direct-{}", local_peer_id);

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
                    SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(GossipsubEvent::Message {
                        propagation_source,
                        message,
                        ..
                    })) => {
                        println!("Received message from {}: {:?}", 
                                propagation_source, 
                                String::from_utf8_lossy(&message.data));
                        
                        // Try to parse as RelayMessage
                        if let Ok(relay_msg) = RelayMessage::from_bytes(&message.data) {
                            println!("  -> Relay message from: {}", relay_msg.identity);
                            println!("  -> Message type: {:?}", relay_msg.message_type);
                            if relay_msg.message_type == NetworkMessageType::ShinkaiMessage {
                                if let Ok(shinkai_msg) = relay_msg.extract_shinkai_message() {
                                    println!("  -> Shinkai message content: {:?}", shinkai_msg.get_message_content());
                                }
                            }
                        }
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
            
            // Handle user input
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let parts: Vec<&str> = line.trim().split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }

                        match parts[0] {
                            "register" => {
                                if parts.len() < 2 {
                                    println!("Usage: register <identity>");
                                    continue;
                                }
                                let identity = parts[1].to_string();
                                let relay_msg = RelayMessage::new_proxy_message(identity);
                                let data = relay_msg.to_bytes()?;
                                
                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(registration_topic.clone(), data) {
                                    println!("Failed to send registration: {:?}", e);
                                } else {
                                    println!("Sent registration for identity: {}", parts[1]);
                                }
                            }
                            "send" => {
                                if parts.len() < 3 {
                                    println!("Usage: send <target> <message>");
                                    continue;
                                }
                                let target = parts[1].to_string();
                                let message_content = parts[2..].join(" ");
                                
                                // Create a mock Shinkai message (in real usage, this would be a proper ShinkaiMessage)
                                println!("Sending message '{}' to target '{}'", message_content, target);
                                // For this example, we'll just send a simple text message via gossipsub
                                let topic = gossipsub::IdentTopic::new(format!("shinkai-relay-{}", target));
                                let _ = swarm.behaviour_mut().gossipsub.subscribe(&topic);
                                
                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, message_content.as_bytes()) {
                                    println!("Failed to send message: {:?}", e);
                                } else {
                                    println!("Message sent!");
                                }
                            }
                            "quit" => {
                                println!("Exiting...");
                                break;
                            }
                            _ => {
                                println!("Unknown command. Available commands: register, send, quit");
                            }
                        }
                    }
                    Ok(None) => break, // EOF
                    Err(e) => {
                        println!("Error reading input: {}", e);
                        break;
                    }
                }
                print!("> ");
            }
        }
    }

    Ok(())
} 