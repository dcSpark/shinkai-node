use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::schemas::identity::{
    DeviceIdentity, Identity, IdentityPermissions, IdentityType, StandardIdentity, StandardIdentityType,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_message_wasm::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_public_key_to_string_ref,
};
use shinkai_message_wasm::shinkai_utils::signatures::{
    signature_public_key_to_string, signature_public_key_to_string_ref,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::agent_serialization::SerializedAgent;
use super::identity_network_manager::IdentityNetworkManager;

#[derive(Clone)]
pub struct IdentityManager {
    pub local_node_name: ShinkaiName,
    pub local_identities: Vec<Identity>,
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub external_identity_manager: Arc<Mutex<IdentityNetworkManager>>,
}

impl IdentityManager {
    pub async fn new(db: Arc<Mutex<ShinkaiDB>>, local_node_name: ShinkaiName) -> Result<Self, Box<dyn std::error::Error>> {
        let mut identities: Vec<Identity> = {
            let db = db.lock().await;
            db.get_all_profiles(local_node_name.clone().get_node_name())?
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
            local_node_name: local_node_name.extract_node(),
            local_identities: identities,
            db,
            external_identity_manager,
        })
    }

    pub async fn add_subidentity(&mut self, identity: StandardIdentity) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        db.insert_profile(identity.clone())?;
        self.local_identities
            .push(Identity::Standard(identity.clone()));
        Ok(())
    }

    pub async fn add_agent_subidentity(&mut self, agent: SerializedAgent) -> anyhow::Result<()> {
        let mut db = self.db.lock().await;
        db.add_agent(agent.clone())?;
        self.local_identities.push(Identity::Agent(agent.clone()));
        Ok(())
    }

    pub async fn search_local_identity(&self, full_identity_name: &str) -> Option<Identity> {
        let node_in_question = ShinkaiName::new(full_identity_name.to_string()).ok()?.extract_node();

        // If the node name matches local node, search in self.identities
        if self.local_node_name == node_in_question {
            self.local_identities
                .iter()
                .filter_map(|identity| match identity {
                    Identity::Standard(standard_identity) => {
                        if standard_identity.full_identity_name.to_string() == full_identity_name {
                            Some(Identity::Standard(standard_identity.clone()))
                        } else {
                            None
                        }
                    }
                    Identity::Agent(agent) => {
                        if agent.id == full_identity_name {
                            Some(Identity::Agent(agent.clone()))
                        } else {
                            None
                        }
                    }
                    Identity::Device(device) => {
                        if device.full_identity_name.to_string() == full_identity_name {
                            Some(Identity::Device(device.clone()))
                        } else {
                            None
                        }
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
        let identity_name = ShinkaiName::new(full_identity_name.to_string()).ok()?;
        let node_name = identity_name.extract_node();

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
                    node_name,
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
        self.local_identities.iter().find(|identity| {
            match identity {
                Identity::Standard(identity) => identity.profile_signature_public_key.as_ref() == Some(key),
                // TODO: fix this
                Identity::Device(device) => device.profile_signature_public_key.as_ref() == Some(key),
                Identity::Agent(_) => false, // Return false if the identity is an Agent
            }
        })
    }

    pub fn find_by_profile_name(&self, full_profile_name: &str) -> Option<&Identity> {
        self.local_identities.iter().find(|identity| {
            match identity {
                Identity::Standard(identity) => identity.full_identity_name.to_string() == full_profile_name,
                Identity::Device(device) => device.full_identity_name.to_string() == full_profile_name,
                Identity::Agent(agent) => agent.full_identity_name.to_string() == full_profile_name, // Assuming the 'name' field of Agent struct can be considered as the profile name
            }
        })
    }

    pub async fn external_profile_to_global_identity(&self, full_profile_name: &str) -> Option<StandardIdentity> {
        println!(
            "external_profile_to_global_identity > full_profile_name: {}",
            full_profile_name
        );

        let full_identity_name = match ShinkaiName::new(full_profile_name.to_string().clone()) {
            Ok(name) => name,
            Err(_) => {
                println!(
                    "external_profile_to_global_identity > is_valid_node_identity_name_and_no_subidentities: false"
                );
                return None;
            }
        };
        let node_name = full_identity_name.get_node_name().to_string();

        let external_im = self.external_identity_manager.lock().await;
        match external_im
            .external_identity_to_profile_data(node_name.to_string())
            .await
        {
            Ok(identity_network_manager) => Some(StandardIdentity::new(
                full_identity_name.extract_node(),
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
    // pub fn extract_subidentity(s: &str) -> String {
    //     let re = Regex::new(r"@@[^/]+\.shinkai/(.+)").unwrap();
    //     re.captures(s)
    //         .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    //         .unwrap_or_else(|| s.to_string())
    // }

    // pub fn extract_node_name(s: &str) -> String {
    //     let re = Regex::new(r"(@@[^/]+\.shinkai)(?:/.*)?").unwrap();
    //     re.captures(s)
    //         .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    //         .unwrap_or_else(|| s.to_string())
    // }

    // pub fn is_valid_node_identity_name_and_no_subidentities(s: &str) -> bool {
    //     let re = Regex::new(r"^@@[^/]+\.shinkai$").unwrap();
    //     re.is_match(s)
    // }

    // pub fn is_valid_node_identity_name_with_subidentities(s: &str) -> bool {
    //     let re = Regex::new(r"^@@[^/]+\.shinkai(/[^/]*)*$").unwrap();
    //     re.is_match(s)
    // }

    // pub fn merge_to_full_identity_name(node_name: String, subidentity_name: String) -> String {
    //     let node_name = if IdentityManager::is_valid_node_identity_name_and_no_subidentities(&node_name) {
    //         node_name
    //     } else {
    //         format!("@@{}.shinkai", node_name)
    //     };

    //     let name = format!("{}/{}", node_name.to_lowercase(), subidentity_name.to_lowercase());
    //     name
    // }

    // TODO: add a new that creates an Identity instance from a message
    // pub fn extract_sender_node_global_name(message: &ShinkaiMessage) -> String {
    //     let sender_profile_name = message.external_metadata.clone().unwrap().sender;
    //     ShinkaiName::new(sender_profile_name).unwrap().node_name().to_string()
    // }

    // pub fn extract_recipient_node_global_name(message: &ShinkaiMessage) -> String {
    //     let sender_profile_name = message.external_metadata.clone().unwrap().recipient;
    //     IdentityManager::extract_node_name(&sender_profile_name)
    // }

    pub fn get_full_identity_name(identity: &Identity) -> Option<String> {
        match identity {
            Identity::Standard(std_identity) => Some(std_identity.full_identity_name.clone().to_string()),
            Identity::Agent(agent) => Some(agent.full_identity_name.clone().to_string()),
            Identity::Device(device) => Some(device.full_identity_name.clone().to_string()),
        }
    }

    // pub fn get_profile_name_from_device(device: &DeviceIdentity) -> Option<String> {
    //     device.to_standard_identity().profile_name().map(|s| s.to_string())
    // }
}
