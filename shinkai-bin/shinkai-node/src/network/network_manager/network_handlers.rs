use crate::{
    managers::IdentityManager,
    network::{
        agent_payments_manager::{
            external_agent_offerings_manager::ExtAgentOfferingsManager,
            my_agent_offerings_manager::MyAgentOfferingsManager,
        },
        node::ProxyConnectionInfo,
        Node,
    },
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use shinkai_db::{db::ShinkaiDB, schemas::ws_types::WSUpdateHandler};
use shinkai_message_primitives::{
    schemas::{
        invoices::{Invoice, InvoiceRequest, InvoiceRequestNetworkError},
        shinkai_name::ShinkaiName,
    },
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_error::ShinkaiMessageError,
        shinkai_message_extension::EncryptionStatus,
        shinkai_message_schemas::MessageSchemaType,
    },
    shinkai_utils::{
        encryption::clone_static_secret_key,
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString},
        signatures::{clone_signature_secret_key, signature_public_key_to_string},
    },
};
use std::sync::{Arc, Weak};
use std::{io, net::SocketAddr};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::network_job_manager_error::NetworkJobQueueError;

pub enum PingPong {
    Ping,
    Pong,
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_based_on_message_content_and_encryption(
    message: ShinkaiMessage,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_address: SocketAddr,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SigningKey,
    my_node_profile_name: &str,
    maybe_db: Arc<ShinkaiDB>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
    my_agent_offering_manager: Weak<Mutex<MyAgentOfferingsManager>>,
    external_agent_offering_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
) -> Result<(), NetworkJobQueueError> {
    let message_body = message.body.clone();
    let message_content = match &message_body {
        MessageBody::Encrypted(body) => &body.content,
        MessageBody::Unencrypted(body) => match &body.message_data {
            MessageData::Encrypted(data) => &data.content,
            MessageData::Unencrypted(data) => &data.message_raw_content,
        },
    };
    let message_encryption_status = message.clone().get_encryption_status();
    shinkai_log(
        ShinkaiLogOption::Network,
        ShinkaiLogLevel::Debug,
        &format!(
            "{} > handle_based_on_message_content_and_encryption message: {:?} {:?}",
            receiver_address, message, message_encryption_status
        ),
    );

    // TODO: if content body encrypted to the node itself then decrypt it and process it.
    match (message_content.as_str(), message_encryption_status) {
        (_, EncryptionStatus::BodyEncrypted) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("{} > Body encrypted", receiver_address),
            );
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
                my_agent_offering_manager,
                external_agent_offering_manager,
                proxy_connection_info,
                ws_manager,
            )
            .await
        }
        (_, EncryptionStatus::ContentEncrypted) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("{} {} > Content encrypted", my_node_profile_name, receiver_address),
            );
            handle_network_message_cases(
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
                my_agent_offering_manager,
                external_agent_offering_manager,
                proxy_connection_info,
                ws_manager,
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
                proxy_connection_info,
                ws_manager,
            )
            .await
        }
        ("ACK", _) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!(
                    "{} {} > ACK from {:?}",
                    my_node_profile_name, receiver_address, unsafe_sender_address
                ),
            );
            // Currently, we are not saving ACKs received to the DB.
            Ok(())
        }
        (_, EncryptionStatus::NotCurrentlyEncrypted) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!(
                    "{} {} > Not currently encrypted",
                    my_node_profile_name, receiver_address
                ),
            );
            handle_network_message_cases(
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
                my_agent_offering_manager,
                external_agent_offering_manager,
                proxy_connection_info,
                ws_manager,
            )
            .await
        }
    }
}

// All the new helper functions here:
pub fn extract_message(bytes: &[u8], receiver_address: SocketAddr) -> io::Result<ShinkaiMessage> {
    ShinkaiMessage::decode_message_result(bytes.to_vec()).map_err(|_| {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Error,
            &format!("{} > Failed to decode message.", receiver_address),
        );
        io::Error::new(io::ErrorKind::Other, "Failed to decode message")
    })
}

pub fn verify_message_signature(sender_signature_pk: VerifyingKey, message: &ShinkaiMessage) -> io::Result<()> {
    match message.verify_outer_layer_signature(&sender_signature_pk) {
        Ok(is_valid) if is_valid => Ok(()),
        Ok(_) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                "Failed to validate outer message's signature",
            );
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!(
                    "Sender signature pk: {:?}",
                    signature_public_key_to_string(sender_signature_pk)
                ),
            );
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to validate outer message's signature",
            ))
        }
        Err(_) => {
            eprintln!("Failed to verify signature. Message: {:?}", message);
            eprintln!(
                "Sender signature pk: {:?}",
                signature_public_key_to_string(sender_signature_pk)
            );
            Err(io::Error::new(io::ErrorKind::Other, "Failed to verify signature"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_ping(
    sender_address: SocketAddr,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SigningKey,
    my_node_profile_name: &str,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
    maybe_db: Arc<ShinkaiDB>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
) -> Result<(), NetworkJobQueueError> {
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
        proxy_connection_info,
        ws_manager,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_default_encryption(
    message: ShinkaiMessage,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_address: SocketAddr,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SigningKey,
    my_node_profile_name: &str,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
    maybe_db: Arc<ShinkaiDB>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    my_agent_offering_manager: Weak<Mutex<MyAgentOfferingsManager>>,
    external_agent_offering_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
) -> Result<(), NetworkJobQueueError> {
    let decrypted_message_result = message.decrypt_outer_layer(my_encryption_secret_key, &sender_encryption_pk);
    match decrypted_message_result {
        Ok(decrypted_message) => {
            println!(
                "{} {} > Successfully decrypted message outer layer",
                my_node_profile_name, receiver_address
            );
            let message = decrypted_message.get_message_content();
            match message {
                Ok(message_content) => {
                    if message_content != "ACK" {
                        // Call handle_other_cases after decrypting the payload
                        handle_network_message_cases(
                            decrypted_message,
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
                            my_agent_offering_manager,
                            external_agent_offering_manager,
                            proxy_connection_info,
                            ws_manager.clone(),
                        )
                        .await?;
                    }
                }
                Err(_) => {
                    // Note(Nico): if we can't decrypt the inner content (it's okay). We still send an ACK
                    // it is most likely meant for a profile which we don't have the encryption secret key for.
                    Node::save_to_db(
                        false,
                        &decrypted_message,
                        clone_static_secret_key(my_encryption_secret_key),
                        maybe_db.clone(),
                        maybe_identity_manager.clone(),
                        ws_manager.clone(),
                    )
                    .await?;

                    let _ = send_ack(
                        (sender_address, sender_profile_name.clone()),
                        clone_static_secret_key(my_encryption_secret_key),
                        clone_signature_secret_key(my_signature_secret_key),
                        sender_encryption_pk,
                        my_node_profile_name.to_string(),
                        sender_profile_name,
                        maybe_db,
                        maybe_identity_manager,
                        proxy_connection_info,
                        ws_manager,
                    )
                    .await;
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to decrypt message: {:?}", e);
            eprintln!("Message: {:?}", message);
            println!("handle_default_encryption > Failed to decrypt message.");
            // TODO: send error back?
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_network_message_cases(
    message: ShinkaiMessage,
    sender_encryption_pk: x25519_dalek::PublicKey,
    sender_address: SocketAddr,
    sender_profile_name: String,
    my_encryption_secret_key: &EncryptionStaticKey,
    my_signature_secret_key: &SigningKey,
    my_node_full_name: &str,
    receiver_address: SocketAddr,
    unsafe_sender_address: SocketAddr,
    maybe_db: Arc<ShinkaiDB>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    my_agent_offering_manager: Weak<Mutex<MyAgentOfferingsManager>>,
    external_agent_offering_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
) -> Result<(), NetworkJobQueueError> {
    println!(
        "{} {} > Network Message Got message from {:?}. Processing and sending ACK",
        my_node_full_name, receiver_address, unsafe_sender_address
    );

    let mut message = message.clone();

    // Check if the message is coming from a relay proxy and update it
    // ONLY if our identity is localhost (tradeoff for not having an identity)
    if my_node_full_name.starts_with("@@localhost.") {
        let proxy_connection = proxy_connection_info.lock().await;
        if let Some(proxy_info) = &*proxy_connection {
            if message.external_metadata.sender == proxy_info.proxy_identity.get_node_name_string() {
                match ShinkaiName::new(message.external_metadata.other.clone()) {
                    Ok(origin_identity) => {
                        message.external_metadata.sender = origin_identity.get_node_name_string();
                        if let MessageBody::Unencrypted(ref mut body) = message.body {
                            body.internal_metadata.sender_subidentity =
                                origin_identity.get_profile_name_string().unwrap_or("".to_string());
                        }
                    }
                    Err(e) => {
                        eprintln!("Error creating ShinkaiName: {}", e);
                    }
                }
            }
        }
    }

    // Logic to handle if messages needs to be saved to disk
    let schema_result = message.get_message_content_schema();
    let should_save = matches!(
        schema_result,
        Ok(MessageSchemaType::TextContent)
            | Ok(MessageSchemaType::JobMessageSchema)
            | Ok(MessageSchemaType::SubscribeToSharedFolderResponse)
    ) || matches!(
        schema_result,
        Err(ShinkaiMessageError::InvalidMessageSchemaType(err)) if err == "Message data is encrypted"
    );

    if should_save {
        Node::save_to_db(
            false,
            &message,
            clone_static_secret_key(my_encryption_secret_key),
            maybe_db.clone(),
            maybe_identity_manager.clone(),
            ws_manager.clone(),
        )
        .await?;
    }

    // Add Log about the message content schema
    shinkai_log(
        ShinkaiLogOption::Network,
        ShinkaiLogLevel::Info,
        &format!(
            "{} > Network Message Got message with content schema {:?}",
            my_node_full_name,
            message.get_message_content_schema()
        ),
    );
    println!(
        "message.get_message_content_schema(): {:?}",
        message.get_message_content_schema()
    );

    // Check the schema of the message and decide what to do
    match message.get_message_content_schema() {
        Ok(schema) => {
            match schema {
                MessageSchemaType::InvoiceRequest => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!(
                            "{} > InvoiceRequest from: {:?} to: {:?}",
                            receiver_address, requester, receiver
                        ),
                    );
                    println!("InvoiceRequest Received from: {:?} to {:?}", requester, receiver);

                    let ext_agent_offering_manager = if let Some(manager) = external_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade external_agent_offering_manager",
                        );
                        return Err(NetworkJobQueueError::ManagerUnavailable);
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<InvoiceRequest>(&content) {
                        Ok(invoice_request) => {
                            // Successfully converted, you can now use shared_folder_infos
                            let mut ext_agent_offering_manager = ext_agent_offering_manager.lock().await;
                            let _ = ext_agent_offering_manager
                                .network_request_invoice(requester, invoice_request)
                                .await;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to APIRequestInvoice: {}", e),
                            );
                            eprintln!("Failed to deserialize JSON to APIRequestInvoice: {}", e);
                        }
                    }
                }
                MessageSchemaType::InvoiceRequestNetworkError => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Error,
                        &format!(
                            "InvoiceRequestNetworkError Received from: {:?} to {:?}",
                            requester, receiver
                        ),
                    );

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<InvoiceRequestNetworkError>(&content) {
                        Ok(invoice_request_network_error) => {
                            if let Err(e) = maybe_db.set_invoice_network_error(&invoice_request_network_error) {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to store InvoiceRequestNetworkError in DB: {:?}", e),
                                );
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to InvoiceRequestNetworkError: {}", e),
                            );
                        }
                    }
                }
                MessageSchemaType::Invoice => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > Invoice Received from: {:?} to: {:?}",
                            receiver_address, requester, receiver
                        ),
                    );
                    println!("Invoice Received from: {:?} to {:?}", requester, receiver);

                    let my_agent_offering_manager = if let Some(manager) = my_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade my_agent_offering_manager",
                        );
                        return Err(NetworkJobQueueError::ManagerUnavailable);
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<Invoice>(&content) {
                        Ok(invoice) => {
                            let my_agent_offering_manager = my_agent_offering_manager.lock().await;
                            my_agent_offering_manager.store_invoice(&invoice).await?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to Invoice: {}", e),
                            );
                            eprintln!("Failed to deserialize JSON to Invoice: {}", e);
                        }
                    }
                }
                MessageSchemaType::PaidInvoice => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > PaidInvoice Received from: {:?} to: {:?}",
                            receiver_address, requester, receiver
                        ),
                    );
                    println!("PaidInvoice Received from: {:?} to {:?}", requester, receiver);

                    let ext_agent_offering_manager = if let Some(manager) = external_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade external_agent_offering_manager",
                        );
                        return Err(NetworkJobQueueError::ManagerUnavailable);
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<Invoice>(&content) {
                        Ok(invoice) => {
                            let mut ext_agent_offering_manager = ext_agent_offering_manager.lock().await;
                            ext_agent_offering_manager
                                .network_confirm_invoice_payment_and_process(requester, invoice)
                                .await?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to Invoice: {}", e),
                            );
                            eprintln!("Failed to deserialize JSON to Invoice: {}", e);
                        }
                    }
                }
                MessageSchemaType::InvoiceResult => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > InvoiceResult Received from: {:?} to: {:?}",
                            receiver_address, requester, receiver
                        ),
                    );
                    println!("InvoiceResult Received from: {:?} to {:?}", requester, receiver);

                    let my_agent_offering_manager = if let Some(manager) = my_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade my_agent_offering_manager",
                        );
                        return Err(NetworkJobQueueError::ManagerUnavailable);
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<Invoice>(&content) {
                        Ok(invoice_result) => {
                            println!("Invoice result received: {:?}", invoice_result);
                            let my_agent_offering_manager = my_agent_offering_manager.lock().await;
                            my_agent_offering_manager.store_invoice_result(&invoice_result).await?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to InvoiceResult: {}", e),
                            );
                            eprintln!("Failed to deserialize JSON to InvoiceResult: {}", e);
                        }
                    }
                }
                _ => {
                    // Ignore other schemas
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!("{} > Ignoring other schemas. Schema: {:?}", receiver_address, schema),
                    );
                }
            }
        }
        Err(e) => {
            // Handle error case
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!("{} > Error getting message schema: {:?}", receiver_address, e),
            );
        }
    }

    send_ack(
        (sender_address, sender_profile_name.clone()),
        clone_static_secret_key(my_encryption_secret_key),
        clone_signature_secret_key(my_signature_secret_key),
        sender_encryption_pk,
        my_node_full_name.to_string(),
        sender_profile_name,
        maybe_db,
        maybe_identity_manager,
        proxy_connection_info,
        ws_manager,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn send_ack(
    peer: (SocketAddr, ShinkaiNameString),
    encryption_secret_key: EncryptionStaticKey, // not important for ping pong
    signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey, // not important for ping pong
    sender: ShinkaiNameString,
    receiver: ShinkaiNameString,
    maybe_db: Arc<ShinkaiDB>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
) -> Result<(), NetworkJobQueueError> {
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
        proxy_connection_info,
        maybe_db,
        maybe_identity_manager,
        ws_manager,
        false,
        None,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn ping_pong(
    peer: (SocketAddr, ShinkaiNameString),
    ping_or_pong: PingPong,
    encryption_secret_key: EncryptionStaticKey, // not important for ping pong
    signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey, // not important for ping pong
    sender: ShinkaiNameString,
    receiver: ShinkaiNameString,
    maybe_db: Arc<ShinkaiDB>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
) -> Result<(), NetworkJobQueueError> {
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
        proxy_connection_info,
        maybe_db,
        maybe_identity_manager,
        ws_manager,
        false,
        None,
    );
    Ok(())
}
