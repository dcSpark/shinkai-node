use super::node::NodeCommand;
use async_channel::Sender;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shinkai_message_wasm::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_wasm::shinkai_utils::encryption::encryption_public_key_to_string;
use shinkai_message_wasm::shinkai_utils::signatures::signature_public_key_to_string;
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

#[derive(Serialize, Debug, Clone)]
pub struct APIError {
    pub code: u16,
    pub error: String,
    pub message: String,
}

impl APIError {
    fn new(code: StatusCode, error: &str, message: &str) -> Self {
        Self {
            code: code.as_u16(),
            error: error.to_string(),
            message: message.to_string(),
        }
    }
}

impl From<&str> for APIError {
    fn from(error: &str) -> Self {
        APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: error.to_string(),
        }
    }
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
            .and_then(move || ping_all_handler(node_commands_sender.clone()))
    };

    // POST v1/send
    let send_msg = {
        let node_commands_sender = node_commands_sender.clone();
        warp::post()
            .and(warp::path("v1"))
            .and(warp::path("send"))
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| send_msg_handler(node_commands_sender.clone(), message))
    };

    // GET v1/get_peers
    let get_peers = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_peers")
            .and(warp::get())
            .and_then(move || get_peers_handler(node_commands_sender.clone()))
    };

    // POST v1/identity_name_to_external_profile_data
    let identity_name_to_external_profile_data = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "identity_name_to_external_profile_data")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |body: NameToExternalProfileData| {
                identity_name_to_external_profile_data_handler(node_commands_sender.clone(), body)
            })
    };

    // GET v1/get_public_keys
    let get_public_key = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_public_keys")
            .and(warp::get())
            .and_then(move || get_public_key_handler(node_commands_sender.clone()))
    };

    // POST v1/connect
    let connect = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "connect")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |body: ConnectBody| connect_handler(node_commands_sender.clone(), body))
    };

    // TODO: Implement. Admin Only
    // // POST v1/last_messages?limit={number}&offset={key}
    // let get_last_messages = {
    //     let node_commands_sender = node_commands_sender.clone();
    //     warp::path!("v1" / "last_messages_from_inbox")
    //         .and(warp::post())
    //         .and(warp::body::json::<ShinkaiMessage>())
    //         .and_then(move |message: ShinkaiMessage| {
    //             get_last_messages_handler(node_commands_sender.clone(), message)
    //         })
    // };

    // POST v1/last_messages_from_inbox?limit={number}&offset={key}
    let get_last_messages_from_inbox = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "last_messages_from_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_messages_from_inbox_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/last_unread_messages?limit={number}&offset={key}
    let get_last_unread_messages = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "last_unread_messages_from_inbox")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_last_unread_messages_from_inbox_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/get_all_inboxes_for_profile_handler
    let get_all_inboxes_for_profile = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_all_inboxes_for_profile")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                get_all_inboxes_for_profile_handler(node_commands_sender.clone(), message)
            })
    };

    // get_all_inboxes_for_profile_handler

    // POST v1/create_job
    let create_job = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "create_job")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| create_job_handler(node_commands_sender.clone(), message))
    };

    // POST v1/mark_as_read_up_to
    let mark_as_read_up_to = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "mark_as_read_up_to")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| mark_as_read_up_to_handler(node_commands_sender.clone(), message))
    };

    // POST v1/create_registration_code
    let create_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "create_registration_code")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                create_registration_code_handler(node_commands_sender.clone(), message)
            })
    };

    // POST v1/use_registration_code
    let use_registration_code = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "use_registration_code")
            .and(warp::post())
            .and(warp::body::json::<ShinkaiMessage>())
            .and_then(move |message: ShinkaiMessage| {
                use_registration_code_handler(node_commands_sender.clone(), message)
            })
    };

    // GET v1/get_all_subidentities
    let get_all_subidentities = {
        let node_commands_sender = node_commands_sender.clone();
        warp::path!("v1" / "get_all_subidentities")
            .and(warp::get())
            .and_then(move || get_all_subidentities_handler(node_commands_sender.clone()))
    };

    let cors = warp::cors() // build the CORS filter
        .allow_any_origin() // allow requests from any origin
        .allow_methods(vec!["GET", "POST", "OPTIONS"]) // allow GET, POST, and OPTIONS methods
        .allow_headers(vec!["Content-Type"]); // allow the Content-Type header

    let routes = ping_all
        .or(send_msg)
        .or(get_peers)
        .or(identity_name_to_external_profile_data)
        .or(get_public_key)
        .or(connect)
        .or(get_all_inboxes_for_profile)
        .or(get_last_messages_from_inbox)
        .or(get_last_unread_messages)
        .or(create_job)
        .or(mark_as_read_up_to)
        .or(create_registration_code)
        .or(use_registration_code)
        .or(get_all_subidentities)
        .recover(handle_rejection)
        .with(log)
        .with(cors);

    warp::serve(routes).run(address).await;

    println!("Server successfully started at: {}", &address);
}

async fn handle_node_command<T, U>(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
    command: T,
) -> Result<impl warp::Reply, warp::reject::Rejection>
where
    T: FnOnce(Sender<NodeCommand>, ShinkaiMessage, Sender<Result<U, APIError>>) -> NodeCommand,
    U: Serialize,
{
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .clone() // Clone here
        .send(command(node_commands_sender, message, res_sender))
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(message) => Ok(warp::reply::with_status(warp::reply::json(&message), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

async fn ping_all_handler(node_commands_sender: Sender<NodeCommand>) -> Result<impl warp::Reply, warp::Rejection> {
    match node_commands_sender.send(NodeCommand::PingAll).await {
        Ok(_) => Ok(warp::reply::json(&json!({
            "result": "Pinged all nodes successfully"
        }))),
        Err(_) => Ok(warp::reply::json(&json!({
            "error": "Error occurred while pinging all nodes"
        }))),
    }
}

async fn send_msg_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_send_msg_sender, res_send_msg_receiver): (
        async_channel::Sender<Result<(), APIError>>,
        async_channel::Receiver<Result<(), APIError>>,
    ) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::SendOnionizedMessage {
            msg: message,
            res: res_send_msg_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let send_result = res_send_msg_receiver.recv().await.map_err(|_| warp::reject::reject())?;
    if send_result.is_err() {
        return Err(warp::reject::reject());
    }

    let resp = warp::reply::json(&"Message sent successfully");
    Ok(resp)
}

async fn get_peers_handler(node_commands_sender: Sender<NodeCommand>) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::GetPeers(res_sender))
        .await
        .map_err(|_| warp::reject::reject())?; // Send the command to Node
    let peer_addresses = res_receiver.recv().await.unwrap();
    Ok(warp::reply::json(&peer_addresses))
}

async fn identity_name_to_external_profile_data_handler(
    node_commands_sender: Sender<NodeCommand>,
    body: NameToExternalProfileData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::IdentityNameToExternalProfileData {
            name: body.name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let external_profile_data = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;
    Ok(warp::reply::json(&IdentityNameToExternalProfileDataResponse {
        signature_public_key: signature_public_key_to_string(external_profile_data.node_signature_public_key),
        encryption_public_key: encryption_public_key_to_string(external_profile_data.node_encryption_public_key),
    }))
}

async fn get_public_key_handler(
    node_commands_sender: Sender<NodeCommand>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::GetPublicKeys(res_sender))
        .await
        .map_err(|_| warp::reject::reject())?; // Send the command to Node
    let (signature_public_key, encryption_public_key) =
        res_receiver.recv().await.map_err(|_| warp::reject::reject())?;
    let signature_public_key_string = signature_public_key_to_string(signature_public_key.clone());
    let encryption_public_key_string = encryption_public_key_to_string(encryption_public_key.clone());
    Ok(warp::reply::json(&GetPublicKeysResponse {
        signature_public_key: signature_public_key_string,
        encryption_public_key: encryption_public_key_string,
    }))
}

async fn connect_handler(
    node_commands_sender: Sender<NodeCommand>,
    body: ConnectBody,
) -> Result<impl warp::Reply, warp::Rejection> {
    let address: SocketAddr = body.address.parse().expect("Failed to parse SocketAddr");
    let profile_name = body.profile_name.clone();
    let node_commands_sender = node_commands_sender.clone();
    let _ = node_commands_sender
        .send(NodeCommand::Connect {
            address: address.clone(),
            profile_name: profile_name.clone(),
        })
        .await;
    Ok(warp::reply::json(&"OK".to_string()))
}

async fn get_last_messages_from_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |node_commands_sender, message, res_sender| NodeCommand::APIGetLastMessagesFromInbox {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

async fn get_last_unread_messages_from_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |node_commands_sender, message, res_sender| NodeCommand::APIGetLastUnreadMessagesFromInbox {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

async fn get_all_inboxes_for_profile_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |node_commands_sender, message, res_sender| NodeCommand::APIGetAllInboxesForProfile {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

// Some(NodeCommand::APIGetAllInboxesForProfile { msg, res }) => self.api_get_all_inboxes_for_profile(msg, res).await?,

async fn create_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |node_commands_sender, message, res_sender| NodeCommand::APICreateNewJob {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

async fn mark_as_read_up_to_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |node_commands_sender, message, res_sender| NodeCommand::APIMarkAsReadUpTo {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

async fn create_registration_code_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APICreateRegistrationCode {
            msg: message,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?; // Send the command to Node
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(code) => {
            let response = serde_json::json!({ "code": code });
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

async fn use_registration_code_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIUseRegistrationCode {
            msg: message,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?; // Send the command to Node
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(message) => Ok(warp::reply::with_status(warp::reply::json(&message), StatusCode::OK)),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

async fn get_all_subidentities_handler(
    node_commands_sender: Sender<NodeCommand>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);

    node_commands_sender
        .send(NodeCommand::APIGetAllSubidentities { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    match res_receiver.recv().await {
        Ok(subidentities) => Ok(warp::reply::json(&subidentities)),
        Err(_) => Err(warp::reject::reject()),
    }
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, warp::Rejection> {
    if err.is_not_found() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Please check your URL.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::NOT_FOUND))
    } else if let Some(_) = err.find::<warp::filters::body::BodyDeserializeError>() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::BAD_REQUEST,
            "Invalid Body",
            "Please check your JSON body.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::BAD_REQUEST))
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed",
            "Please check your request method.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::METHOD_NOT_ALLOWED))
    } else {
        // Unexpected error, we don't want to expose anything to the user.
        let json = warp::reply::json(&APIError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "An unexpected error occurred. Please try again.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
    }
}
