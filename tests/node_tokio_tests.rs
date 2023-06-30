use async_std::task;
use futures::TryFutureExt;
use shinkai_node::network::Node;
use tokio::sync::Mutex;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

#[test]
fn tcp_node_test() {
    let mut rt = Runtime::new().unwrap();

    rt.block_on(async {
        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(addr1, 1024, 10, 100.0, 5);

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let node2 = Node::new(addr2, 1024, 10, 100.0, 5);
        
        // Give some time for nodes to exchange messages
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let addr2_string = &addr2.to_string();
        let node1_handle = node1.start_and_connect(&addr2_string).await;
        println!("\n");
        node1.ping_all().await;

        // now testing node 2
        println!("\n");
        println!("Pinging all from node 2");
        node2.ping_all().await;
        let addr1_string = &addr1.to_string();
        let node2_handle = node2.start_and_connect(&addr1_string).await;
        println!("After connecting: Pinging all from node 2");
        node2.ping_all().await;

        tokio::spawn(async move {
            print!("\n\n");
            print!("Starting node 1");
            node1.start().await;
            print!("Node 1 started");
        });
        
        node2.ping_all().await;
    });
}
