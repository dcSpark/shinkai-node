use chrono::Utc;
use shinkai_crypto_identities::{OnchainIdentity, ShinkaiRegistry};
use shinkai_libp2p_relayer::RelayManager;
use shinkai_message_primitives::shinkai_utils::{
    encryption::{encryption_public_key_to_string, unsafe_deterministic_encryption_keypair},
    signatures::{signature_public_key_to_string, unsafe_deterministic_signature_keypair},
};
use shinkai_node::{managers::identity_network_manager::IdentityNetworkManager, network::Node};
use std::{sync::Arc, time::SystemTime};
use tokio::{sync::Mutex, task::JoinHandle};

/// Run the real relay with local registry records. No deployed relay is required.
pub struct LocalRelay(JoinHandle<()>);

impl LocalRelay {
    pub async fn start(nodes: &[&Arc<Mutex<Node>>]) -> Self {
        let relay_name = "@@libp2p_relayer.sep-shinkai";
        let (signature_secret, signature_public) = unsafe_deterministic_signature_keypair(42);
        let (encryption_secret, encryption_public) = unsafe_deterministic_encryption_keypair(42);
        let mut registry = ShinkaiRegistry::new("http://127.0.0.1:1", "unused", None)
            .await
            .unwrap();
        // Unknown records must never fall back to public RPC endpoints.
        registry.rpc_endpoints = vec!["http://127.0.0.1:1".into()];
        let record = |name: String, encryption_key: String, signature_key: String, routing| OnchainIdentity {
            shinkai_identity: name.trim_start_matches("@@").to_string(),
            bound_nft: "0".into(),
            staked_tokens: "0".into(),
            encryption_key,
            signature_key,
            routing,
            address_or_proxy_nodes: Vec::new(),
            delegated_tokens: "0".into(),
            last_updated: Utc::now(),
        };
        let relay = record(
            relay_name.into(),
            encryption_public_key_to_string(encryption_public),
            signature_public_key_to_string(signature_public),
            false,
        );
        registry
            .cache
            .insert(relay.shinkai_identity.clone(), (SystemTime::now(), relay));
        for node in nodes {
            let node = node.lock().await;
            let identity = record(
                node.node_name.get_node_name_string(),
                encryption_public_key_to_string(node.encryption_public_key),
                signature_public_key_to_string(node.identity_public_key),
                true,
            );
            registry
                .cache
                .insert(identity.shinkai_identity.clone(), (SystemTime::now(), identity));
        }
        let mut records = registry
            .cache
            .iter()
            .map(|entry| entry.value().1.clone())
            .collect::<Vec<_>>();
        // Localhost peers have no registry identity. Keep their records only in
        // node resolvers, where the tests need their local keys and route.
        registry.cache.retain(|identity, _| !identity.starts_with("localhost."));
        let (mut relay, address) =
            RelayManager::new_loopback(relay_name.into(), signature_secret, encryption_secret, registry)
                .await
                .unwrap();
        // The relay owns the socket before any resolver receives its address.
        for record in &mut records {
            record.address_or_proxy_nodes = vec![address.to_string()];
        }
        for node in nodes {
            node.lock()
                .await
                .identity_manager
                .lock()
                .await
                .external_identity_manager =
                Arc::new(Mutex::new(IdentityNetworkManager::from_records(records.clone())));
        }
        Self(tokio::spawn(async move {
            relay.run().await.expect("Local relay failed");
        }))
    }
}

impl Drop for LocalRelay {
    fn drop(&mut self) {
        self.0.abort();
    }
}
