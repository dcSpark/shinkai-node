use crate::{
    db::ShinkaiDB,
    managers::IdentityManager,
    network::{
        node::ProxyConnectionInfo,
        subscription_manager::{
            external_subscriber_manager::{ExternalSubscriberManager, SharedFolderInfo},
            fs_entry_tree::FSEntryTree,
            my_subscription_manager::MySubscriptionsManager,
        },
        ws_manager::WSUpdateHandler,
        Node,
    },
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use shinkai_message_primitives::{
    schemas::{
        shinkai_name::{ShinkaiName, ShinkaiNameError},
        shinkai_subscription::SubscriptionId,
    },
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_error::ShinkaiMessageError,
        shinkai_message_extension::EncryptionStatus,
        shinkai_message_schemas::{
            APISubscribeToSharedFolder, APIUnsubscribeToSharedFolder, MessageSchemaType, SubscriptionGenericResponse,
            SubscriptionResponseStatus,
        },
    },
    shinkai_utils::{
        encryption::clone_static_secret_key,
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString},
        signatures::{clone_signature_secret_key, signature_public_key_to_string},
    },
};
use std::sync::Arc;
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
    my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
    external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
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
                my_subscription_manager,
                external_subscription_manager,
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
                my_subscription_manager,
                external_subscription_manager,
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
                my_subscription_manager,
                external_subscription_manager,
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
    my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
    external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
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
                            my_subscription_manager,
                            external_subscription_manager,
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
                        my_subscription_manager,
                        external_subscription_manager,
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
    my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
    external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
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

    // Check the schema of the message and decide what to do
    match message.get_message_content_schema() {
        Ok(schema) => {
            match schema {
                MessageSchemaType::AvailableSharedItems => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!("{} > AvailableSharedItems from {:?}", receiver_address, requester),
                    );

                    let mut response = "".to_string();

                    // Access the subscription_manager, which is of type Arc<Mutex<Option<SubscriberManager>>>
                    let mut subscription_manager = external_subscription_manager.lock().await;

                    // Now, the lock is released, and we can proceed without holding onto the `MutexGuard`
                    let path = "/"; // Define the path you want to query
                    let shared_folder_infos = subscription_manager.get_cached_shared_folder_tree(path).await;
                    if !shared_folder_infos.is_empty() {
                        // Transform Vec<Arc<SharedFolderInfo>> to Vec<&SharedFolderInfo> for serialization
                        let shared_folder_infos_ref: Vec<&SharedFolderInfo> = shared_folder_infos.iter().collect();

                        // Attempt to serialize the vector of SharedFolderInfo references to a JSON string
                        match serde_json::to_string(&shared_folder_infos_ref) {
                            Ok(shared_folder_info_str) => {
                                response = shared_folder_info_str;
                            }
                            Err(e) => println!("Failed to serialize SharedFolderInfo: {}", e),
                        }
                    } else {
                        // The requested path is not cached
                        println!("No cached shared folder information found for path: {}", path);
                    }

                    // 1.5- extract info from the original message

                    let request_node_name = requester.get_node_name_string();
                    let request_profile_name = requester.get_profile_name_string().unwrap_or("".to_string());

                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;

                    // 2.- Create message using vecfs_available_shared_items_response
                    // Send message back with response
                    let msg = ShinkaiMessageBuilder::vecfs_available_shared_items_response(
                        response,
                        clone_static_secret_key(my_encryption_secret_key),
                        clone_signature_secret_key(my_signature_secret_key),
                        sender_encryption_pk,
                        my_node_full_name.to_string(),
                        receiver.get_profile_name_string().unwrap_or("".to_string()),
                        request_node_name.clone(),
                        request_profile_name,
                    )
                    .unwrap();

                    // 3.- Send message back with response
                    Node::send(
                        msg,
                        Arc::new(clone_static_secret_key(my_encryption_secret_key)),
                        (sender_address, request_node_name),
                        proxy_connection_info,
                        maybe_db,
                        maybe_identity_manager,
                        ws_manager,
                        false,
                        None,
                    );
                    return Ok(());
                }
                MessageSchemaType::AvailableSharedItemsResponse => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} AvailableSharedItemsResponse from: {:?}",
                            receiver_address, requester
                        ),
                    );

                    // 2.- extract response from the message
                    let content = message.get_message_content().unwrap_or("".to_string());
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} AvailableSharedItemsResponse Node {}. Received response: {}",
                            receiver_address, my_node_full_name, content
                        ),
                    );

                    // Convert the response string to Vec<SharedFolderInfo>
                    match serde_json::from_str::<String>(&content) {
                        Ok(json_string) => {
                            // Now, deserialize the JSON string (without the outer quotes) to Vec<SharedFolderInfo>
                            match serde_json::from_str::<Vec<SharedFolderInfo>>(&json_string) {
                                Ok(shared_folder_infos) => {
                                    // Successfully converted, you can now use shared_folder_infos
                                    let mut my_subscription_manager = my_subscription_manager.lock().await;
                                    let _ = my_subscription_manager
                                        .insert_shared_folder(requester, shared_folder_infos)
                                        .await;
                                }
                                Err(e) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!(
                                            "AvailableSharedItemsResponse Failed to deserialize JSON to Vec<SharedFolderInfo>: {}",
                                            e
                                        ),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "AvailableSharedItemsResponse Failed to deserialize outer JSON string: {}",
                                    e
                                ),
                            );
                        }
                    }

                    return Ok(());
                }
                MessageSchemaType::SubscribeToSharedFolder => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > SubscribeToSharedFolder from: {:?} to: {:?}",
                            receiver_address, requester, receiver
                        ),
                    );

                    let content = message.get_message_content().unwrap_or("".to_string());
                    match serde_json::from_str::<APISubscribeToSharedFolder>(&content) {
                        Ok(subscription_request) => {
                            // Successfully converted, you can now use shared_folder_infos
                            let streamer = ShinkaiName::from_node_and_profile_names(
                                subscription_request.streamer_node_name,
                                subscription_request.streamer_profile_name.clone(),
                            )
                            .map_err(|e| ShinkaiNameError::InvalidNameFormat(e.to_string()))?;
                            let mut external_subscriber_manager = external_subscription_manager.lock().await;
                            let result = external_subscriber_manager
                                .subscribe_to_shared_folder(
                                    requester.clone(),
                                    streamer,
                                    subscription_request.path.clone(),
                                    subscription_request.payment,
                                    subscription_request.http_preferred,
                                )
                                .await;
                            match result {
                                Ok(_) => {
                                    let response = SubscriptionGenericResponse {
                                        subscription_details: format!("Subscribed to {}", subscription_request.path),
                                        status: SubscriptionResponseStatus::Success,
                                        shared_folder: subscription_request.path,
                                        error: None,
                                        metadata: None,
                                    };

                                    let request_profile = requester.get_profile_name_string().unwrap_or("".to_string());
                                    let msg = ShinkaiMessageBuilder::p2p_subscription_generic_response(
                                        response,
                                        MessageSchemaType::SubscribeToSharedFolderResponse,
                                        clone_static_secret_key(my_encryption_secret_key),
                                        clone_signature_secret_key(my_signature_secret_key),
                                        sender_encryption_pk,
                                        my_node_full_name.to_string(),
                                        subscription_request.streamer_profile_name,
                                        requester.get_node_name_string(),
                                        request_profile,
                                    )
                                    .unwrap();

                                    Node::send(
                                        msg,
                                        Arc::new(clone_static_secret_key(my_encryption_secret_key)),
                                        (sender_address, requester.get_node_name_string()),
                                        proxy_connection_info,
                                        maybe_db,
                                        maybe_identity_manager,
                                        ws_manager,
                                        false,
                                        None,
                                    );
                                    return Ok(());
                                }
                                Err(e) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!("Subscription failed: {}", e),
                                    );
                                    // TODO: Send error message back in APISubscribeToSharedFolderResponse
                                }
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "SubscribeToSharedFolder Failed to deserialize JSON to Vec<SharedFolderInfo>: {}",
                                    e
                                ),
                            );
                            // TODO: Send error message back in APISubscribeToSharedFolderResponse
                        }
                    }

                    return Ok(());
                }
                MessageSchemaType::SubscribeToSharedFolderResponse => {
                    let requester = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let receiver = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > SubscribeToSharedFolderResponse from: {:?} to: {:?}",
                            receiver_address, requester, receiver
                        ),
                    );

                    let requester_profile_name = requester.get_profile_name_string();
                    let content = message.get_message_content().unwrap_or("".to_string());
                    let receiver_profile = receiver.get_profile_name_string().unwrap_or("".to_string());

                    match serde_json::from_str::<SubscriptionGenericResponse>(&content) {
                        Ok(response) => {
                            // Successfully converted, you can now use shared_folder_infos
                            let my_subscription_manager = my_subscription_manager.lock().await;
                            let result = my_subscription_manager
                                .update_subscription_status(
                                    requester.extract_node(),
                                    requester_profile_name.unwrap_or("".to_string()),
                                    receiver_profile,
                                    MessageSchemaType::SubscribeToSharedFolderResponse,
                                    response,
                                )
                                .await;

                            match result {
                                Ok(_) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Debug,
                                        &format!(
                                            "SubscribeToSharedFolderResponse Node {}: Successfully updated subscription status",
                                            my_node_full_name
                                        ),
                                    );
                                }
                                Err(e) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!(
                                            "SubscribeToSharedFolderResponse Failed to update subscription status: {}",
                                            e
                                        ),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "SubscribeToSharedFolderResponse Failed to deserialize JSON to SubscriptionGenericResponse: {}",
                                    e
                                ),
                            );
                        }
                    }

                    return Ok(());
                }
                MessageSchemaType::SubscriptionRequiresTreeUpdate => {
                    let streamer_node_with_profile =
                        ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    let requester_node_with_profile =
                        ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > SubscriptionRequiresTreeUpdate from: {:?} to: {:?}",
                            receiver_address, streamer_node_with_profile, requester_node_with_profile
                        ),
                    );

                    let streamer_node = streamer_node_with_profile.extract_node();
                    let streamer_profile_name = streamer_node_with_profile.get_profile_name_string().unwrap();
                    let requester_node = requester_node_with_profile.extract_node();
                    let requester_profile_name = requester_node_with_profile.get_profile_name_string().unwrap();

                    // TODO: convert to SubscriptionGenericResponse type
                    let content = message
                        .get_message_content()
                        .unwrap_or("".to_string())
                        .trim_matches('"')
                        .to_string();
                    println!(
                        "SubscribeToSharedFolderResponse Node {}. Received response: {}",
                        my_node_full_name, content
                    );

                    let shared_folder = content.clone();
                    let subscription_id = SubscriptionId::new(
                        streamer_node.clone(),
                        streamer_profile_name.clone(),
                        shared_folder.clone(),
                        requester_node.clone(),
                        requester_profile_name.clone(),
                    );

                    let my_subscription_manager = my_subscription_manager.lock().await;
                    let result = my_subscription_manager
                        .share_local_shared_folder_copy_state(
                            streamer_node,
                            streamer_profile_name,
                            requester_node,
                            requester_profile_name,
                            subscription_id.get_unique_id().to_string(),
                        )
                        .await;

                    match result {
                        Ok(_) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Debug,
                                &format!(
                                    "SubscriptionRequiresTreeUpdate Node {}: Successfully updated subscription status",
                                    my_node_full_name
                                ),
                            );
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "SubscriptionRequiresTreeUpdate Failed to update subscription status: {}",
                                    e
                                ),
                            );
                        }
                    }

                    return Ok(());
                }
                // Note(Nico): This is usually coming from a request but we also can allow it without the request
                // for when the node transitions to a new state (e.g. hard reset, recovery to previous state, etc).
                MessageSchemaType::SubscriptionRequiresTreeUpdateResponse => {
                    let streamer_node_with_profile =
                        ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    let requester_node_with_profile =
                        ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > SubscriptionRequiresTreeUpdateResponse from: {:?} to: {:?}",
                            receiver_address, streamer_node_with_profile, requester_node_with_profile
                        ),
                    );

                    let streamer_node = streamer_node_with_profile.extract_node();
                    let streamer_profile_name = streamer_node_with_profile.get_profile_name_string().unwrap();
                    let requester_node = requester_node_with_profile.extract_node();
                    let requester_profile_name = requester_node_with_profile.get_profile_name_string().unwrap();
                    let item_tree_json_content = message.get_message_content().unwrap_or("".to_string());

                    match serde_json::from_str::<SubscriptionGenericResponse>(&item_tree_json_content) {
                        Ok(response) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Debug,
                                &format!(
                                    "SubscriptionRequiresTreeUpdateResponse Node {}: Handling SubscribeToSharedFolderResponse from: {}",
                                    my_node_full_name, requester_node_with_profile.get_node_name_string()
                                ),
                            );
                            // Attempt to deserialize the inner JSON string into FSEntryTree
                            if let Some(metadata) = response.metadata {
                                let symmetric_key = metadata.get("symmetric_key").cloned();
                                if symmetric_key.is_none() {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        "symmetric_key not found in metadata",
                                    );
                                    // Handle the case where 'symmetric_key' is missing in metadata
                                    // Potentially return or handle error here
                                } else if let Some(tree_content) = metadata.get("folder_state") {
                                    let symmetric_key = symmetric_key.unwrap();
                                    match serde_json::from_str::<FSEntryTree>(tree_content) {
                                        Ok(item_tree) => {
                                            let subscription_unique_id = SubscriptionId::new(
                                                streamer_node.clone(),
                                                streamer_profile_name.clone(),
                                                response.shared_folder.clone(),
                                                requester_node.clone(),
                                                requester_profile_name.clone(),
                                            );
                                            let external_subscriber_manager =
                                                external_subscription_manager.lock().await;
                                            let _ = external_subscriber_manager
                                                .subscriber_current_state_response(
                                                    subscription_unique_id.get_unique_id().to_string(),
                                                    item_tree,
                                                    requester_node,
                                                    requester_profile_name,
                                                    symmetric_key,
                                                )
                                                .await;
                                            shinkai_log(
                                                ShinkaiLogOption::Network,
                                                ShinkaiLogLevel::Debug,
                                                &format!(
                                                    "SubscriptionRequiresTreeUpdateResponse Node {}: Successfully updated subscription status",
                                                    my_node_full_name
                                                ),
                                            );
                                        }
                                        Err(e) => {
                                            shinkai_log(
                                                ShinkaiLogOption::Network,
                                                ShinkaiLogLevel::Error,
                                                &format!(
                                                    "Failed to deserialize inner JSON string to FSEntryTree: {}",
                                                    e
                                                ),
                                            );
                                        }
                                    }
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        "'folder_state' not found in metadata",
                                    );
                                    // Handle the case where 'folder_state' is missing in metadata
                                }
                            } else {
                                shinkai_log(ShinkaiLogOption::Network, ShinkaiLogLevel::Error, "Metadata is missing");
                                // Handle the case where metadata is missing
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "SubscriptionRequiresTreeUpdateResponse Failed to deserialize JSON to SubscriptionGenericResponse: {}",
                                    e
                                ),
                            );
                        }
                    }
                }
                MessageSchemaType::UnsubscribeToSharedFolder => {
                    let streamer_node_with_profile =
                        ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                    let requester_node_with_profile =
                        ShinkaiName::from_shinkai_message_using_sender_subidentity(&message)?;
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Debug,
                        &format!(
                            "{} > Unsubscribe Request from: {:?} to: {:?}",
                            receiver_address, streamer_node_with_profile, requester_node_with_profile
                        ),
                    );

                    // Extract the shared folder path from the message content
                    let json_content = message.get_message_content().unwrap_or("".to_string());

                    match serde_json::from_str::<APIUnsubscribeToSharedFolder>(&json_content) {
                        Ok(response) => {
                            // Call unsubscribe_from_shared_folder
                            let mut external_subscriber_manager = external_subscription_manager.lock().await;
                            match external_subscriber_manager
                                .unsubscribe_from_shared_folder(
                                    requester_node_with_profile.clone(),
                                    streamer_node_with_profile.clone(),
                                    response.path.clone(),
                                )
                                .await
                            {
                                Ok(result) => {
                                    if result {
                                        shinkai_log(
                                            ShinkaiLogOption::Network,
                                            ShinkaiLogLevel::Debug,
                                            "Successfully unsubscribed from shared folder.",
                                        );
                                    } else {
                                        shinkai_log(
                                            ShinkaiLogOption::Network,
                                            ShinkaiLogLevel::Error,
                                            "Failed to unsubscribe from shared folder.",
                                        );
                                    }

                                    let status = if result {
                                        SubscriptionResponseStatus::Success
                                    } else {
                                        SubscriptionResponseStatus::Failure
                                    };

                                    let response = SubscriptionGenericResponse {
                                        subscription_details: format!(
                                            "Unsubscribing to {} Successful",
                                            response.path.clone()
                                        ),
                                        status,
                                        shared_folder: response.path.clone(),
                                        error: None,
                                        metadata: None,
                                    };

                                    let msg = ShinkaiMessageBuilder::p2p_subscription_generic_response(
                                        response,
                                        MessageSchemaType::UnsubscribeToSharedFolderResponse,
                                        clone_static_secret_key(my_encryption_secret_key),
                                        clone_signature_secret_key(my_signature_secret_key),
                                        sender_encryption_pk,
                                        my_node_full_name.to_string(),
                                        streamer_node_with_profile
                                            .get_profile_name_string()
                                            .unwrap_or("".to_string()),
                                        requester_node_with_profile.get_node_name_string(),
                                        requester_node_with_profile
                                            .get_profile_name_string()
                                            .unwrap_or("".to_string()),
                                    )
                                    .unwrap();

                                    Node::send(
                                        msg,
                                        Arc::new(clone_static_secret_key(my_encryption_secret_key)),
                                        (sender_address, requester_node_with_profile.get_node_name_string()),
                                        proxy_connection_info.clone(),
                                        maybe_db.clone(),
                                        maybe_identity_manager.clone(),
                                        ws_manager.clone(),
                                        false,
                                        None,
                                    );
                                }
                                Err(e) => {
                                    shinkai_log(
                                        ShinkaiLogOption::Network,
                                        ShinkaiLogLevel::Error,
                                        &format!("Error unsubscribing from shared folder: {:?}", e),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!(
                                    "UnsubscribeToSharedFolder Failed to deserialize JSON to SubscriptionGenericResponse: {}",
                                    e
                                ),
                            );
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
        my_subscription_manager,
        external_subscription_manager,
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
    _my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
    _external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
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

// Helper struct to encapsulate sender keys
#[derive(Debug)]
pub struct PublicKeyInfo {
    pub address: SocketAddr,
    pub signature_public_key: VerifyingKey,
    pub encryption_public_key: x25519_dalek::PublicKey,
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
