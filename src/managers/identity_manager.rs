use crate::db::ShinkaiDB;
use crate::network::node_error::NodeError;
use crate::network::node_message_handlers::verify_message_signature;
use crate::schemas::identity::{
    DeviceIdentity, Identity, IdentityType, StandardIdentity, StandardIdentityType,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_wasm::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_message_wasm::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::IdentityPermissions;
use shinkai_message_wasm::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_public_key_to_string_ref,
};
use shinkai_message_wasm::shinkai_utils::signatures::{
    signature_public_key_to_string, signature_public_key_to_string_ref,
};
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::identity_network_manager::IdentityNetworkManager;

#[derive(Clone)]
pub struct IdentityManager {
    pub local_node_name: ShinkaiName,
    pub local_identities: Vec<Identity>,
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub external_identity_manager: Arc<Mutex<IdentityNetworkManager>>,
    pub is_ready: bool,
}

impl IdentityManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        local_node_name: ShinkaiName,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let local_node_name = local_node_name.extract_node();
        let mut identities: Vec<Identity> = {
            let db = db.lock().await;
            db.get_all_profiles_and_devices(local_node_name.clone())?
                .into_iter()
                .collect()
        };
        println!("identities_manager identities: {:?}", identities);

        let agents = {
            let db = db.lock().await;
            db.get_all_agents()?
                .into_iter()
                .map(Identity::Agent)
                .collect::<Vec<_>>()
        };

        {
            let db = db.lock().await;
            db.debug_print_all_keys_for_profiles_identity_key();
            println!("identities_manager agents: {:?}", identities);
        }

        identities.extend(agents);

        let external_identity_manager = Arc::new(Mutex::new(IdentityNetworkManager::new()));

        // Logic to check if the node is ready
        let current_ready_status = identities.iter().any(|identity| {
            matches!(identity, Identity::Standard(standard_identity) if standard_identity.identity_type == StandardIdentityType::Profile)
        });

        Ok(Self {
            local_node_name: local_node_name.extract_node(),
            local_identities: identities,
            db,
            external_identity_manager,
            is_ready: current_ready_status,
        })
    }

    pub async fn add_profile_subidentity(&mut self, identity: StandardIdentity) -> anyhow::Result<()> {
        eprintln!("add_profile_subidentity > identity: {}", identity);
        let previously_had_profile_identity = self.has_profile_identity();
        self.local_identities.push(Identity::Standard(identity.clone()));

        if !previously_had_profile_identity && self.has_profile_identity() {
            eprintln!("YAY! first profile added!");
            self.is_ready = true;
        }

        {
            let db = self.db.lock().await;
            db.debug_print_all_keys_for_profiles_identity_key();
        }
        Ok(())
    }

    pub async fn add_agent_subidentity(&mut self, agent: SerializedAgent) -> anyhow::Result<()> {
        println!("add_agent_subidentity > agent: {:?}", agent);
        self.local_identities.push(Identity::Agent(agent.clone()));
        Ok(())
    }

    pub async fn add_device_subidentity(&mut self, device: DeviceIdentity) -> anyhow::Result<()> {
        println!("add_device_subidentity > device: {}", device);
        self.local_identities.push(Identity::Device(device.clone()));

        {
            let db = self.db.lock().await;
            db.debug_print_all_keys_for_profiles_identity_key();
        }
        Ok(())
    }

    pub fn has_profile_identity(&self) -> bool {
        self.local_identities.iter().any(|identity| {
            matches!(identity, Identity::Standard(standard_identity) if standard_identity.identity_type == StandardIdentityType::Profile)
        })
    }

    pub async fn search_local_identity(&self, full_identity_name: &str) -> Option<Identity> {
        {
            let db = self.db.lock().await;
            db.debug_print_all_keys_for_profiles_identity_key();
        }

        let node_in_question = ShinkaiName::new(full_identity_name.to_string()).ok()?.extract_node();
        println!("search_local_identity > node_in_question: {}", node_in_question);

        // If the node name matches local node, search in self.identities
        if self.local_node_name == node_in_question {
            self.local_identities
                .iter()
                .filter_map(|identity| match identity {
                    Identity::Standard(standard_identity) => {
                        if standard_identity.full_identity_name.to_string() == full_identity_name {
                            println!("FOUND! search_local_identity > standard_identity: {}", standard_identity);
                            Some(Identity::Standard(standard_identity.clone()))
                        } else {
                            None
                        }
                    }
                    Identity::Agent(agent) => {
                        if agent.full_identity_name.to_string() == full_identity_name {
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
        println!("search_local_agent > agent_id: {}", agent_id);
        let db = self.db.lock().await;
        db.get_agent(agent_id).ok().flatten()
    }

    // Primarily for testing
    pub fn get_all_subidentities_devices_and_agents(&self) -> Vec<Identity> {
        self.local_identities.clone()
    }

    pub async fn search_identity(&self, full_identity_name: &str) -> Option<Identity> {
        println!("search_identity > full_identity_name: {}", full_identity_name);
        {
            let db = self.db.lock().await;
            db.debug_print_all_keys_for_profiles_identity_key();
        }
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
        println!("identities_manager identities: {:?}", self.local_identities); 
        self.local_identities.clone()
    }

    pub async fn get_all_agents(&self) -> Result<Vec<SerializedAgent>, rocksdb::Error> {
        let db = self.db.lock().await;
        db.get_all_agents()
    }

    pub fn find_by_identity_name(&self, full_profile_name: ShinkaiName) -> Option<&Identity> {
        println!("identities_manager identities: {:?}", self.local_identities); 
        self.local_identities.iter().find(|identity| {
            match identity {
                Identity::Standard(identity) => identity.full_identity_name == full_profile_name,
                Identity::Device(device) => device.full_identity_name == full_profile_name,
                Identity::Agent(agent) => agent.full_identity_name == full_profile_name, // Assuming the 'name' field of Agent struct can be considered as the profile name
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
    pub fn get_full_identity_name(identity: &Identity) -> Option<String> {
        match identity {
            Identity::Standard(std_identity) => Some(std_identity.full_identity_name.clone().to_string()),
            Identity::Agent(agent) => Some(agent.full_identity_name.clone().to_string()),
            Identity::Device(device) => Some(device.full_identity_name.clone().to_string()),
        }
    }

    pub fn verify_message_signature(
        sender_subidentity: Option<Identity>,
        original_message: &ShinkaiMessage,
        decrypted_message: &ShinkaiMessage,
    ) -> Result<(), NodeError> {
        eprintln!("signature check > sender_subidentity: {:?}", sender_subidentity);
        if sender_subidentity.is_none() {
            eprintln!(
                "signature check > Subidentity not found for profile name: {}",
                decrypted_message.external_metadata.clone().sender
            );
            return Err(NodeError {
                message: format!(
                    "Subidentity not found for profile name: {}",
                    decrypted_message.external_metadata.clone().sender
                ),
            });
        }
        // If we reach this point, it means that subidentity exists, so it's safe to unwrap
        let subidentity = sender_subidentity.unwrap();

        // Validate that the message actually came from the subidentity
        let signature_public_key = match &subidentity {
            Identity::Standard(std_identity) => std_identity.profile_signature_public_key.clone(),
            Identity::Device(std_device) => Some(std_device.device_signature_public_key.clone()),
            Identity::Agent(_) => {
                eprintln!("signature check > Agent identities cannot send onionized messages");
                return Ok(());
            }
        };

        if signature_public_key.is_none() {
            eprintln!(
                "signature check > Signature public key doesn't exist for identity: {}",
                subidentity.get_full_identity_name()
            );
            return Err(NodeError {
                message: format!("Failed to verify message signature. Signature public key doesn't exist for identity"),
            });
        }

        match verify_message_signature(signature_public_key.unwrap(), &original_message.clone()) {
            Ok(_) => Ok({}),
            Err(e) => {
                eprintln!("signature check > Failed to verify message signature: {}", e);
                return Err(NodeError {
                    message: format!("Failed to verify message signature: {}", e.to_string()),
                });
            }
        }
    }
}
