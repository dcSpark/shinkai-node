use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use async_channel::Sender;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use tokio::sync::Mutex;

use crate::{db::ShinkaiDB, managers::IdentityManager};

use super::{
    node_api::APIError,
    node_error::NodeError,
    node_message_handlers::{extract_message, verify_message_signature},
    Node,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeProxyMode {
    // Node acts as a proxy, holds identities it proxies for
    // and a flag indicating if it allows new identities
    // if the flag is also then it will also clean up saved identities
    IsProxy(IsProxyConf),
    // Node is being proxied, holds its proxy's identity
    IsProxied(ProxyIdentity),
    // Node is not using a proxy
    NoProxy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IsProxyConf {
    // Flag indicating if new identities can be added
    pub allow_new_identities: bool,
    // Starting node identities
    pub proxy_node_identities: HashMap<String, ProxyIdentity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProxyIdentity {
    // Address of the API proxy
    pub api_peer: SocketAddr,
    // Address of the TCP proxy
    pub tcp_peer: SocketAddr,
    // Name of the proxied node
    // Or the name of my identity proxied
    pub shinkai_name: ShinkaiName,
}

impl Node {
    async fn is_recipient_proxied(
        db: Arc<Mutex<ShinkaiDB>>,
        potentially_encrypted_msg: &ShinkaiMessage,
    ) -> Result<Option<ProxyIdentity>, NodeError> {
        let recipient_node =
            ShinkaiName::from_shinkai_message_only_using_recipient_node_name(&potentially_encrypted_msg.clone());
        match recipient_node {
            Ok(recipient_node_name) => {
                let result = db.lock().await.get_proxied_identity(&recipient_node_name);
                match result {
                    Ok(Some(proxied_identity)) => Ok(Some(proxied_identity)),
                    Ok(None) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Debug,
                            format!("No proxied identity found for node: {}", recipient_node_name).as_str(),
                        );
                        Ok(None)
                    }
                    Err(err) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            format!("Error getting proxied identity: {}", err).as_str(),
                        );
                        Err(NodeError {
                            message: format!("Error getting proxied identity: {}", err),
                        })
                    }
                }
            }
            Err(_) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    "Error getting recipient node name from message",
                );
                Err(NodeError {
                    message: String::from("Error getting recipient node name from message"),
                })
            }
        }
    }

    pub async fn handle_received_message(
        receiver_address: SocketAddr,
        unsafe_sender_address: SocketAddr,
        bytes: &[u8],
        my_node_profile_name: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        maybe_db: Arc<Mutex<ShinkaiDB>>,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
        node_proxy_mode: NodeProxyMode,
    ) -> Result<(), NodeError> {
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > Got message from {:?}", receiver_address, unsafe_sender_address),
        );

        // Extract and validate the message
        let message = extract_message(bytes, receiver_address)?;
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Decoded Message: {:?}", receiver_address, message),
        );

        // Extract sender's public keys and verify the signature
        let sender_node_name_string = ShinkaiName::from_shinkai_message_only_using_sender_node_name(&message)
            .unwrap()
            .get_node_name();
        let sender_identity = maybe_identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&sender_node_name_string)
            .await
            .unwrap();

        verify_message_signature(sender_identity.node_signature_public_key, &message)?;

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!(
                "{} > Sender Profile Name: {:?}",
                receiver_address, sender_node_name_string
            ),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Node Sender Identity: {}", receiver_address, sender_identity),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Verified message signature", receiver_address),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Sender Identity: {}", receiver_address, sender_identity),
        );

        // TODO: add handle_based_on_message_content_and_encryption back
        // ^ need to review this

        match &node_proxy_mode {
            NodeProxyMode::IsProxied(proxyIdentity) => {
                Node::handle_message_no_proxy(
                    message,
                    sender_node_name_string,
                    sender_identity,
                    my_encryption_secret_key,
                    my_signature_secret_key,
                    my_node_profile_name,
                    maybe_db,
                    maybe_identity_manager,
                    receiver_address,
                    unsafe_sender_address,
                )
                .await
            }
            NodeProxyMode::IsProxy(IsProxyConf) => match Node::is_recipient_proxied(maybe_db.clone(), &message).await {
                Ok(Some(proxied_identity)) => {
                    let peer = proxied_identity.tcp_peer;
                    Node::send(
                        message,
                        Arc::new(my_encryption_secret_key),
                        (peer, proxied_identity.shinkai_name.get_node_name()),
                        maybe_db,
                        maybe_identity_manager,
                        false,
                        Some(u32::MAX),
                    );
                    Ok(())
                }
                Ok(None) => Ok(()),
                Err(err) => Err(err),
            },
            NodeProxyMode::NoProxy => {
                Node::handle_message_no_proxy(
                    message,
                    sender_node_name_string,
                    sender_identity,
                    my_encryption_secret_key,
                    my_signature_secret_key,
                    my_node_profile_name,
                    maybe_db,
                    maybe_identity_manager,
                    receiver_address,
                    unsafe_sender_address,
                )
                .await
            }
        }
    }

    pub async fn handle_send_message(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    ) -> Result<(), NodeError> {
        match &self.proxy_mode {
            NodeProxyMode::IsProxied(_) => {
                // I received the message! so we are already good
                self.api_handle_send_onionized_message(potentially_encrypted_msg, res)
                    .await
            }
            NodeProxyMode::IsProxy(_) => match Node::is_recipient_proxied(self.db.clone(), &potentially_encrypted_msg).await {
                Ok(Some(proxied_identity)) => {
                    let api_peer = proxied_identity.api_peer;
                    let client = reqwest::Client::new();
                    let res = client
                        .post(format!("http://{}:{}/v1/send", api_peer.ip(), api_peer.port()))
                        .json(&potentially_encrypted_msg)
                        .send()
                        .await;
                    match res {
                        Ok(response) => {
                            if response.status().is_success() {
                                Ok(())
                            } else {
                                Err(NodeError {
                                    message: format!("Failed to send message to peer: {}", response.status()),
                                })
                            }
                        }
                        Err(err) => Err(NodeError {
                            message: format!("Failed to send message to peer: {}", err),
                        }),
                    }
                }
                Ok(None) => Ok(()),
                Err(err) => Err(err),
            },
            NodeProxyMode::NoProxy => {
                self.api_handle_send_onionized_message(potentially_encrypted_msg, res)
                    .await
            }
        }
    }
}
