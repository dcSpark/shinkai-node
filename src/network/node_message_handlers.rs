use crate::{db::ShinkaiDB, managers::IdentityManager, network::Node};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_primitives::{
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_extension::EncryptionStatus,
    },
    shinkai_utils::{
        encryption::{clone_static_secret_key, encryption_public_key_to_string},
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        shinkai_message_builder::{ProfileName, ShinkaiMessageBuilder},
        signatures::{clone_signature_secret_key, signature_public_key_to_string},
    },
};
use std::sync::Arc;
use std::{io, net::SocketAddr};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::node_error::NodeError;

pub enum PingPong {
    Ping,
    Pong,
}

pub async fn handle_based_on_message_content_and_encryption(
    message: ShinkaiMessage,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_address: SocketAddr,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SignatureStaticKey,
    my_node_profile_name: &str,
    maybe_db: Arc<Mutex<ShinkaiDB>>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
) -> Result<(), NodeError> {
    let message_body = message.body.clone();
    let message_content = match &message_body {
        MessageBody::Encrypted(body) => &body.content,
        MessageBody::Unencrypted(body) => match &body.message_data {
            MessageData::Encrypted(data) => &data.content,
            MessageData::Unencrypted(data) => &data.message_raw_content,
        },
    };
    let message_encryption_status = message.clone().get_encryption_status();
    println!(
        "{} > handle_based_on_message_content_and_encryption message: {:?} {:?}",
        receiver_address, message, message_encryption_status
    );

    // TODO: if content body encrypted to the node itself then decrypt it and process it.
    match (message_content.as_str(), message_encryption_status) {
        (_, EncryptionStatus::BodyEncrypted) => {
            handle_default_encryption(
                message,
                sender_encryption_pk,
                sender_address,
                sender_profile_name,
                my_encryption_secret_key,
                my_signature_secret_key,
                my_node_profile_name,
                receiver_address,
                unsafe_sender_address,
                maybe_db,
                maybe_identity_manager,
            )
            .await
        }
        (_, EncryptionStatus::ContentEncrypted) => {
            // TODO: save to db to send the profile when connected
            println!("{} > Content encrypted", receiver_address);
            handle_other_cases(
                sender_encryption_pk,
                sender_address,
                sender_profile_name,
                my_encryption_secret_key,
                my_signature_secret_key,
                my_node_profile_name,
                receiver_address,
                unsafe_sender_address,
                maybe_db,
                maybe_identity_manager,
            )
            .await
        }
        ("Ping", _) => {
            handle_ping(
                sender_address,
                sender_encryption_pk,
                sender_profile_name,
                my_encryption_secret_key,
                my_signature_secret_key,
                my_node_profile_name,
                receiver_address,
                unsafe_sender_address,
                maybe_db,
                maybe_identity_manager,
            )
            .await
        }
        ("ACK", _) => {
            println!("{} > ACK from {:?}", receiver_address, unsafe_sender_address);
            Ok(())
        }
        (_, _) => {
            handle_other_cases(
                sender_encryption_pk,
                sender_address,
                sender_profile_name,
                my_encryption_secret_key,
                my_signature_secret_key,
                my_node_profile_name,
                receiver_address,
                unsafe_sender_address,
                maybe_db,
                maybe_identity_manager,
            )
            .await
        }
    }
}

// All the new helper functions here:
pub fn extract_message(bytes: &[u8], receiver_address: SocketAddr) -> io::Result<ShinkaiMessage> {
    ShinkaiMessage::decode_message_result(bytes.to_vec()).map_err(|_| {
        println!("{} > Failed to decode message.", receiver_address);
        io::Error::new(io::ErrorKind::Other, "Failed to decode message")
    })
}

pub fn verify_message_signature(
    sender_signature_pk: ed25519_dalek::PublicKey,
    message: &ShinkaiMessage,
) -> io::Result<()> {
    match message.verify_outer_layer_signature(&sender_signature_pk) {
        Ok(is_valid) if is_valid => Ok(()),
        Ok(_) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                "Failed to validate message's signature",
            );
            shinkai_log(ShinkaiLogOption::Network, ShinkaiLogLevel::Error, &format!(
                "Sender signature pk: {:?}",
                signature_public_key_to_string(sender_signature_pk)
            ));
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to validate message's signature",
            ))
        }
        Err(_) => {
            println!("Failed to verify signature. Message: {:?}", message);
            println!(
                "Sender signature pk: {:?}",
                signature_public_key_to_string(sender_signature_pk)
            );
            Err(io::Error::new(io::ErrorKind::Other, "Failed to verify signature"))
        }
    }
}

pub async fn handle_ping(
    sender_address: SocketAddr,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SignatureStaticKey,
    my_node_profile_name: &str,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
    maybe_db: Arc<Mutex<ShinkaiDB>>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
) -> Result<(), NodeError> {
    println!("{} > Got ping from {:?}", receiver_address, unsafe_sender_address);
    ping_pong(
        (sender_address, sender_profile_name.clone()),
        PingPong::Pong,
        clone_static_secret_key(my_encryption_secret_key),
        clone_signature_secret_key(my_signature_secret_key),
        sender_encryption_pk,
        my_node_profile_name.to_string(),
        sender_profile_name,
        maybe_db,
        maybe_identity_manager,
    )
    .await
}

pub async fn handle_default_encryption(
    message: ShinkaiMessage,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_address: SocketAddr,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SignatureStaticKey,
    my_node_profile_name: &str,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
    maybe_db: Arc<Mutex<ShinkaiDB>>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
) -> Result<(), NodeError> {
    println!(
        "{} > handle_default_encryption message: {:?}",
        receiver_address, message
    );
    println!(
        "Sender encryption pk: {:?}",
        encryption_public_key_to_string(sender_encryption_pk)
    );
    let decrypted_message_result = message.decrypt_outer_layer(&my_encryption_secret_key, &sender_encryption_pk);
    match decrypted_message_result {
        Ok(_) => {
            println!(
                "{} > Got message from {:?}. Sending ACK",
                receiver_address, unsafe_sender_address
            );
            send_ack(
                (sender_address.clone(), sender_profile_name.clone()),
                clone_static_secret_key(my_encryption_secret_key),
                clone_signature_secret_key(my_signature_secret_key),
                sender_encryption_pk,
                my_node_profile_name.to_string(),
                sender_profile_name,
                maybe_db,
                maybe_identity_manager,
            )
            .await
        }
        Err(_) => {
            println!("handle_default_encryption > Failed to decrypt message.");
            // TODO: send error back?
            Ok(())
        }
    }
}

pub async fn handle_other_cases(
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_address: SocketAddr,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SignatureStaticKey,
    my_node_profile_name: &str,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
    maybe_db: Arc<Mutex<ShinkaiDB>>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
) -> Result<(), NodeError> {
    println!(
        "{} > Got message from {:?}. Sending ACK",
        receiver_address, unsafe_sender_address
    );
    send_ack(
        (sender_address.clone(), sender_profile_name.clone()),
        clone_static_secret_key(my_encryption_secret_key),
        clone_signature_secret_key(my_signature_secret_key),
        sender_encryption_pk,
        my_node_profile_name.to_string(),
        sender_profile_name,
        maybe_db,
        maybe_identity_manager,
    )
    .await
}

pub async fn send_ack(
    peer: (SocketAddr, ProfileName),
    encryption_secret_key: EncryptionStaticKey, // not important for ping pong
    signature_secret_key: SignatureStaticKey,
    receiver_public_key: EncryptionPublicKey, // not important for ping pong
    sender: ProfileName,
    receiver: ProfileName,
    maybe_db: Arc<Mutex<ShinkaiDB>>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
) -> Result<(), NodeError> {
    let msg = ShinkaiMessageBuilder::ack_message(
        clone_static_secret_key(&encryption_secret_key),
        signature_secret_key,
        receiver_public_key,
        sender,
        receiver,
    )
    .unwrap();

    Node::send(
        msg,
        Arc::new(clone_static_secret_key(&encryption_secret_key)),
        peer,
        maybe_db,
        maybe_identity_manager,
        false,
        None,
    );
    Ok(())
}

// Helper struct to encapsulate sender keys
#[derive(Debug)]
pub struct PublicKeyInfo {
    pub address: SocketAddr,
    pub signature_public_key: ed25519_dalek::PublicKey,
    pub encryption_public_key: x25519_dalek::PublicKey,
}

pub async fn ping_pong(
    peer: (SocketAddr, ProfileName),
    ping_or_pong: PingPong,
    encryption_secret_key: EncryptionStaticKey, // not important for ping pong
    signature_secret_key: SignatureStaticKey,
    receiver_public_key: EncryptionPublicKey, // not important for ping pong
    sender: ProfileName,
    receiver: ProfileName,
    maybe_db: Arc<Mutex<ShinkaiDB>>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
) -> Result<(), NodeError> {
    let message = match ping_or_pong {
        PingPong::Ping => "Ping",
        PingPong::Pong => "Pong",
    };

    let msg = ShinkaiMessageBuilder::ping_pong_message(
        message.to_owned(),
        clone_static_secret_key(&encryption_secret_key),
        signature_secret_key,
        receiver_public_key,
        sender,
        receiver,
    )
    .unwrap();
    Node::send(
        msg,
        Arc::new(clone_static_secret_key(&encryption_secret_key)),
        peer,
        maybe_db,
        maybe_identity_manager,
        false,
        None,
    );
    Ok(())
}
