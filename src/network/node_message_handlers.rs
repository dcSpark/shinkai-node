use super::external_identities;
use crate::{
    db::ShinkaiMessageDB,
    network::Node,
    shinkai_message::{
        encryption::{clone_static_secret_key, decrypt_message},
        shinkai_message_builder::{ProfileName, ShinkaiMessageBuilder},
        shinkai_message_handler::ShinkaiMessageHandler,
        signatures::{clone_signature_secret_key, verify_signature},
    },
    shinkai_message_proto::ShinkaiMessage,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use std::sync::Arc;
use std::{io, net::SocketAddr};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub enum PingPong {
    Ping,
    Pong,
}

// All the new helper functions here:
pub fn extract_message(bytes: &[u8], receiver_address: SocketAddr) -> io::Result<ShinkaiMessage> {
    ShinkaiMessageHandler::decode_message(bytes.to_vec()).map_err(|_| {
        println!("{} > Failed to decode message.", receiver_address);
        io::Error::new(io::ErrorKind::Other, "Failed to decode message")
    })
}

pub fn extract_sender_profile_name(message: &ShinkaiMessage) -> String {
    message.external_metadata.clone().unwrap().sender
}

pub fn extract_sender_keys(sender_profile_name: String) -> io::Result<PublicKeyInfo> {
    let identity_pk =
        external_identities::external_identity_to_identity_pk(sender_profile_name.clone()).unwrap();
    Ok(PublicKeyInfo {
        address: identity_pk.addr,
        signature_public_key: identity_pk.signature_public_key,
        encryption_public_key: identity_pk.encryption_public_key,
    })
}

pub fn verify_message_signature(
    sender_signature_pk: ed25519_dalek::PublicKey,
    message: &ShinkaiMessage,
    receiver_address: SocketAddr,
) -> io::Result<()> {
    match verify_signature(&sender_signature_pk.clone(), &message.clone()) {
        Ok(is_valid) if is_valid => Ok(()),
        Ok(_) => {
            println!(
                "{} > Failed to validate message's signature",
                receiver_address
            );
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to validate message's signature",
            ))
        }
        Err(_) => {
            println!("{} > Failed to verify signature.", receiver_address);
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to verify signature",
            ))
        }
    }
}

pub fn extract_message_content_and_encryption(message: &ShinkaiMessage) -> (String, String) {
    (
        message.body.clone().unwrap().content,
        message.encryption.clone(),
    )
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
    maybe_db: Arc<Mutex<ShinkaiMessageDB>>,
) -> io::Result<()> {
    println!(
        "{} > Got ping from {:?}",
        receiver_address, unsafe_sender_address
    );
    let db_lock = maybe_db.lock().await;
    ping_pong(
        (sender_address, sender_profile_name.clone()),
        PingPong::Pong,
        clone_static_secret_key(my_encryption_secret_key),
        clone_signature_secret_key(my_signature_secret_key),
        sender_encryption_pk,
        my_node_profile_name.to_string(),
        sender_profile_name,
        &db_lock,
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
    maybe_db: Arc<Mutex<ShinkaiMessageDB>>,
) -> io::Result<()> {
    let decrypted_message_result = decrypt_message(
        &message.clone(),
        my_encryption_secret_key,
        &sender_encryption_pk,
    );

    match decrypted_message_result {
        Ok(decrypted_message) => {
            let _ = decrypted_message.body.unwrap().content.as_str();
            println!(
                "{} > Got message from {:?}. Sending ACK",
                receiver_address, unsafe_sender_address
            );
            let db_lock = maybe_db.lock().await;
            send_ack(
                (sender_address.clone(), sender_profile_name.clone()),
                clone_static_secret_key(my_encryption_secret_key),
                clone_signature_secret_key(my_signature_secret_key),
                sender_encryption_pk,
                my_node_profile_name.to_string(),
                sender_profile_name,
                &db_lock,
            )
            .await
        },
        Err(_) => {
            println!("Failed to decrypt message.");
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
    maybe_db: Arc<Mutex<ShinkaiMessageDB>>,
) -> io::Result<()> {
    println!(
        "{} > Got message from {:?}. Sending ACK",
        receiver_address, unsafe_sender_address
    );
    let db_lock = maybe_db.lock().await;
    send_ack(
        (sender_address.clone(), sender_profile_name.clone()),
        clone_static_secret_key(my_encryption_secret_key),
        clone_signature_secret_key(my_signature_secret_key),
        sender_encryption_pk,
        my_node_profile_name.to_string(),
        sender_profile_name,
        &db_lock,
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
    db: &ShinkaiMessageDB,
) -> io::Result<()> {
    let msg = ShinkaiMessageBuilder::ack_message(
        encryption_secret_key,
        signature_secret_key,
        receiver_public_key,
        sender,
        receiver,
    )
    .unwrap();

    Node::send(&msg, peer, db).await?;
    Ok(())
}

// Helper struct to encapsulate sender keys
pub struct PublicKeyInfo {
    pub address: SocketAddr,
    pub signature_public_key: ed25519_dalek::PublicKey,
    pub encryption_public_key: x25519_dalek::PublicKey,
}

pub async fn handle_based_on_message_content_and_encryption(
    message: ShinkaiMessage,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_address: SocketAddr,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SignatureStaticKey,
    my_node_profile_name: &str,
    maybe_db: Arc<Mutex<ShinkaiMessageDB>>,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
) -> io::Result<()> {
    let message_body = message.body.clone().unwrap();
    let message_content = message_body.content.as_str();
    let message_encryption = message.encryption.as_str();

    match (message_content, message_encryption) {
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
            )
            .await
        }
        ("ACK", _) => {
            println!(
                "{} > ACK from {:?}",
                receiver_address, unsafe_sender_address
            );
            Ok(())
        }
        (_, "default") => {
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
            )
            .await
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
            )
            .await
        }
    }
}

pub async fn ping_pong(
    peer: (SocketAddr, ProfileName),
    ping_or_pong: PingPong,
    encryption_secret_key: EncryptionStaticKey, // not important for ping pong
    signature_secret_key: SignatureStaticKey,
    receiver_public_key: EncryptionPublicKey, // not important for ping pong
    sender: ProfileName,
    receiver: ProfileName,
    db: &ShinkaiMessageDB,
) -> io::Result<()> {
    let message = match ping_or_pong {
        PingPong::Ping => "Ping",
        PingPong::Pong => "Pong",
    };

    let msg = ShinkaiMessageBuilder::ping_pong_message(
        message.to_owned(),
        encryption_secret_key,
        signature_secret_key,
        receiver_public_key,
        sender,
        receiver,
    )
    .unwrap();
    Node::send(&msg, peer, db).await
}
