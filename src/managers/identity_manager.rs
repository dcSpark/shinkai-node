use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::schemas::identity::{Identity, StandardIdentity, IdentityType, StandardIdentityType, DeviceIdentity, IdentityPermissions};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde::ser::{Serializer, SerializeStruct};
use shinkai_message_wasm::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_wasm::shinkai_utils::encryption::{encryption_public_key_to_string, encryption_public_key_to_string_ref};
use shinkai_message_wasm::shinkai_utils::signatures::{signature_public_key_to_string, signature_public_key_to_string_ref};
use std::sync::Arc;
use std::{fmt, net::SocketAddr};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::agent_serialization::SerializedAgent;
use super::identity_network_manager::IdentityNetworkManager;

#[derive(Clone)]
pub struct IdentityManager {
    pub local_node_name: String,
    pub local_identities: Vec<Identity>,
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub external_identity_manager: Arc<Mutex<IdentityNetworkManager>>,
}

impl IdentityManager {
    pub async fn new(db: Arc<Mutex<ShinkaiDB>>, local_node_name: String) -> Result<Self, Box<dyn std::error::Error>> {
        if local_node_name.clone().is_empty() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Local node name cannot be empty",
            )));
        }
        match IdentityManager::is_valid_node_identity_name_and_no_subidentities(&local_node_name.clone()) == false {
            true => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Local node name is not valid",
                )))
            }
            false => (),
        }

        let mut identities: Vec<Identity> = {
            let db = db.lock().await;
            db.load_all_sub_identities(local_node_name.clone())?
                .into_iter()
                .map(Identity::Standard)
                .collect()
        };

        let agents = {
            let db = db.lock().await;
            db.get_all_agents()?
                .into_iter()
                .map(Identity::Agent)
                .collect::<Vec<_>>()
        };

        identities.extend(agents);

        // TODO: enable this later on once we add the state machine to the node for adding the first subidentity
        // if identities.is_empty() {
        //     return Err(Box::new(std::io::Error::new(
        //         std::io::ErrorKind::Other,
        //         "No identities found in database",
        //     )));
        // }
        let external_identity_manager = Arc::new(Mutex::new(IdentityNetworkManager::new()));

        Ok(Self {
            local_node_name,
            local_identities: identities,
            db,
            external_identity_manager,
        })
    }

    pub async fn add_subidentity(&mut self, identity: StandardIdentity) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        let normalized_identity = StandardIdentity::new(
            IdentityManager::extract_subidentity(&identity.full_identity_name.clone()),
            identity.addr.clone(),
            identity.node_encryption_public_key.clone(),
            identity.node_signature_public_key.clone(),
            identity.profile_encryption_public_key.clone(),
            identity.profile_signature_public_key.clone(),
            identity.identity_type.clone(),
            identity.permission_type.clone(),
        );
        db.insert_sub_identity(normalized_identity.clone())?;
        self.local_identities.push(Identity::Standard(normalized_identity.clone()));
        Ok(())
    }

    pub fn identities_to_profile_names(identities: Vec<StandardIdentity>) -> anyhow::Result<Vec<String>> {
        let profile_names = identities
            .into_iter()
            .map(|identity| identity.full_identity_name)
            .collect();

        Ok(profile_names)
    }

    pub async fn add_agent_subidentity(
        &mut self,
        agent: SerializedAgent,
    ) -> anyhow::Result<()> {
        let mut db = self.db.lock().await;
        db.add_agent(agent.clone())?;
        self.local_identities.push(Identity::Agent(agent.clone()));
        Ok(())
    }

    pub async fn search_local_identity(&self, full_identity_name: &str) -> Option<Identity> {
        let node_name = full_identity_name.split('/').next().unwrap_or(full_identity_name);
    
        // If the node name matches local node, search in self.identities
        if self.local_node_name == node_name {
            self.local_identities
                .iter()
                .filter_map(|identity| {
                    match identity {
                        Identity::Standard(standard_identity) => {
                            if standard_identity.full_identity_name == full_identity_name {
                                Some(Identity::Standard(standard_identity.clone()))
                            } else {
                                None
                            }
                        },
                        Identity::Agent(agent) => {
                            if agent.id == full_identity_name {
                                Some(Identity::Agent(agent.clone()))
                            } else {
                                None
                            }
                        },
                        Identity::Device(device) => {
                            if device.full_identity_name == full_identity_name {
                                Some(Identity::Device(device.clone()))
                            } else {
                                None
                            }
                        },
                    }
                })
                .next()
        } else {
            None
        }
    }

    pub async fn search_local_agent(&self, agent_id: &str) -> Option<SerializedAgent> {
        let db = self.db.lock().await;
        db.get_agent(agent_id).ok().flatten()
    }

    pub async fn search_identity(&self, full_identity_name: &str) -> Option<Identity> {
        // Extract node name from the full identity name
        let node_name = full_identity_name.split('/').next().unwrap_or(full_identity_name);
    
        // If the node name matches local node, search in self.identities
        if self.local_node_name == node_name {
            self.search_local_identity(full_identity_name).await
        } else {
            // If not, query the identity network manager
            let external_im = self.external_identity_manager.lock().await;
            match external_im
                .external_identity_to_profile_data(full_identity_name.to_string())
                .await
            {
                Ok(identity_network_manager) => Some(Identity::Standard(StandardIdentity::new(
                    node_name.to_string(),
                    Some(identity_network_manager.addr),
                    identity_network_manager.encryption_public_key,
                    identity_network_manager.signature_public_key,
                    None,
                    None,
                    StandardIdentityType::Global,
                    IdentityPermissions::None,
                ))),
                Err(_) => None, // return None if the identity is not found in the network manager
            }
        }
    }
    

    pub fn get_all_subidentities(&self) -> Vec<Identity> {
        self.local_identities.clone()
    }

    pub async fn get_all_agents(&self) -> Result<Vec<SerializedAgent>, rocksdb::Error> {
        let db = self.db.lock().await;
        db.get_all_agents()
    }

    pub fn find_by_signature_key(&self, key: &SignaturePublicKey) -> Option<&Identity> {
        self.local_identities
            .iter()
            .find(|identity| {
                match identity {
                    Identity::Standard(identity) => identity.profile_signature_public_key.as_ref() == Some(key),
                    // TODO: fix this
                    Identity::Device(device) => device.profile_signature_public_key.as_ref() == Some(key),
                    Identity::Agent(_) => false, // Return false if the identity is an Agent
                }
            })
    }
    
    pub fn find_by_profile_name(&self, full_profile_name: &str) -> Option<&Identity> {
        self.local_identities
            .iter()
            .find(|identity| {
                match identity {
                    Identity::Standard(identity) => identity.full_identity_name == full_profile_name,
                    Identity::Device(device) => device.full_identity_name == full_profile_name,
                    Identity::Agent(agent) => agent.name == full_profile_name, // Assuming the 'name' field of Agent struct can be considered as the profile name
                }
            })
    }

    pub async fn external_profile_to_global_identity(&self, full_profile_name: &str) -> Option<StandardIdentity> {
        let node_name = IdentityManager::extract_node_name(full_profile_name);

        println!(
            "external_profile_to_global_identity > full_profile_name: {}",
            full_profile_name
        );
        println!("external_profile_to_global_identity > node_name: {}", node_name);
        // validate the profile name
        if IdentityManager::is_valid_node_identity_name_and_no_subidentities(&node_name) == false {
            println!("external_profile_to_global_identity > is_valid_node_identity_name_and_no_subidentities: false");
            return None;
        }
        let external_im = self.external_identity_manager.lock().await;
        match external_im
            .external_identity_to_profile_data(node_name.to_string())
            .await
        {
            Ok(identity_network_manager) => Some(StandardIdentity::new(
                node_name.to_string(),
                Some(identity_network_manager.addr),
                identity_network_manager.encryption_public_key,
                identity_network_manager.signature_public_key,
                None,
                None,
                StandardIdentityType::Global,
                IdentityPermissions::None,
            )),
            Err(_) => None, // return None if the identity is not found in the network manager
        }
    }
}

impl IdentityManager {
    pub fn extract_subidentity(s: &str) -> String {
        let re = Regex::new(r"@@[^/]+\.shinkai/(.+)").unwrap();
        re.captures(s)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_else(|| s.to_string())
    }

    pub fn extract_node_name(s: &str) -> String {
        let re = Regex::new(r"(@@[^/]+\.shinkai)(?:/.*)?").unwrap();
        re.captures(s)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_else(|| s.to_string())
    }

    pub fn is_valid_node_identity_name_and_no_subidentities(s: &str) -> bool {
        let re = Regex::new(r"^@@[^/]+\.shinkai$").unwrap();
        re.is_match(s)
    }

    pub fn is_valid_node_identity_name_with_subidentities(s: &str) -> bool {
        let re = Regex::new(r"^@@[^/]+\.shinkai(/[^/]*)*$").unwrap();
        re.is_match(s)
    }
    

    pub fn merge_to_full_identity_name(node_name: String, subidentity_name: String) -> String {
        let name = format!("{}/{}", node_name, subidentity_name);
        IdentityManager::is_valid_node_identity_name_and_no_subidentities(name.clone().as_str());
        name
    }

    // TODO: add a new that creates an Identity instance from a message
    pub fn extract_sender_node_global_name(message: &ShinkaiMessage) -> String {
        let sender_profile_name = message.external_metadata.clone().unwrap().sender;
        IdentityManager::extract_node_name(&sender_profile_name)
    }

    pub fn extract_recipient_node_global_name(message: &ShinkaiMessage) -> String {
        let sender_profile_name = message.external_metadata.clone().unwrap().recipient;
        IdentityManager::extract_node_name(&sender_profile_name)
    }

    pub fn get_full_identity_name(identity: &Identity) -> Option<String> {
        match identity {
            Identity::Standard(std_identity) => Some(std_identity.full_identity_name.clone()),
            Identity::Agent(agent) => Some(agent.name.clone()),
            Identity::Device(device) => Some(device.full_identity_name.clone()),
        }
    }

    pub fn get_profile_name_from_device(device: &DeviceIdentity) -> Option<String> {
        device.to_standard_identity().profile_name().map(|s| s.to_string())
    }
}
