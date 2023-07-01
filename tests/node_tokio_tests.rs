use async_std::task;
use chrono::prelude::*;
use chrono_tz::America::Chicago;
use futures::TryFutureExt;
use shinkai_node::network::Node;
use shinkai_node::shinkai_message::encryption::ephemeral_keys;
use shinkai_node::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::shinkai_message_proto::Field;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

pub fn print_chicago_time() {
    let utc: DateTime<Utc> = Utc::now();
    let chicago_time: DateTime<chrono_tz::Tz> = utc.with_timezone(&Chicago);
    println!("The current date and time in Chicago is {}", chicago_time);
}

#[test]
fn tcp_node_test() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let (node1_sk, node1_pk) = ephemeral_keys();
        let (node2_sk, node2_pk) = ephemeral_keys();
        let client_sk_clone = node2_sk.clone();

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(addr1, node1_sk, node1_pk, 5);

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let node2 = Node::new(addr2, node2_sk, node2_pk, 5);

        // Give some time for nodes to exchange messages
        tokio::time::sleep(Duration::from_secs(2)).await;

        let addr2_string = &addr2.to_string();
        let node1_handle = node1.start_and_connect(&addr2_string, node2_pk).await;
        println!("\n");
        node1.ping_all().await;

        // now testing node 2
        println!("\n");
        println!("Pinging all from node 2");
        node2.ping_all().await;
        let addr1_string = &addr1.to_string();
        let node2_handle = node2.start_and_connect(&addr1_string, node1_pk).await;
        println!("After connecting: Pinging all from node 2");
        node2.ping_all().await;

        // tokio::spawn(async move {
        //     println!("\n\n");
        //     println!("Starting node 1");
        //     node1.start().await;
        //     println!("Node 1 started");
        // });

        tokio::time::sleep(Duration::from_secs(2)).await;
        node2.ping_all().await;

        let fields = vec![Field {
            name: "field1".to_string(),
            r#type: "type1".to_string(),
        }];

        let shinkai_msg = ShinkaiMessageBuilder::new(client_sk_clone, node1_pk)
            .body("body content".to_string())
            .encryption("default".to_string())
            .message_schema_type("schema type".to_string(), fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata(node2_pk, "scheduled_time".to_string())
            .build();

        let result = node2.send(&shinkai_msg.unwrap(), (addr1, node1_pk)).await;
        // check if result has an Error if so print it
        if let Err(e) = result {
            println!("Error sending ShinkaiMessage: {:?}", e);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        node2.ping_all().await;

        let peers = node2.get_peers();
        println!("Peers: {:?}", peers);

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Wait for both tasks to complete
        // let _ = tokio::try_join!(node1_handle, node2_handle);
    });
}

#[test]
fn get_three_peers_test() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        // Define node1, node2, node3, node4
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080);
        let (node1_sk, node1_pk) = ephemeral_keys();
        let node1 = Node::new(addr1, node1_sk, node1_pk, 5);

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8081);
        let (node2_sk, node2_pk) = ephemeral_keys();
        let node2 = Node::new(addr2, node2_sk, node2_pk, 5);

        let addr3 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8082);
        let (node3_sk, node3_pk) = ephemeral_keys();
        let node3 = Node::new(addr3, node3_sk, node3_pk, 5);

        let addr4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8083);
        let (node4_sk, node4_pk) = ephemeral_keys();
        let node4 = Node::new(addr4, node4_sk, node4_pk, 5);

        // let handler1 = tokio::spawn(async move {
            println!("Starting node4");
            print_chicago_time();

            match node4.start().await {
                Ok(_) => {
                    println!("*** Node4 started ***");
                    print_chicago_time();
                }
                Err(e) => {
                    println!("\n\n:( :( :( Node4 failed to start: {:?} :( :( :(", e);
                    print_chicago_time();
                }
            }

            // Give some time for nodes to exchange messages
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Check if get_peers from node4 returns 3 peers
            let peers = node4.get_peers();
            println!("Peers: {:?}", peers);
            assert_eq!(peers.len(), 3);
        // });

        // let handler2 = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
        //     // Connect node1, node2, and node3 to node4
        //     println!("Node1 trying to connect to Node4");
        //     print_chicago_time();
            // match node1.start_and_connect(&addr4.to_string(), node4_pk).await {
            //     Ok(_) => {
            //         println!("Node1 connected to Node4");
            //         print_chicago_time();                    
            //     }
            //     Err(e) => {
            //         println!("Failed to connect Node1 to Node4: {}", e);
            //         print_chicago_time();
            //     }
            // };

            // tokio::time::sleep(Duration::from_secs(2)).await;
            // println!("Node2 trying to connect to Node4");
            // print_chicago_time();
            // match node2.start_and_connect(&addr4.to_string(), node4_pk).await {
            //     Ok(_) => println!("Node2 connected to Node4"),
            //     Err(e) => println!("Failed to connect Node2 to Node4: {}", e),
            // };

            // match node3.start_and_connect(&addr4.to_string(), node4_pk).await {
            //     Ok(_) => println!("Node3 connected to Node4"),
            //     Err(e) => println!("Failed to connect Node3 to Node4: {}", e),
            // };
        // });

        // let _ = tokio::try_join!(handler1, handler2);
    });
}
