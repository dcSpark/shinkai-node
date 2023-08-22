use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use lazy_static::lazy_static;
use shinkai_message_wasm::shinkai_utils::{signatures::{signature_public_key_to_string, string_to_signature_public_key}, encryption::string_to_encryption_public_key};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};
use std::net::ToSocketAddrs;

#[derive(Debug)]
pub struct NetworkIdentity {
    pub node_identity_name: String,
    pub addr: SocketAddr,
    pub signature_public_key: SignaturePublicKey,
    pub encryption_public_key: EncryptionPublicKey,
}

pub struct IdentityNetworkManager {
    identities: Arc<Mutex<HashMap<String, NetworkIdentity>>>,
}

impl IdentityNetworkManager {
    pub fn get_mock_identities() -> HashMap<String, NetworkIdentity> {
        let mut m = HashMap::new();
        // RPC call simulation for node data fetch
        // Here, I am reusing the hardcoded data, but you should replace this section with actual RPC calls
        m.insert(
            "@@node1.shinkai".to_string(), // nico
            NetworkIdentity {
                node_identity_name: "@@node1.shinkai".to_string(),
                addr: SocketAddr::from(([192, 168, 1, 109], 8080)),
                signature_public_key: string_to_signature_public_key("801e158387f6d78ebf39104a964ea1b11b4d3e5269556fb9fd063daf92dfa972")
                    .expect("Failed to parse public key"),
                encryption_public_key: string_to_encryption_public_key("798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679")
                    .expect("Failed to parse public key"),
            },
        );
        m.insert(
            "@@node2.shinkai".to_string(), // nico-linux
            NetworkIdentity {
                node_identity_name: "@@node2.shinkai".to_string(),
                addr: SocketAddr::from(([192, 168, 1, 233], 8081)),
                signature_public_key: string_to_signature_public_key("7c5585c186fb3f45c66c0722b963e36e444535111ed49d4b481d3e9020bbd6cf")
                    .expect("Failed to parse public key"),
                encryption_public_key: string_to_encryption_public_key("6d8072573dbebb0305777f57332286271693f8c050843d7bf15db81a4556446a")
                    .expect("Failed to parse public key"),
            },
        );
        m.insert(
            "@@node3.shinkai".to_string(),
            NetworkIdentity {
                node_identity_name: "@@node3.shinkai".to_string(),
                addr: SocketAddr::from(([127, 0, 0, 1], 8082)),
                signature_public_key: string_to_signature_public_key("63dd3953fe0b9e3212503fc1de9be9b46008615a4522facf271f0c2b3585c3e6")
                    .expect("Failed to parse public key"),
                encryption_public_key: string_to_encryption_public_key("3273d113e401a215e429e3272352186a7370cf7edf1e2d68aa7ef87a20237371")
                    .expect("Failed to parse public key"),
            },
        );
        m.insert(
            "@@node1_test.shinkai".to_string(), // nico
            NetworkIdentity {
                node_identity_name: "@@node1_test.shinkai".to_string(),
                addr: SocketAddr::from(([127, 0, 0, 1], 8080)),
                signature_public_key: string_to_signature_public_key("801e158387f6d78ebf39104a964ea1b11b4d3e5269556fb9fd063daf92dfa972")
                    .expect("Failed to parse public key"),
                encryption_public_key: string_to_encryption_public_key("798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679")
                    .expect("Failed to parse public key"),
            },
        );
        m.insert(
            "@@node2_test.shinkai".to_string(), // nico-linux
            NetworkIdentity {
                node_identity_name: "@@node2_test.shinkai".to_string(),
                addr: SocketAddr::from(([127, 0, 0, 1], 8081)),
                signature_public_key: string_to_signature_public_key("7c5585c186fb3f45c66c0722b963e36e444535111ed49d4b481d3e9020bbd6cf")
                    .expect("Failed to parse public key"),
                encryption_public_key: string_to_encryption_public_key("6d8072573dbebb0305777f57332286271693f8c050843d7bf15db81a4556446a")
                    .expect("Failed to parse public key"),
            },
        );
        m
    }

    pub fn new() -> Self {
        let identities = Arc::new(Mutex::new(Self::get_mock_identities()));
        IdentityNetworkManager { identities }
    }

    pub async fn external_identity_to_profile_data(
        &self,
        global_identity: String,
    ) -> Result<NetworkIdentity, &'static str> {
        let identities = self.identities.lock().await;
        match identities.get(&global_identity) {
            Some(data) => Ok(NetworkIdentity {
                node_identity_name: data.node_identity_name.clone(),
                addr: data.addr,
                signature_public_key: data.signature_public_key.clone(),
                encryption_public_key: data.encryption_public_key.clone(),
            }),
            None => Err("Unrecognized global identity"),
        }
    }

    pub async fn identity_pk_to_external_identity(&self, pk: SignaturePublicKey) -> Result<String, &'static str> {
        let pk_string = signature_public_key_to_string(pk);
        let identities = self.identities.lock().await;
        for (global_identity, data) in identities.iter() {
            if signature_public_key_to_string(data.signature_public_key) == pk_string {
                return Ok(global_identity.clone());
            }
        }
        Err("Unrecognized public key")
    }

    pub async fn addr_to_external_profile_data(&self, addr: SocketAddr) -> Vec<NetworkIdentity> {
        let mut result = Vec::new();
        let identities = self.identities.lock().await;

        for (global_identity, data) in identities.iter() {
            if data.addr == addr {
                result.push(NetworkIdentity {
                    node_identity_name: data.node_identity_name.clone(),
                    addr: data.addr,
                    signature_public_key: data.signature_public_key.clone(),
                    encryption_public_key: data.encryption_public_key.clone(),
                });
            }
        }

        result
    }
}
