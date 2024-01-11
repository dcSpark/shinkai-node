use ed25519_dalek::VerifyingKey;
use shinkai_message_primitives::shinkai_utils::{
    encryption::string_to_encryption_public_key,
    signatures::{signature_public_key_to_string, string_to_signature_public_key},
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::crypto_identities::shinkai_registry::{OnchainIdentity, ShinkaiRegistry};

pub struct IdentityNetworkManager {
    registry: Arc<Mutex<ShinkaiRegistry>>,
    identities: Arc<Mutex<HashMap<String, OnchainIdentity>>>,
}

impl IdentityNetworkManager {
    // pub fn get_mock_identities() -> HashMap<String, NetworkIdentity> {
    //     let mut m = HashMap::new();
    //     // RPC call simulation for node data fetch
    //     // Here, I am reusing the hardcoded data, but you should replace this section with actual RPC calls
    //     m.insert(
    //         "@@node1.shinkai".to_string(), // nico
    //         NetworkIdentity {
    //             node_identity_name: "@@node1.shinkai".to_string(),
    //             addr: SocketAddr::from(([192, 168, 1, 109], 8080)),
    //             signature_public_key: string_to_signature_public_key("69fa099bdce516bfeb46d5fc6e908f6cf8ffac0aba76ca0346a7b1a751a2712e")
    //                 .expect("Failed to parse public key"),
    //             encryption_public_key: string_to_encryption_public_key("60045bdb15c24b161625cf05558078208698272bfe113f792ea740dbd79f4708")
    //                 .expect("Failed to parse public key"),
    //         },
    //     );
    //     m.insert(
    //         "@@node2.shinkai".to_string(), // nico-linux
    //         NetworkIdentity {
    //             node_identity_name: "@@node2.shinkai".to_string(),
    //             addr: SocketAddr::from(([192, 168, 1, 233], 8081)),
    //             signature_public_key: string_to_signature_public_key("389fb4bbb3d382a2f2f23cdfa5614ed288975bc4f4a0448876efba108dc2c583")
    //                 .expect("Failed to parse public key"),
    //             encryption_public_key: string_to_encryption_public_key("912fed05e286af45f44580d6a87da61e1f9a0946237dd29f7bc2d3cbeba0857f")
    //                 .expect("Failed to parse public key"),
    //         },
    //     );
    //     m.insert(
    //         "@@node3.shinkai".to_string(),
    //         NetworkIdentity {
    //             node_identity_name: "@@node3.shinkai".to_string(),
    //             addr: SocketAddr::from(([127, 0, 0, 1], 8082)),
    //             signature_public_key: string_to_signature_public_key("63dd3953fe0b9e3212503fc1de9be9b46008615a4522facf271f0c2b3585c3e6")
    //                 .expect("Failed to parse public key"),
    //             encryption_public_key: string_to_encryption_public_key("3273d113e401a215e429e3272352186a7370cf7edf1e2d68aa7ef87a20237371")
    //                 .expect("Failed to parse public key"),
    //         },
    //     );
    //     m.insert(
    //         "@@node1_test.shinkai".to_string(), // nico
    //         NetworkIdentity {
    //             node_identity_name: "@@node1_test.shinkai".to_string(),
    //             addr: SocketAddr::from(([127, 0, 0, 1], 8080)),
    //             signature_public_key: string_to_signature_public_key("69fa099bdce516bfeb46d5fc6e908f6cf8ffac0aba76ca0346a7b1a751a2712e")
    //                 .expect("Failed to parse public key"),
    //             encryption_public_key: string_to_encryption_public_key("60045bdb15c24b161625cf05558078208698272bfe113f792ea740dbd79f4708")
    //                 .expect("Failed to parse public key"),
    //         },
    //     );
    //     m.insert(
    //         "@@node2_test.shinkai".to_string(), // nico-linux
    //         NetworkIdentity {
    //             node_identity_name: "@@node2_test.shinkai".to_string(),
    //             addr: SocketAddr::from(([127, 0, 0, 1], 8081)),
    //             signature_public_key: string_to_signature_public_key("389fb4bbb3d382a2f2f23cdfa5614ed288975bc4f4a0448876efba108dc2c583")
    //                 .expect("Failed to parse public key"),
    //             encryption_public_key: string_to_encryption_public_key("912fed05e286af45f44580d6a87da61e1f9a0946237dd29f7bc2d3cbeba0857f")
    //                 .expect("Failed to parse public key"),
    //         },
    //     );
    //     m
    // }

    pub async fn new() -> Self {
        // TODO: read from config
        let registry = ShinkaiRegistry::new(
            "https://rpc.sepolia.org",
            "0xb2945D0CDa4C119DE184380955aA4FbfAFb6B8cC",
            "./src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json",
        )
        .await
        .unwrap();

        let registry = Arc::new(Mutex::new(registry));
        let identities = Arc::new(Mutex::new(HashMap::new()));
        
        IdentityNetworkManager { registry, identities }
    }

    pub async fn external_identity_to_profile_data(
        &self,
        global_identity: String,
    ) -> Result<OnchainIdentity, &'static str> {
        let record = {
            let mut registry = self.registry.lock().await;
            match registry.get_identity_record(global_identity.clone()).await {
                Ok(record) => record,
                Err(_) => return Err("Unrecognized global identity"),
            }
        };
    
        Ok(record)
    }

    // pub async fn identity_pk_to_external_identity(&self, pk: VerifyingKey) -> Result<String, &'static str> {
    //     let pk_string = signature_public_key_to_string(pk);
    //     let identities = self.identities.lock().await;
    //     for (global_identity, data) in identities.iter() {
    //         if signature_public_key_to_string(data.signature_public_key) == pk_string {
    //             return Ok(global_identity.clone());
    //         }
    //     }
    //     Err("Unrecognized public key")
    // }

    // pub async fn addr_to_external_profile_data(&self, addr: SocketAddr) -> Vec<NetworkIdentity> {
    //     let mut result = Vec::new();
    //     let identities = self.identities.lock().await;

    //     for (_, data) in identities.iter() {
    //         if data.addr == addr {
    //             result.push(NetworkIdentity {
    //                 node_identity_name: data.node_identity_name.clone(),
    //                 addr: data.addr,
    //                 signature_public_key: data.signature_public_key.clone(),
    //                 encryption_public_key: data.encryption_public_key.clone(),
    //             });
    //         }
    //     }

    //     result
    // }
}
