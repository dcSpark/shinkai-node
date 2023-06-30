use libp2p::futures::StreamExt;
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::{Swarm, SwarmEvent, NetworkBehaviour};
use libp2p::{identity, Multiaddr, PeerId};
use std::error::Error;

pub async fn create_node() -> Result<Swarm<Ping>, Box<dyn Error>> {
    let new_key = identity::Keypair::generate_ed25519();
    let new_peer_id = PeerId::from(new_key.public());
    println!("local peer id is: {:?}", new_peer_id);
    
    let transport = libp2p::development_transport(new_key).await?;
    let behaviour = Ping::new(PingConfig::new().with_keep_alive(true));

    let mut swarm = Swarm::new(transport, behaviour, new_peer_id);
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    Ok(swarm)
}
