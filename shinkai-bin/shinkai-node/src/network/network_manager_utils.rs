use std::sync::{Arc, Weak};

use shinkai_message_primitives::{
    schemas::{identity::StandardIdentity, shinkai_proxy_builder_info::ShinkaiProxyBuilderInfo}, shinkai_message::shinkai_message::ShinkaiMessage, shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption}
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::{Mutex, RwLock};

use crate::managers::identity_manager::IdentityManagerTrait;

use super::{
    agent_payments_manager::external_agent_offerings_manager::AgentOfferingManagerError, node::ProxyConnectionInfo, Node
};
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn get_proxy_builder_info_static(
    identity_manager_lock: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
    proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
) -> Option<ShinkaiProxyBuilderInfo> {
    let identity_manager = identity_manager_lock.lock().await;
    let proxy_connection_info = match proxy_connection_info.upgrade() {
        Some(proxy_info) => proxy_info,
        None => return None,
    };

    let proxy_connection_info = proxy_connection_info.lock().await;
    if let Some(proxy_connection) = proxy_connection_info.as_ref() {
        let proxy_name = proxy_connection.proxy_identity.clone().get_node_name_string();
        match identity_manager
            .external_profile_to_global_identity(&proxy_name, None)
            .await
        {
            Ok(proxy_identity) => Some(ShinkaiProxyBuilderInfo {
                proxy_enc_public_key: proxy_identity.node_encryption_public_key,
            }),
            Err(_) => None,
        }
    } else {
        None
    }
}

pub async fn send_message_to_peer(
    message: ShinkaiMessage,
    db: Weak<SqliteManager>,
    receiver_identity: StandardIdentity,
    my_encryption_secret_key: EncryptionStaticKey,
    maybe_identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
    proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
) -> Result<(), AgentOfferingManagerError> {
    shinkai_log(
        ShinkaiLogOption::MySubscriptions,
        ShinkaiLogLevel::Debug,
        format!(
            "Sending message to peer: {}",
            receiver_identity.full_identity_name.extract_node()
        )
        .as_str(),
    );
    // Extract the receiver's socket address and profile name from the StandardIdentity
    let receiver_socket_addr = receiver_identity.addr.ok_or_else(|| {
        AgentOfferingManagerError::OperationFailed(
            format!(
                "Shinkai ID doesn't have a valid socket address: {}",
                receiver_identity.full_identity_name.extract_node()
            )
            .to_string(),
        )
    })?;
    let receiver_profile_name = receiver_identity.full_identity_name.to_string();

    // Upgrade the weak reference to Node
    // Prepare the parameters for the send function
    let my_encryption_sk = Arc::new(my_encryption_secret_key.clone());
    let peer = (receiver_socket_addr, receiver_profile_name);
    let db = db.upgrade().ok_or(AgentOfferingManagerError::OperationFailed(
        "DB not available to be upgraded".to_string(),
    ))?;
    let maybe_identity_manager = maybe_identity_manager
        .upgrade()
        .ok_or(AgentOfferingManagerError::OperationFailed(
            "IdentityManager not available to be upgraded".to_string(),
        ))?;

    let proxy_connection_info = proxy_connection_info
        .upgrade()
        .ok_or(AgentOfferingManagerError::OperationFailed(
            "ProxyConnectionInfo not available to be upgraded".to_string(),
        ))?;

    // Call the send function
    Node::send(
        message,
        my_encryption_sk,
        peer,
        proxy_connection_info,
        db,
        maybe_identity_manager,
        None,
        false,
        None,
    );

    Ok(())
}
