use crate::shinkai_message::{shinkai_message_handler::ShinkaiMessageHandler, encryption::public_key_to_string};
use warp::Filter;

use super::Node;
use std::{sync::Arc, net::SocketAddr};
use tokio::sync::Mutex;

// Shared node between filters
type SharedNode = Arc<Mutex<Node>>;

pub async fn serve(node: Arc<Mutex<Node>>, address: SocketAddr) {
    // let get_peers = warp::get()
    //     .and(warp::path("peers"))
    //     .and(with_node(node.clone()))
    //     .and_then(get_peers_handler);

    let ping_peers = warp::post()
        .and(warp::path("ping"))
        .and(with_node(node.clone()))
        .and_then(ping_peers_handler);

    let get_node_public_key = warp::get()
        .and(warp::path("node_public_key"))
        .and(with_node(node.clone()))
        .and_then(get_node_public_key_handler);

    let send_message = warp::post()
        .and(warp::path("send"))
        .and(warp::body::content_length_limit(1024 * 1024)) // Limit to 1mb
        .and(warp::body::bytes())
        .and(with_node(node.clone()))
        .and_then(send_message_handler);

    // let get_status = warp::get()
    //     .and(warp::path("status"))
    //     .and(with_node(node.clone()))
    //     .and_then(get_status_handler);

    let routes = send_message.or(ping_peers).or(get_node_public_key);
    // .or(get_peers);
    // .or(get_status);

    println!("Node API Listening on {}", address);
    warp::serve(routes).run(address).await;
}

fn with_node(
    node: SharedNode,
) -> impl Filter<Extract = (SharedNode,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || node.clone())
}

// async fn get_peers_handler(node: SharedNode) -> Result<impl warp::Reply, warp::Rejection> {
//     let node = node.lock().await;
//     let peers = node.get_peers(); // Assuming you have this method implemented
//     let peer_list: Vec<_> = peers.into_iter().collect();
//     Ok(warp::reply::json(&peer_list))
// }

async fn ping_peers_handler(node: SharedNode) -> Result<impl warp::Reply, warp::Rejection> {
    let node = node.lock().await;
    node.ping_all().await.unwrap();
    Ok(warp::reply())
}

async fn send_message_handler(
    body: prost::bytes::Bytes,
    node: SharedNode,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node = node.lock().await;
    let msg = ShinkaiMessageHandler::decode_message(body.to_vec()).unwrap();

    // Check if sender is allowed to use full node as a proxy
    // let sender = msg.external_metadata.clone().unwrap().sender;
    // let recipient_pk = string_to_public_key(&sender).unwrap();
    //
    // if !node.is_allowed(&sender) {
    //     return Ok(warp::reply::json(&"Sender not allowed"));
    // }

    node.forward_from_profile(&msg).await.unwrap();
    Ok(warp::reply())
}

async fn get_node_public_key_handler(
    node: SharedNode,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node = node.lock().await;
    let public_key = node.get_public_key().unwrap();
    let public_key_string = public_key_to_string(public_key);
    let response = serde_json::json!({ "result": public_key_string });
    Ok(warp::reply::json(&response))
}

// async fn get_status_handler(node: SharedNode) -> Result<impl warp::Reply, warp::Rejection> {
//     let node = node.lock().await;
//     let status = node.get_status(); // Assuming you have this method implemented
//     Ok(warp::reply::json(&status))
// }
