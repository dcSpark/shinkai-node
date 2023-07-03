use async_channel::{bounded, Receiver, Sender};
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use shinkai_node::shinkai_message::encryption::{
    hash_public_key, public_key_to_string,
    unsafe_deterministic_private_key,
};
use shinkai_node::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
}

#[test]
fn tcp_node_test() {
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let (node1_sk, node1_pk) = unsafe_deterministic_private_key(0);
        let (node2_sk, node2_pk) = unsafe_deterministic_private_key(1);

        let (node1_commands_sender, node1_commands_receiver): (
            Sender<NodeCommand>,
            Receiver<NodeCommand>,
        ) = bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (
            Sender<NodeCommand>,
            Receiver<NodeCommand>,
        ) = bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_public_key(node1_pk.clone()));
        let node2_db_path = format!("db_tests/{}", hash_public_key(node2_pk.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut node1 = Node::new(addr1, node1_sk, 0, node1_commands_receiver, node1_db_path);

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let mut node2 = Node::new(addr2, node2_sk, 0, node2_commands_receiver, node2_db_path);

        println!("Starting nodes");
        // Start node1 and node2
        let node1_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 1");
            let _ = node1.start().await;
        });

        let node2_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 2");
            let _ = node2.start().await;
        });

        let interactions_handler = tokio::spawn(async move {
            println!("Starting interactions");
            println!("Connecting node 2 to node 1");
            tokio::time::sleep(Duration::from_secs(3)).await;
            node2_commands_sender
                .send(NodeCommand::Connect {
                    address: addr1,
                    pk: public_key_to_string(node1_pk),
                })
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_secs(4)).await;
            // Get Node2 messages
            let (res2_sender, res2_receiver) = async_channel::bounded(1);
            node2_commands_sender
                .send(NodeCommand::FetchLastMessages {
                    limit: 5,
                    res: res2_sender,
                })
                .await
                .unwrap();
            let node2_last_messages = res2_receiver.recv().await.unwrap();

            // Get Node1 messages
            let (res1_sender, res1_receiver) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::FetchLastMessages {
                    limit: 5,
                    res: res1_sender,
                })
                .await
                .unwrap();
            let node1_last_messages = res1_receiver.recv().await.unwrap();

            println!("Node 1 last messages: {:?}", node1_last_messages);
            println!("Node 2 last messages: {:?}", node2_last_messages);

            assert_eq!(node1_last_messages.len(), 3, "Node 1 (listening) should have 3 message");
            assert_eq!(node2_last_messages.len(), 3, "Node 2 (connecting) should have 3 messages");

            // Node 1 (receiving the Ping, sending back a Pong)
            assert_eq!(
                node1_last_messages[1].body.as_ref().unwrap().content == "Pong".to_string(),
                true,
            );
            assert_eq!(
                node1_last_messages[1].external_metadata.as_ref().unwrap().sender == public_key_to_string(node1_pk.clone()),
                true
            );
            assert_eq!(
                node1_last_messages[1].external_metadata.as_ref().unwrap().recipient == public_key_to_string(node2_pk.clone()),
                true
            );

            // Node 2 (sending the Ping, Receiving a Pong and confirming with ACK)
            assert_eq!(
                node2_last_messages[0].body.as_ref().unwrap().content == "ACK".to_string(),
                true
            );
            assert_eq!(
                node2_last_messages[0].external_metadata.as_ref().unwrap().sender == public_key_to_string(node2_pk.clone()),
                true
            );
            assert_eq!(
                node2_last_messages[0].external_metadata.as_ref().unwrap().recipient == public_key_to_string(node1_pk.clone()),
                true
            ); 
            assert_eq!(
                node2_last_messages[2].body.as_ref().unwrap().content == "Ping".to_string(),
                true
            );
            assert_eq!(
                node2_last_messages[2].external_metadata.as_ref().unwrap().sender == public_key_to_string(node2_pk.clone()),
                true
            );
            assert_eq!(
                node2_last_messages[2].external_metadata.as_ref().unwrap().recipient == public_key_to_string(node1_pk.clone()),
                true
            );

            // Messages should be equal
            assert_eq!(
                ShinkaiMessageHandler::calculate_hash(&node1_last_messages[0]),
                ShinkaiMessageHandler::calculate_hash(&node2_last_messages[0])
            );
            assert_eq!(
                ShinkaiMessageHandler::calculate_hash(&node1_last_messages[1]),
                ShinkaiMessageHandler::calculate_hash(&node2_last_messages[1])
            );
            assert_eq!(
                ShinkaiMessageHandler::calculate_hash(&node1_last_messages[2]),
                ShinkaiMessageHandler::calculate_hash(&node2_last_messages[2])
            );
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, node2_handler, interactions_handler);
    });
}
