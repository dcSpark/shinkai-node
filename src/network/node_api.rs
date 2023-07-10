use crate::shinkai_message::encryption::{encryption_public_key_to_string, decrypt_body_message};
use crate::shinkai_message::json_serde_shinkai_message::JSONSerdeShinkaiMessage;
use crate::shinkai_message::shinkai_message_extension::ShinkaiMessageWrapper;
use crate::shinkai_message::signatures::signature_public_key_to_string;
use crate::shinkai_message_proto::ShinkaiMessage;

use super::node::NodeCommand;
use async_channel::Sender;
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use warp::Filter;

#[derive(serde::Deserialize)]
struct NameToExternalProfileData {
    name: String,
}

#[derive(serde::Serialize)]
struct GetPublicKeysResponse {
    signature_public_key: String,
    encryption_public_key: String,
}

#[derive(serde::Serialize)]
struct IdentityNameToExternalProfileDataResponse {
    signature_public_key: String,
    encryption_public_key: String,
}

#[derive(serde::Deserialize)]
struct ConnectBody {
    address: String,
    profile_name: String,
}

#[derive(serde::Deserialize)]
struct UseRegistrationCodeBody {
    code: String,
    profile_name: String,
    identity_pk: String,
    encryption_pk: String,
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
            .and(warp::body::json::<ShinkaiMessageWrapper>())
            .and_then(move |message: ShinkaiMessageWrapper| {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let msg = ShinkaiMessage::from(message); // Convert wrapper back to ShinkaiMessage
                    node_commands_sender
                        .send(NodeCommand::SendOnionizedMessage { msg })
                        .await
                        .unwrap();
                    let resp = warp::reply::json(&"Message sent successfully");
                    Ok::<_, warp::Rejection>(resp)
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

    // POST v1/identity_name_to_external_profile_data
    let identity_name_to_external_profile_data = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "identity_name_to_external_profile_data")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |body: NameToExternalProfileData| {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node_commands_sender
                        .send(NodeCommand::IdentityNameToExternalProfileData {
                            name: body.name,
                            res: res_sender,
                        })
                        .await
                        .unwrap();
                    let external_profile_data = res_receiver.recv().await.unwrap();
                    Ok::<_, warp::Rejection>(warp::reply::json(
                        &IdentityNameToExternalProfileDataResponse {
                            signature_public_key: signature_public_key_to_string(
                                external_profile_data.signature_public_key,
                            ),
                            encryption_public_key: encryption_public_key_to_string(
                                external_profile_data.encryption_public_key,
                            ),
                        },
                    ))
                }
            })
    };

    // GET v1/get_public_keys
    let get_public_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_public_keys")
            .and(warp::get())
            .and_then(move || {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node_commands_sender
                        .send(NodeCommand::GetPublicKeys(res_sender))
                        .await
                        .map_err(|_| warp::reject())?;
                    let (signature_public_key, encryption_public_key) =
                        res_receiver.recv().await.map_err(|_| warp::reject())?;
                    let signature_public_key_string =
                        signature_public_key_to_string(signature_public_key.clone());
                    let encryption_public_key_string =
                        encryption_public_key_to_string(encryption_public_key.clone());
                    Ok::<_, warp::Rejection>(warp::reply::json(&GetPublicKeysResponse {
                        signature_public_key: signature_public_key_string,
                        encryption_public_key: encryption_public_key_string,
                    }))
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
                let profile_name = body.profile_name.clone();
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let _ = node_commands_sender
                        .send(NodeCommand::Connect {
                            address: address.clone(),
                            profile_name: profile_name.clone(),
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

    // POST v1/create_registration_code
    let create_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "create_registration_code")
            .and(warp::post())
            .and_then(move || {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node_commands_sender
                        .send(NodeCommand::CreateRegistrationCode { res: res_sender })
                        .await
                        .map_err(|_| warp::reject())?;
                    let code = res_receiver.recv().await.map_err(|_| warp::reject())?;
                    let response = serde_json::json!({ "code": code });
                    Ok::<_, warp::Rejection>(warp::reply::json(&response))
                }
            })
    };

    // POST v1/use_registration_code
    let use_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "use_registration_code")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessageWrapper>())
            .and_then(move |message_wrapper: ShinkaiMessageWrapper| {
                let node_commands_sender = node_commands_sender.clone();
                async move {
                    let msg = ShinkaiMessage::from(message_wrapper);

                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node_commands_sender
                        .send(NodeCommand::UseRegistrationCode {
                            msg,
                            res: res_sender,
                        })
                        .await
                        .map_err(|_| warp::reject())?;
                    let result = res_receiver.recv().await.map_err(|_| warp::reject())?;
                    Ok::<_, warp::Rejection>(warp::reply::json(&result))
                }
            })
    };

    let routes = ping_all
        .or(send_msg)
        .or(get_peers)
        .or(identity_name_to_external_profile_data)
        .or(get_public_key)
        .or(connect)
        .or(get_last_messages)
        .or(create_registration_code)
        .or(use_registration_code)
        .with(log);
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
