use super::identity_network_manager::IdentityNetworkManager;
use shinkai_db::db::db_errors::ShinkaiDBError;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::identity::{DeviceIdentity, Identity, StandardIdentity, StandardIdentityType};
use crate::network::network_manager::network_handlers::verify_message_signature;
use crate::network::node_error::NodeError;
use async_trait::async_trait;
use shinkai_crypto_identities::ShinkaiRegistryError;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::IdentityPermissions;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct IdentityManager {
    pub local_node_name: ShinkaiName,
    pub local_identities: Vec<Identity>,
    pub db: Weak<ShinkaiDB>,
    pub external_identity_manager: Arc<Mutex<IdentityNetworkManager>>,
    pub is_ready: bool,
}

// Note this makes testing much easier
#[async_trait]
pub trait IdentityManagerTrait {
    fn find_by_identity_name(&self, full_profile_name: ShinkaiName) -> Option<&Identity>;
    async fn search_identity(&self, full_identity_name: &str) -> Option<Identity>;
    fn clone_box(&self) -> Box<dyn IdentityManagerTrait + Send>;
    async fn external_profile_to_global_identity(&self, full_profile_name: &str) -> Result<StandardIdentity, String>;
}

impl Clone for Box<dyn IdentityManagerTrait + Send> {
    fn clone(&self) -> Box<dyn IdentityManagerTrait + Send> {
        self.clone_box()
    }
}

impl IdentityManager {
    pub async fn new(db: Weak<ShinkaiDB>, local_node_name: ShinkaiName) -> Result<Self, Box<dyn std::error::Error>> {
        let local_node_name = local_node_name.extract_node();
        let mut identities: Vec<Identity> = {
            let db = db.upgrade().ok_or(ShinkaiRegistryError::CustomError(
                "Couldn't convert to strong db".to_string(),
            ))?;
            db.get_all_profiles_and_devices(local_node_name.clone())?
                .into_iter()
                .collect()
        };

        let llm_providers = {
            let db = db.upgrade().ok_or(ShinkaiRegistryError::CustomError(
                "Couldn't convert to strong db".to_string(),
            ))?;
            db.get_all_llm_providers()?
                .into_iter()
                .map(Identity::LLMProvider)
                .collect::<Vec<_>>()
        };
        {
            let db = db.upgrade().ok_or(ShinkaiRegistryError::CustomError(
                "Couldn't convert to strong db".to_string(),
            ))?;
            db.debug_print_all_keys_for_profiles_identity_key();
        }

        identities.extend(llm_providers);

        let external_identity_manager = Arc::new(Mutex::new(IdentityNetworkManager::new().await));

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

    pub fn get_main_identity(&self) -> Option<&Identity> {
        self.local_identities.iter().find(|identity| match identity {
            Identity::Standard(standard_identity) => {
                standard_identity
                    .full_identity_name
                    .get_profile_name_string()
                    .unwrap_or_default()
                    == "main"
            }
            _ => false,
        })
    }

    pub async fn add_profile_subidentity(&mut self, identity: StandardIdentity) -> anyhow::Result<()> {
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!("add_profile_subidentity > identity: {}", identity).as_str(),
        );
        let previously_had_profile_identity = self.has_profile_identity();
        self.local_identities.push(Identity::Standard(identity.clone()));

        if !previously_had_profile_identity && self.has_profile_identity() {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Debug,
                format!("YAY! first profile added! identity: {}", identity).as_str(),
            );
            self.is_ready = true;
        }
        Ok(())
    }

    pub async fn add_llm_provider_subidentity(&mut self, llm_provider: SerializedLLMProvider) -> anyhow::Result<()> {
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!("add_agent_subidentity > llm provider: {:?}", llm_provider).as_str(),
        );
        self.local_identities.push(Identity::LLMProvider(llm_provider.clone()));
        Ok(())
    }

    pub async fn modify_llm_provider_subidentity(
        &mut self,
        updated_llm_provider: SerializedLLMProvider,
    ) -> anyhow::Result<()> {
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!(
                "modify_llm_provider_subidentity > updated_llm_provider: {:?}",
                updated_llm_provider
            )
            .as_str(),
        );

        let mut found = false;
        for identity in &mut self.local_identities {
            if let Identity::LLMProvider(agent) = identity {
                if agent.full_identity_name == updated_llm_provider.full_identity_name {
                    *agent = updated_llm_provider.clone();
                    found = true;
                    break;
                }
            }
        }

        if found {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Debug,
                format!("Agent modified: {:?}", updated_llm_provider.full_identity_name).as_str(),
            );
            Ok(())
        } else {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Error,
                format!("Agent not found: {}", updated_llm_provider.full_identity_name).as_str(),
            );
            Err(anyhow::anyhow!(
                "Agent with ID '{}' not found.",
                updated_llm_provider.full_identity_name
            ))
        }
    }

    pub async fn remove_agent_subidentity(&mut self, llm_provider_id: &str) -> anyhow::Result<()> {
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!("remove_agent_subidentity > llm_provider_id: {}", llm_provider_id).as_str(),
        );
        // eprintln!("all llm providers: {:?}", self.local_identities);

        let initial_count = self.local_identities.len();
        self.local_identities.retain(|identity| match identity {
            Identity::LLMProvider(agent) => {
                agent.full_identity_name.get_agent_name_string().unwrap() != llm_provider_id
            }
            _ => true,
        });

        let final_count = self.local_identities.len();
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Debug,
            format!("Removed {} agent(s)", initial_count - final_count).as_str(),
        );

        if initial_count == final_count {
            Err(anyhow::anyhow!("Agent with ID '{}' not found.", llm_provider_id))
        } else {
            Ok(())
        }
    }

    pub async fn add_device_subidentity(&mut self, device: DeviceIdentity) -> anyhow::Result<()> {
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Info,
            format!("add_device_subidentity > device: {}", device).as_str(),
        );
        self.local_identities.push(Identity::Device(device.clone()));
        Ok(())
    }

    pub fn has_profile_identity(&self) -> bool {
        self.local_identities.iter().any(|identity| {
            matches!(identity, Identity::Standard(standard_identity) if standard_identity.identity_type == StandardIdentityType::Profile)
        })
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
                    Identity::LLMProvider(agent) => {
                        if agent.full_identity_name.to_string() == full_identity_name {
                            Some(Identity::LLMProvider(agent.clone()))
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

    pub async fn search_local_llm_provider(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
    ) -> Option<SerializedLLMProvider> {
        let db_arc = self.db.upgrade()?;
        db_arc.get_llm_provider(agent_id, profile).ok().flatten()
    }

    // Primarily for testing
    pub fn get_all_subidentities_devices_and_llm_providers(&self) -> Vec<Identity> {
        self.local_identities.clone()
    }

    pub fn get_all_subidentities(&self) -> Vec<Identity> {
        self.local_identities.clone()
    }

    pub async fn get_all_llm_providers(&self) -> Result<Vec<SerializedLLMProvider>, ShinkaiDBError> {
        let db_arc = self
            .db
            .upgrade()
            .ok_or(ShinkaiDBError::SomeError("Couldn't convert to db strong".to_string()))?;
        db_arc.get_all_llm_providers()
    }
}

#[async_trait]
impl IdentityManagerTrait for IdentityManager {
    fn find_by_identity_name(&self, full_profile_name: ShinkaiName) -> Option<&Identity> {
        self.local_identities.iter().find(|identity| {
            match identity {
                Identity::Standard(identity) => identity.full_identity_name == full_profile_name,
                Identity::Device(device) => device.full_identity_name == full_profile_name,
                Identity::LLMProvider(agent) => agent.full_identity_name == full_profile_name, // Assuming the 'name' field of Agent struct can be considered as the profile name
            }
        })
    }

    async fn search_identity(&self, full_identity_name: &str) -> Option<Identity> {
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
                Ok(identity_network_manager) => match identity_network_manager.first_address().await {
                    Ok(first_address) => {
                        let encryption_key = match identity_network_manager.encryption_public_key() {
                            Ok(key) => key,
                            Err(_) => return None,
                        };
                        let signature_key = match identity_network_manager.signature_verifying_key() {
                            Ok(key) => key,
                            Err(_) => return None,
                        };

                        Some(Identity::Standard(StandardIdentity::new(
                            node_name,
                            Some(first_address),
                            encryption_key,
                            signature_key,
                            None,
                            None,
                            StandardIdentityType::Global,
                            IdentityPermissions::None,
                        )))
                    }
                    Err(_) => None,
                },
                Err(_) => None, // return None if the identity is not found in the network manager
            }
        }
    }

    fn clone_box(&self) -> Box<dyn IdentityManagerTrait + Send> {
        Box::new(self.clone())
    }

    async fn external_profile_to_global_identity(
        &self,
        full_profile_name: &str,
    ) -> Result<StandardIdentity, String> {
        shinkai_log(
            ShinkaiLogOption::Identity,
            ShinkaiLogLevel::Debug,
            format!(
                "external_profile_to_global_identity > full_profile_name: {}",
                full_profile_name
            )
            .as_str(),
        );

        let full_identity_name = match ShinkaiName::new(full_profile_name.to_string().clone()) {
            Ok(name) => name,
            Err(_) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    "external_profile_to_global_identity > is_valid_node_identity_name_and_no_subidentities: false",
                );
                return Err(format!(
                    "Failed to convert profile name to ShinkaiName: {}",
                    full_profile_name
                ));
            }
        };
        let node_name = full_identity_name.get_node_name_string().to_string();

        let external_im = self.external_identity_manager.lock().await;

        match external_im
            .external_identity_to_profile_data(node_name.to_string())
            .await
        {
            Ok(identity_network_manager) => match identity_network_manager.first_address().await {
                Ok(first_address) => {
                    let encryption_key = match identity_network_manager.encryption_public_key() {
                        Ok(key) => key,
                        Err(e) => return Err(format!("Failed to get encryption public key: {}", e)),
                    };
                    let signature_key = match identity_network_manager.signature_verifying_key() {
                        Ok(key) => key,
                        Err(e) => return Err(format!("Failed to get signature verifying key: {}", e)),
                    };
                    Ok(StandardIdentity::new(
                        full_identity_name.extract_node(),
                        Some(first_address),
                        encryption_key,
                        signature_key,
                        None,
                        None,
                        StandardIdentityType::Global,
                        IdentityPermissions::None,
                    ))
                }
                Err(_) => Err("Failed to get first address".to_string()),
            },
            Err(_) => Err(format!(
                "Failed to get identity network manager for profile name: {}",
                full_profile_name
            )),
        }
    }
}

impl IdentityManager {
    pub fn get_full_identity_name(identity: &Identity) -> Option<String> {
        match identity {
            Identity::Standard(std_identity) => Some(std_identity.full_identity_name.clone().to_string()),
            Identity::LLMProvider(agent) => Some(agent.full_identity_name.clone().to_string()),
            Identity::Device(device) => Some(device.full_identity_name.clone().to_string()),
        }
    }

    pub fn verify_message_signature(
        sender_subidentity: Option<Identity>,
        original_message: &ShinkaiMessage,
        decrypted_message: &ShinkaiMessage,
    ) -> Result<(), NodeError> {
        // eprintln!("signature check > sender_subidentity: {:?}", sender_subidentity);
        if sender_subidentity.is_none() {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Error,
                format!(
                    "signature check > Subidentity not found for profile name: {}",
                    decrypted_message.external_metadata.clone().sender
                )
                .as_str(),
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
        // eprintln!("signature check > subidentity: {:?}", subidentity);

        // Validate that the message actually came from the subidentity
        let signature_public_key = match &subidentity {
            Identity::Standard(std_identity) => std_identity.profile_signature_public_key,
            Identity::Device(std_device) => Some(std_device.device_signature_public_key),
            Identity::LLMProvider(_) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    "signature check > Agent identities cannot send onionized messages",
                );
                return Ok(());
            }
        };

        if signature_public_key.is_none() {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Error,
                format!(
                    "signature check > Signature public key doesn't exist for identity: {}",
                    subidentity.get_full_identity_name()
                )
                .as_str(),
            );
            return Err(NodeError {
                message: "Failed to verify message signature. Signature public key doesn't exist for identity"
                    .to_string(),
            });
        }

        match verify_message_signature(signature_public_key.unwrap(), &original_message.clone()) {
            Ok(_) => Ok(()),
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Identity,
                    ShinkaiLogLevel::Error,
                    format!("signature check > Failed to verify message signature: {}", e).as_str(),
                );
                Err(NodeError {
                    message: format!("Failed to verify message signature: {}", e),
                })
            }
        }
    }
}
