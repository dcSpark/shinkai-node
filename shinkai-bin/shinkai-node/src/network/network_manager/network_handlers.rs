use crate::{
    managers::IdentityManager, network::{
        agent_payments_manager::{
            external_agent_offerings_manager::ExtAgentOfferingsManager, my_agent_offerings_manager::MyAgentOfferingsManager
        }, libp2p_manager::NetworkEvent, node::ProxyConnectionInfo, Node
    }
};
use ed25519_dalek::{SigningKey, VerifyingKey};

use libp2p::{request_response::ResponseChannel, PeerId};
use serde_json::json;
use serde_json::Value;
use shinkai_message_primitives::schemas::agent_network_offering::{
    AgentNetworkOfferingRequest, AgentNetworkOfferingResponse
};
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::{
    schemas::{
        invoices::{Invoice, InvoiceRequest, InvoiceRequestNetworkError}, shinkai_name::ShinkaiName
    }, shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage}, shinkai_message_error::ShinkaiMessageError, shinkai_message_extension::EncryptionStatus, shinkai_message_schemas::MessageSchemaType
    }, shinkai_utils::{
        encryption::clone_static_secret_key, shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption}, shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString}, signatures::{clone_signature_secret_key, signature_public_key_to_string}
    }
};
use shinkai_sqlite::SqliteManager;
use std::sync::{Arc, Weak};
use std::{io, net::SocketAddr};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub enum PingPong {
    Ping,
    Pong,
}

/// Helper function to get the tracing ID for an invoice
/// Uses parent_message_id if it exists, otherwise falls back to invoice_id
async fn get_invoice_tracing_id(maybe_db: &Arc<SqliteManager>, invoice_id: &str) -> String {
    match maybe_db.get_invoice(invoice_id) {
        Ok(invoice) => invoice.parent_message_id.unwrap_or_else(|| invoice_id.to_string()),
        Err(_) => {
            // If we can't fetch the invoice, fall back to using invoice_id
            invoice_id.to_string()
        }
    }
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
    maybe_db: Arc<SqliteManager>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    receiver_address: SocketAddr,
    sender_peer_id: PeerId,
    my_agent_offering_manager: Weak<Mutex<MyAgentOfferingsManager>>,
    external_agent_offering_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    channel: Option<ResponseChannel<ShinkaiMessage>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            eprintln!("🔑 Body encrypted, message: {:?}", message);
            handle_default_encryption(
                message,
                sender_encryption_pk,
                sender_address,
                sender_profile_name,
                my_encryption_secret_key,
                my_signature_secret_key,
                my_node_profile_name,
                receiver_address,
                sender_peer_id,
                maybe_db,
                maybe_identity_manager,
                my_agent_offering_manager,
                external_agent_offering_manager,
                proxy_connection_info,
                ws_manager,
                libp2p_event_sender,
                channel,
            )
            .await
        }
        (_, EncryptionStatus::ContentEncrypted) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!("{} {} > Content encrypted", my_node_profile_name, receiver_address),
            );
            eprintln!("🔑 Content encrypted, message: {:?}", message);
            handle_network_message_cases(
                message,
                sender_encryption_pk,
                sender_address,
                sender_profile_name,
                my_encryption_secret_key,
                my_signature_secret_key,
                my_node_profile_name,
                receiver_address,
                sender_peer_id,
                maybe_db,
                maybe_identity_manager,
                my_agent_offering_manager,
                external_agent_offering_manager,
                proxy_connection_info,
                ws_manager,
                libp2p_event_sender,
                channel,
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
                sender_peer_id,
                maybe_db,
                maybe_identity_manager,
                proxy_connection_info,
                ws_manager,
                libp2p_event_sender,
            )
            .await
        }
        ("ACK", _) => {
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Debug,
                &format!(
                    "{} {} > ACK from {:?}",
                    my_node_profile_name, receiver_address, sender_peer_id
                ),
            );
            println!("ACK received from {:?}", sender_peer_id);
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
                sender_peer_id,
                maybe_db,
                maybe_identity_manager,
                my_agent_offering_manager,
                external_agent_offering_manager,
                proxy_connection_info,
                ws_manager,
                libp2p_event_sender,
                channel,
            )
            .await
        }
    }
}

// All the new helper functions here:
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
    sender_peer_id: PeerId,
    maybe_db: Arc<SqliteManager>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{} > Got ping from {:?}", receiver_address, sender_peer_id);
    shinkai_log(
        ShinkaiLogOption::Network,
        ShinkaiLogLevel::Debug,
        &format!(
            "{} {} > Ping from {:?}",
            my_node_profile_name, receiver_address, sender_peer_id
        ),
    );
    ping_pong(
        (sender_address, sender_profile_name.clone()),
        PingPong::Pong,
        my_encryption_secret_key.clone(),
        my_signature_secret_key.clone(),
        sender_encryption_pk,
        my_node_profile_name.to_string(),
        sender_profile_name,
        maybe_db,
        maybe_identity_manager,
        proxy_connection_info,
        ws_manager,
        libp2p_event_sender,
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
    sender_peer_id: PeerId,
    maybe_db: Arc<SqliteManager>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    my_agent_offering_manager: Weak<Mutex<MyAgentOfferingsManager>>,
    external_agent_offering_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    channel: Option<ResponseChannel<ShinkaiMessage>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let decrypted_message_result = message.decrypt_outer_layer(my_encryption_secret_key, &sender_encryption_pk);
    match decrypted_message_result {
        Ok(decrypted_message) => {
            println!(
                "{} {} > 🔑 Successfully decrypted message outer layer",
                my_node_profile_name, receiver_address
            );
            let shinkai_message = if decrypted_message.is_content_currently_encrypted() {
                match decrypted_message.decrypt_inner_layer(my_encryption_secret_key, &sender_encryption_pk) {
                    Ok(decrypted_message_inner) => {
                        eprintln!("🔑 Decrypted inner layer: {:?}", decrypted_message_inner);
                        decrypted_message_inner
                    }
                    Err(e) => {
                        eprintln!("🔑 Failed to decrypt message inner layer: {:?}", e);
                        decrypted_message
                    }
                }
            } else {
                decrypted_message
            };
            let message = shinkai_message.get_message_content();
            match message {
                Ok(message_content) => {
                    if message_content != "ACK" {
                        // Call handle_other_cases after decrypting the payload
                        handle_network_message_cases(
                            shinkai_message,
                            sender_encryption_pk,
                            sender_address,
                            sender_profile_name,
                            my_encryption_secret_key,
                            my_signature_secret_key,
                            my_node_profile_name,
                            receiver_address,
                            sender_peer_id,
                            maybe_db,
                            maybe_identity_manager,
                            my_agent_offering_manager,
                            external_agent_offering_manager,
                            proxy_connection_info,
                            ws_manager.clone(),
                            libp2p_event_sender,
                            channel,
                        )
                        .await?;
                    }
                }
                Err(_) => {
                    // Note(Nico): if we can't decrypt the inner content (it's okay). We still send an ACK
                    // it is most likely meant for a profile which we don't have the encryption secret key for.
                    Node::save_to_db(
                        false,
                        &shinkai_message,
                        clone_static_secret_key(my_encryption_secret_key),
                        maybe_db.clone(),
                        maybe_identity_manager.clone(),
                        ws_manager.clone(),
                    )
                    .await?;

                    let _ = send_ack(
                        clone_static_secret_key(my_encryption_secret_key),
                        clone_signature_secret_key(my_signature_secret_key),
                        sender_encryption_pk,
                        my_node_profile_name.to_string(),
                        sender_profile_name,
                        libp2p_event_sender,
                        channel,
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
    sender_peer_id: PeerId,
    maybe_db: Arc<SqliteManager>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    my_agent_offering_manager: Weak<Mutex<MyAgentOfferingsManager>>,
    external_agent_offering_manager: Weak<Mutex<ExtAgentOfferingsManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    channel: Option<ResponseChannel<ShinkaiMessage>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!(
        "{} {} > Network Message Got message from {:?}. Processing and sending ACK",
        my_node_full_name, receiver_address, sender_peer_id
    );

    let mut message = message.clone();

    // Check if the message is coming from a relay proxy and update it
    // ONLY if our identity is localhost (tradeoff for not having an identity)
    if my_node_full_name.starts_with("@@localhost.") {
        let proxy_connection = proxy_connection_info.lock().await;
        if let Some(proxy_info) = &*proxy_connection {
            if message.external_metadata.sender == proxy_info.proxy_identity.get_node_name_string() {
                match ShinkaiName::new(message.external_metadata.intra_sender.clone()) {
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
        Ok(MessageSchemaType::TextContent) | Ok(MessageSchemaType::JobMessageSchema)
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
                        return Ok(());
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<InvoiceRequest>(&content) {
                        Ok(invoice_request) => {
                            // Successfully converted, you can now use shared_folder_infos
                            let mut ext_agent_offering_manager = ext_agent_offering_manager.lock().await;
                            let _ = ext_agent_offering_manager
                                .network_invoice_requested(requester, invoice_request, Some(message.external_metadata))
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

                    eprintln!(
                        "🔑 InvoiceRequestNetworkError Received from: {:?} to {:?}",
                        requester, receiver
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
                            let tracing_id =
                                get_invoice_tracing_id(&maybe_db, &invoice_request_network_error.invoice_id).await;
                            let _ = maybe_db.add_tracing(
                                &tracing_id,
                                None,
                                "invoice_network_error",
                                &json!({"error": invoice_request_network_error.error_message}),
                            );
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to InvoiceRequestNetworkError: {}", e),
                            );
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(id) = val.get("invoice_id").and_then(|v| v.as_str()) {
                                    let tracing_id = get_invoice_tracing_id(&maybe_db, id).await;
                                    let _ = maybe_db.add_tracing(
                                        &tracing_id,
                                        None,
                                        "invoice_network_error_deserialize",
                                        &json!({"error": e.to_string()}),
                                    );
                                }
                            }
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
                    eprintln!("🔑 Invoice Received from: {:?} to {:?}", requester, receiver);

                    let my_agent_offering_manager = if let Some(manager) = my_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade my_agent_offering_manager",
                        );
                        return Ok(());
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<Invoice>(&content) {
                        Ok(invoice) => {
                            let my_agent_offering_manager = my_agent_offering_manager.lock().await;
                            if let Err(e) = my_agent_offering_manager.store_invoice(&invoice).await {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to store invoice: {:?}", e),
                                );
                            }
                            let tracing_id = get_invoice_tracing_id(&maybe_db, &invoice.invoice_id).await;

                            let trace_info = json!({
                                "provider": invoice.provider_name.to_string(),
                                "requester": invoice.requester_name.to_string(),
                                "tool_key": invoice.shinkai_offering.tool_key,
                                "usage_type": format!("{:?}", invoice.usage_type_inquiry),
                                "invoice_date": invoice.invoice_date_time.to_rfc3339(),
                                "expiration": invoice.expiration_time.to_rfc3339(),
                                "address": {
                                    "network": format!("{:?}", invoice.address.network_id),
                                    "address_id": invoice.address.address_id,
                                },
                                "has_tool_data": invoice.tool_data.is_some(),
                            });

                            if let Err(e) = maybe_db.add_tracing(&tracing_id, None, "invoice_received", &trace_info) {
                                eprintln!("failed to add invoice trace: {:?}", e);
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to Invoice: {}", e),
                            );
                            eprintln!("Failed to deserialize JSON to Invoice: {}", e);
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(id) = val.get("invoice_id").and_then(|v| v.as_str()) {
                                    let tracing_id = get_invoice_tracing_id(&maybe_db, id).await;
                                    let _ = maybe_db.add_tracing(
                                        &tracing_id,
                                        None,
                                        "invoice_deserialize_error",
                                        &json!({"error": e.to_string()}),
                                    );
                                }
                            }
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
                    eprintln!("🔑 PaidInvoice Received from: {:?} to {:?}", requester, receiver);

                    let ext_agent_offering_manager = if let Some(manager) = external_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade external_agent_offering_manager",
                        );
                        return Ok(());
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<Invoice>(&content) {
                        Ok(invoice) => {
                            let mut ext_agent_offering_manager = ext_agent_offering_manager.lock().await;
                            if let Err(e) = ext_agent_offering_manager
                                .network_confirm_invoice_payment_and_process(
                                    requester,
                                    invoice,
                                    Some(message.external_metadata),
                                )
                                .await
                            {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to confirm invoice payment: {:?}", e),
                                );
                            }
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
                    eprintln!("🔑 InvoiceResult Received from: {:?} to {:?}", requester, receiver);

                    let my_agent_offering_manager = if let Some(manager) = my_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade my_agent_offering_manager",
                        );
                        return Ok(());
                    };

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<Invoice>(&content) {
                        Ok(invoice_result) => {
                            println!("Invoice result received: {:?}", invoice_result);
                            let my_agent_offering_manager = my_agent_offering_manager.lock().await;
                            if let Err(e) = my_agent_offering_manager.store_invoice_result(&invoice_result).await {
                                shinkai_log(
                                    ShinkaiLogOption::Network,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to store invoice result: {:?}", e),
                                );
                            }
                            let tracing_id = get_invoice_tracing_id(&maybe_db, &invoice_result.invoice_id).await;
                            if let Err(e) = maybe_db.add_tracing(
                                &tracing_id,
                                None,
                                "invoice_result_received",
                                &json!({"status": invoice_result.status}),
                            ) {
                                eprintln!("failed to add invoice result trace: {:?}", e);
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to deserialize JSON to InvoiceResult: {}", e),
                            );
                            eprintln!("Failed to deserialize JSON to InvoiceResult: {}", e);
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(id) = val.get("invoice_id").and_then(|v| v.as_str()) {
                                    let tracing_id = get_invoice_tracing_id(&maybe_db, id).await;
                                    let _ = maybe_db.add_tracing(
                                        &tracing_id,
                                        None,
                                        "invoice_result_deserialize_error",
                                        &json!({"error": e.to_string()}),
                                    );
                                }
                            }
                        }
                    }
                }
                MessageSchemaType::AgentNetworkOfferingRequest => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    println!("AgentNetworkOfferingRequest received from: {:?}", requester);

                    let ext_agent_offering_manager = if let Some(manager) = external_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        return Ok(());
                    };
                    if let Ok(req) = serde_json::from_str::<AgentNetworkOfferingRequest>(
                        &message.get_message_content().unwrap_or_default(),
                    ) {
                        let ext_manager = ext_agent_offering_manager.lock().await;
                        let _ = ext_manager
                            .network_agent_offering_requested(
                                requester,
                                req.agent_identity,
                                Some(message.external_metadata),
                            )
                            .await;
                    }
                }
                MessageSchemaType::AgentNetworkOfferingResponse => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;

                    let my_manager = if let Some(manager) = my_agent_offering_manager.upgrade() {
                        manager
                    } else {
                        return Ok(());
                    };

                    if let Ok(resp) = serde_json::from_str::<AgentNetworkOfferingResponse>(
                        &message.get_message_content().unwrap_or_default(),
                    ) {
                        let my_manager = my_manager.lock().await;
                        if let Some(val) = resp.value {
                            my_manager.store_agent_network_offering(requester.to_string(), val);
                        }
                    } else {
                        eprintln!(
                            "Failed to deserialize JSON to AgentNetworkOfferingResponse: {:?}",
                            message.get_message_content()
                        );
                    }
                }
                _ => {
                    // Ignore other schemas
                    eprintln!("🔑 Ignoring other schemas. Schema: {:?}", schema);
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
            eprintln!("🔑 Error getting message schema: {:?}", e);
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!("{} > Error getting message schema: {:?}", receiver_address, e),
            );
        }
    }

    send_ack(
        clone_static_secret_key(my_encryption_secret_key),
        clone_signature_secret_key(my_signature_secret_key),
        sender_encryption_pk,
        my_node_full_name.to_string(),
        sender_profile_name,
        libp2p_event_sender.clone(),
        channel,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn send_ack(
    encryption_secret_key: EncryptionStaticKey,
    signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    sender: ShinkaiNameString,
    receiver: ShinkaiNameString,
    libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    channel: Option<ResponseChannel<ShinkaiMessage>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg = ShinkaiMessageBuilder::ack_message(
        clone_static_secret_key(&encryption_secret_key),
        signature_secret_key,
        receiver_public_key,
        sender,
        receiver,
    )
    .unwrap();

    if let Some(channel) = channel {
        let network_event = NetworkEvent::SendResponse {
            channel: channel,
            message: msg,
        };
        if let Some(libp2p_event_sender) = libp2p_event_sender.as_ref() {
            if let Err(e) = libp2p_event_sender.send(network_event) {
                eprintln!("Failed to send response: {:?}", e);
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to send response: {:?}", e),
                );
            }
        } else {
            eprintln!("No libp2p event sender");
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Error,
                &format!("No libp2p event sender"),
            );
        }
    } else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "No channel defined.",
        )));
    }

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
    maybe_db: Arc<SqliteManager>,
    maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        libp2p_event_sender,
    );
    Ok(())
}
