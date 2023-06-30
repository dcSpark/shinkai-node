// use futures::FutureExt;
// use libp2p::futures::StreamExt;
// use libp2p::ping::{Ping, PingConfig, PingEvent};
// use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
// use libp2p::{identity, Multiaddr, PeerId};
// use shinkai_node::network::node::create_node;
// use std::error::Error;
// use std::time::Duration;
// use tokio::runtime::Runtime;
// use tokio::time::sleep;

// #[tokio::test]
// async fn it_works() {
//     let mut swarm1 = create_node().await.unwrap();
//     let mut swarm2 = create_node().await.unwrap();

//     // Poll the swarms to allow them to set up listeners
//     swarm1.next().now_or_never();
//     swarm2.next().now_or_never();

//     let swarm1_peer_id = Swarm::local_peer_id(&swarm1).clone();
//     let swarm2_peer_id = Swarm::local_peer_id(&swarm2).clone();

//     let swarm1_listen_addr = Swarm::listeners(&swarm1)
//         .next()
//         .expect("Swarm1 must have at least one listener");

//     let swarm2_listen_addrs = Swarm::listeners(&swarm2)
//         .next()
//         .expect("Swarm2 must have at least one listener")
//         .clone();

//     // Give some time for listener to be up
//     sleep(Duration::from_secs(2)).await;

//     let swarm2_ping_future = tokio::spawn(async move {
//         let _ = Swarm::dial(&mut swarm2, swarm1_listen_addr.clone().into());
//         while let Some(event) = swarm2.next().await {
//             if let SwarmEvent::Behaviour(PingEvent {
//                 peer,
//                 result: Ok(_rtt),
//             }) = event
//             {
//                 assert_eq!(&peer, &swarm1_peer_id);
//                 break;
//             }
//         }
//     });

//     println!("Swarm1 is listening on {:?}", swarm1_listen_addr);
//     println!("Swarm2 is listening on {:?}", swarm2_listen_addrs);

//     while let Some(event) = swarm1.next().await {
//         if let SwarmEvent::Behaviour(PingEvent {
//             peer,
//             result: Ok(_rtt),
//         }) = event
//         {
//             assert_eq!(&peer, &swarm2_peer_id);
//             break;
//         }
//     }

//     swarm2_ping_future.await.unwrap();
// }
