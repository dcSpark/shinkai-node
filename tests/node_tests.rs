use async_channel::{Sender, Receiver, bounded};
use chrono::prelude::*;
use chrono_tz::America::Chicago;
use shinkai_node::network::Node;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::shinkai_message::encryption::{ephemeral_keys, unsafe_deterministic_private_key, public_key_to_string, secret_key_to_string};
use shinkai_node::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::shinkai_message_proto::Field;
use std::net::{IpAddr, Ipv4Addr};
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

pub fn print_chicago_time() {
    let utc: DateTime<Utc> = Utc::now();
    let chicago_time: DateTime<chrono_tz::Tz> = utc.with_timezone(&Chicago);
    println!("The current date and time in Chicago is {}", chicago_time);
}

#[test]
fn tcp_node_test() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let (node1_sk, node1_pk) = unsafe_deterministic_private_key(0);
        let (node2_sk, node2_pk) = unsafe_deterministic_private_key(1);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) = bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) = bounded(100);

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(addr1, node1_sk, 0, node1_commands_receiver);

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let node2 = Node::new(addr2, node2_sk, 0, node2_commands_receiver);


        // let addr1_string = &addr1.to_string();
        // let node2_handle = node2.start_and_connect(&addr1_string, node1_pk).await;
        // println!("After connecting: Pinging all from node 2");
        // node2.ping_all().await;

        tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 1");
            node1.start().await;
            println!("Node 1 started");
        });

        tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 2");
            let _ = node2.start().await;

            println!("Node 2 started");

            tokio::time::sleep(Duration::from_secs(1)).await;
            let _ = node2.start();
            // let _ = node2.start_and_connect(&addr1.to_string(), node1_pk);
            tokio::time::sleep(Duration::from_secs(1)).await;
    
            println!("\n");
            node2.ping_all().await;
    
            tokio::time::sleep(Duration::from_secs(5)).await;
            node2.ping_all().await;
        });

        tokio::time::sleep(Duration::from_secs(10)).await;

        // let fields = vec![Field {
        //     name: "field1".to_string(),
        //     r#type: "type1".to_string(),
        // }];

        // let shinkai_msg = ShinkaiMessageBuilder::new(client_sk_clone, node1_pk)
        //     .body("body content".to_string())
        //     .encryption("default".to_string())
        //     .message_schema_type("schema type".to_string(), fields)
        //     .topic("topic_id".to_string(), "channel_id".to_string())
        //     .internal_metadata_content("internal metadata content".to_string())
        //     .external_metadata(node2_pk, "scheduled_time".to_string())
        //     .build();

        // let result = Node::send(&shinkai_msg.unwrap(), (addr1, node1_pk)).await;
        // // check if result has an Error if so print it
        // if let Err(e) = result {
        //     println!("Error sending ShinkaiMessage: {:?}", e);
        // }
        // tokio::time::sleep(Duration::from_secs(2)).await;
        // node2.ping_all().await;

        // let peers = node2.get_peers();
        // println!("Peers: {:?}", peers);

        // tokio::time::sleep(Duration::from_secs(3)).await;

        // Wait for both tasks to complete
        // let _ = tokio::try_join!(node1_handle, node2_handle);
    });
}
