use crate::shinkai_message::encryption::{public_key_to_string, string_to_public_key};
use crate::shinkai_message::json_serde_shinkai_message::JSONSerdeShinkaiMessage;
use crate::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use crate::shinkai_message_proto::ShinkaiMessage;

use super::node::NodeCommand;
use async_channel::Sender;
use prost::bytes;
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use warp::http::StatusCode;
use warp::Filter;

#[derive(serde::Deserialize)]
struct PkToAddressBody {
    pk: String,
}

#[derive(serde::Serialize)]
struct PkToAddressResponse {
    result: String,
}

#[derive(serde::Deserialize)]
struct ConnectBody {
    address: String,
    pk: String,
}

pub async fn run_api(node_commands_sender: Sender<NodeCommand>, address: SocketAddr) {
    println!("Starting Node API server at: {}", &address);

    let log = warp::log::custom(|info| {
        eprintln!(
            "ip: {:?}, method: {:?}, path: {:?}, status: {:?}, elapsed: {:?}",
            info.remote_addr(),
            info.method(),
            info.path(),
            info.status(),
            info.elapsed(),
        );
    });

    let ping_all = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "ping_all")
            .and(warp::post())
            .and_then(move || handle_ping_all(node_commands_sender.clone()))
    };

    // POST v1/send
    let send_msg = {
        let node_commands_sender = node_commands_sender.clone();
        warp::post()
            .and(warp::path("v1"))
            .and(warp::path("send"))
            .and(warp::body::bytes())
            .and_then(move |bytes: bytes::Bytes| {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let bytes_vec = bytes.to_vec();
                    match ShinkaiMessageHandler::decode_message(bytes_vec) {
                        Ok(message) => {
                            node_commands_sender
                                .send(NodeCommand::SendMessage { msg: message })
                                .await
                                .unwrap();
                            let resp = warp::reply::json(&"Message sent successfully");
                            Ok::<_, warp::Rejection>(resp)
                        }
                        Err(_) => {
                            let resp = warp::reply::json(&"Error decoding message");
                            Ok::<_, warp::Rejection>(resp)
                        }
                    }
                }
            })
    };

    // GET v1/get_peers
    let get_peers = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_peers").and(warp::get()).and_then({
            let node_commands_sender = node_commands_sender.clone();
            move || {
                let (res_sender, res_receiver) = async_channel::bounded(1);
                let node_commands_sender_clone = node_commands_sender.clone();
                async move {
                    node_commands_sender_clone
                        .send(NodeCommand::GetPeers(res_sender))
                        .await
                        .map_err(|_| warp::reject::reject())?; // Send the command to Node
                    let peer_addresses = res_receiver.recv().await.unwrap();
                    Ok::<_, warp::Rejection>(warp::reply::json(&peer_addresses))
                }
            }
        })
    };

    // POST v1/pk_to_address
    let pk_to_address = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "pk_to_address")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |body: PkToAddressBody| {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node_commands_sender
                        .send(NodeCommand::PkToAddress {
                            pk: body.pk,
                            res: res_sender,
                        })
                        .await
                        .unwrap();
                    let address = res_receiver.recv().await.unwrap();
                    Ok::<_, warp::Rejection>(warp::reply::json(&PkToAddressResponse {
                        result: address.to_string(),
                    }))
                }
            })
    };

    // GET v1/get_public_key
    let get_public_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_public_key")
            .and(warp::get())
            .and_then(move || {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node_commands_sender
                        .send(NodeCommand::GetPublicKey(res_sender))
                        .await
                        .map_err(|_| warp::reject())?;
                    let public_key = res_receiver.recv().await.map_err(|_| warp::reject())?;
                    let public_key_string = public_key_to_string(public_key.clone());
                    Ok::<_, warp::Rejection>(warp::reply::json(&public_key_string))
                }
            })
    };

    // POST v1/connect
    let connect = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "connect")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |body: ConnectBody| {
                let address: SocketAddr = body.address.parse().expect("Failed to parse SocketAddr");
                let pk = body.pk.clone();
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let _ = node_commands_sender
                        .send(NodeCommand::Connect {
                            address: address.clone(),
                            pk: pk.clone(),
                        })
                        .await;

                    Ok::<_, warp::Rejection>(warp::reply::json(&"OK".to_string()))
                }
            })
    };

    // GET v1/last_messages?limit={number}
    let get_last_messages = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "last_messages")
            .and(warp::get())
            .and(warp::query::<HashMap<String, usize>>())
            .and_then(move |params: HashMap<String, usize>| {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let limit = *params.get("limit").unwrap_or(&10); // Default to 10 if limit is not specified
                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit,
                            res: res_sender,
                        })
                        .await
                        .map_err(|_| warp::reject::reject())?;
                    let messages: Vec<ShinkaiMessage> = res_receiver.recv().await.unwrap();
                    let messages: Vec<JSONSerdeShinkaiMessage> = messages
                        .into_iter()
                        .map(JSONSerdeShinkaiMessage::new)
                        .collect();
                    Ok::<_, warp::Rejection>(warp::reply::json(&messages))
                }
            })
    };

    // POST v1/forward_from_profile
    // let forward_from_profile = {
    //     let node_commands_sender = node_commands_sender.clone();
    //     warp::path!("v1" / "forward_from_profile")
    //         .and(warp::post())
    //         .and(warp::body::json())
    //         .and_then(move |msg: ShinkaiMessage| {
    //             node_commands_sender
    //                 .send(NodeCommand::ForwardFromProfile { msg }) // This command would need to be implemented
    //                 .map(|_| ())
    //                 .map_err(warp::reject::any)
    //         })
    //         .map(|_| warp::reply())
    // };

    let routes = ping_all
        .or(send_msg)
        .or(get_peers)
        .or(pk_to_address)
        .or(get_public_key)
        .or(connect)
        .or(get_last_messages)
        .with(log);
    // .or(forward_from_profile);
    warp::serve(routes).run(address).await;

    println!("Server successfully started at: {}", &address);
}

async fn handle_ping_all(
    node_commands_sender: Sender<NodeCommand>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match node_commands_sender.send(NodeCommand::PingAll).await {
        Ok(_) => Ok(warp::reply::json(&json!({
            "result": "Pinged all nodes successfully"
        }))),
        Err(_) => Ok(warp::reply::json(&json!({
            "error": "Error occurred while pinging all nodes"
        }))),
    }
}
