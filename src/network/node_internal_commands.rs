use super::{node_error::NodeError, Node};
use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::managers::identity_manager::IdentityManagerTrait;
use crate::managers::IdentityManager;
use crate::network::network_manager::network_handlers::{ping_pong, PingPong};
use crate::schemas::{
    identity::{Identity, StandardIdentity},
    inbox_permission::InboxPermission,
    smart_inbox::SmartInbox,
};
use async_channel::Sender;
use chashmap::CHashMap;
use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use log::{error, info};
use regex::Regex;
use shinkai_message_primitives::{
    schemas::{
        agents::serialized_agent::{AgentLLMInterface, Ollama, SerializedAgent},
        inbox_name::InboxName,
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::{
        encryption::clone_static_secret_key,
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::clone_signature_secret_key,
    },
};
use std::{
    io::{Error},
    net::SocketAddr,
};
use std::{str::FromStr, sync::Arc};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl Node {
    pub async fn send_peer_addresses(
        peers: CHashMap<(SocketAddr, String), chrono::DateTime<chrono::Utc>>,
        sender: Sender<Vec<SocketAddr>>,
    ) -> Result<(), Error> {
        let peer_addresses: Vec<SocketAddr> = peers.into_iter().map(|(k, _)| k.0).collect();
        sender.send(peer_addresses).await.unwrap();
        Ok(())
    }

    pub async fn handle_external_profile_data(
        identity_manager: Arc<Mutex<IdentityManager>>,
        name: String,
        res: Sender<StandardIdentity>,
    ) -> Result<(), Error> {
        let external_global_identity = identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&name)
            .await
            .unwrap();
        res.send(external_global_identity).await.unwrap();
        Ok(())
    }

    pub async fn internal_get_last_unread_messages_from_inbox(
        db: Arc<ShinkaiDB>,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
    ) -> Vec<ShinkaiMessage> {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = match db.get_last_unread_messages_from_inbox(inbox_name, limit, offset_key) {
            Ok(messages) => messages,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    format!("Failed to get last unread messages from inbox: {}", e).as_str(),
                );
                return Vec::new();
            }
        };

        result
    }

    pub async fn internal_get_all_inboxes_for_profile(
        identity_manager: Arc<Mutex<IdentityManager>>,
        db: Arc<ShinkaiDB>,
        full_profile_name: ShinkaiName) -> Vec<String> {
        // Obtain the IdentityManager and ShinkaiDB locks
        let identity_manager = identity_manager.lock().await;

        // Find the identity based on the provided name
        let identity = identity_manager
            .search_identity(full_profile_name.full_name.as_str())
            .await;

        // If identity is None (doesn't exist), return an error message
        if identity.is_none() {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Error,
                format!("Failed to find identity for profile: {}", full_profile_name).as_str(),
            );
            return Vec::new();
        }

        drop(identity_manager);
        let identity = identity.unwrap();

        // Check if the found identity is a StandardIdentity. If not, return an empty vector.
        let standard_identity = match &identity {
            Identity::Standard(std_identity) => std_identity.clone(),
            _ => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    format!("Identity for profile: {} is not a StandardIdentity", full_profile_name).as_str(),
                );
                return Vec::new();
            }
        };
        let result = match db.get_inboxes_for_profile(standard_identity) {
            Ok(inboxes) => inboxes,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    format!("Failed to get inboxes for profile: {}", e).as_str(),
                );
                return Vec::new();
            }
        };

        result
    }

    pub async fn internal_update_smart_inbox_name(
        db: Arc<ShinkaiDB>,
        inbox_id: String,
        new_name: String,
    ) -> Result<(), String> {
        match db.update_smart_inbox_name(&inbox_id, &new_name) {
            Ok(_) => Ok(()),
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    format!("Failed to update inbox name: {}", e).as_str(),
                );
                Err(format!("Failed to update inbox name: {}", e))
            }
        }
    }

    pub async fn internal_get_all_smart_inboxes_for_profile(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        full_profile_name: String,
    ) -> Vec<SmartInbox> {
        // Obtain the IdentityManager and ShinkaiDB locks
        let identity_manager = identity_manager.lock().await;

        // Find the identity based on the provided name
        let identity = identity_manager.search_identity(full_profile_name.as_str()).await;

        // If identity is None (doesn't exist), return an error message
        if identity.is_none() {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Error,
                format!("Failed to find identity for profile: {}", full_profile_name).as_str(),
            );
            return Vec::new();
        }

        let identity = identity.unwrap();

        // Check if the found identity is a StandardIdentity. If not, return an empty vector.
        let standard_identity = match &identity {
            Identity::Standard(std_identity) => std_identity.clone(),
            _ => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    format!("Identity for profile: {} is not a StandardIdentity", full_profile_name).as_str(),
                );
                return Vec::new();
            }
        };
        let result = match db.get_all_smart_inboxes_for_profile(standard_identity) {
            Ok(inboxes) => inboxes,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    format!("Failed to get inboxes for profile: {}", e).as_str(),
                );
                return Vec::new();
            }
        };

        result
    }

    pub async fn internal_get_last_messages_from_inbox(
        db: Arc<ShinkaiDB>,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
    ) -> Vec<Vec<ShinkaiMessage>> {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = match db.get_last_messages_from_inbox(inbox_name, limit, offset_key) {
            Ok(messages) => messages,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    format!("Failed to get last messages from inbox: {}", e).as_str(),
                );
                return Vec::new();
            }
        };

        result
    }

    pub async fn send_public_keys(
        identity_public_key: VerifyingKey,
        encryption_public_key: EncryptionPublicKey,
        res: Sender<(VerifyingKey, EncryptionPublicKey)>,
    ) -> Result<(), Error> {
        let identity_public_key = identity_public_key.clone();
        let encryption_public_key = encryption_public_key.clone();
        let _ = res
            .send((identity_public_key, encryption_public_key))
            .await
            .map_err(|_| ());
        Ok(())
    }

    pub async fn fetch_and_send_last_messages(
        db: Arc<ShinkaiDB>,
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    ) -> Result<(), Error> {
        let messages = db.get_last_messages_from_all(limit).unwrap_or_else(|_| vec![]);
        let _ = res.send(messages).await.map_err(|_| ());
        Ok(())
    }

    pub async fn internal_mark_as_read_up_to(
        db: Arc<ShinkaiDB>,
        inbox_name: String,
        up_to_time: String,
    ) -> Result<bool, NodeError> {
        // Attempt to mark messages as read in the database
        db.mark_as_read_up_to(inbox_name, up_to_time).map_err(|e| {
            let error_message = format!("Failed to mark messages as read: {}", e);
            error!("{}", &error_message);
            NodeError { message: error_message }
        })?;
        Ok(true)
    }

    pub async fn has_inbox_permission(
        identity_manager: Arc<Mutex<IdentityManager>>,
        db: Arc<ShinkaiDB>,
        inbox_name: String,
        perm_type: String,
        identity_name: String,
        res: Sender<bool>,
    ) {
        // Obtain the IdentityManager and ShinkaiDB locks
        let identity_manager = identity_manager.lock().await;

        // Find the identity based on the provided name
        let identity = identity_manager.search_identity(&identity_name).await;

        // If identity is None (doesn't exist), return an error message
        if identity.is_none() {
            let _ = res.send(false).await;
            return;
        }

        let identity = identity.unwrap();

        // Check if the found identity is a StandardIdentity. If not, send an error message.
        let standard_identity = match &identity {
            Identity::Standard(std_identity) => std_identity.clone(),
            Identity::Device(std_device) => match std_device.clone().to_standard_identity() {
                Some(identity) => identity,
                None => {
                    let _ = res.send(false).await;
                    return;
                }
            },
            Identity::Agent(_) => {
                let _ = res.send(false).await;
                return;
            }
        };

        let perm = match InboxPermission::from_str(&perm_type) {
            Ok(perm) => perm,
            Err(_) => {
                let _ = res.send(false).await;
                return;
            }
        };

        match db.has_permission(&inbox_name, &standard_identity, perm) {
            Ok(result) => {
                let _ = res.send(result).await;
            }
            Err(_) => {
                let _ = res.send(false).await;
            }
        }
    }

    pub async fn internal_create_new_job(
        job_manager: Arc<Mutex<JobManager>>,
        db: Arc<ShinkaiDB>,
        shinkai_message: ShinkaiMessage,
        sender: Identity,
    ) -> Result<String, NodeError> {
        let mut job_manager = job_manager.lock().await;
        match job_manager.process_job_message(shinkai_message).await {
            Ok(job_id) => {
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
                let sender_standard = match sender {
                    Identity::Standard(std_identity) => std_identity,
                    _ => {
                        return Err(NodeError {
                            message: "Sender is not a StandardIdentity".to_string(),
                        })
                    }
                };
                db.add_permission(&inbox_name.to_string(), &sender_standard, InboxPermission::Admin)?;
                Ok(job_id)
            }
            Err(err) => {
                // If there was an error, return the error
                Err(NodeError::from(err))
            }
        }
    }

    pub async fn internal_get_agents_for_profile(
        db: Arc<ShinkaiDB>,
        node_name: String,
        profile: String,
    ) -> Result<Vec<SerializedAgent>, NodeError> {
        let profile_name = match ShinkaiName::from_node_and_profile_names(node_name, profile) {
            Ok(profile_name) => profile_name,
            Err(e) => {
                return Err(NodeError {
                    message: format!("Failed to create profile name: {}", e),
                })
            }
        };

        let result = match db.get_agents_for_profile(profile_name) {
            Ok(agents) => agents,
            Err(e) => {
                return Err(NodeError {
                    message: format!("Failed to get agents for profile: {}", e),
                })
            }
        };

        Ok(result)
    }

    pub async fn internal_job_message(
        job_manager: Arc<Mutex<JobManager>>,
        shinkai_message: ShinkaiMessage,
    ) -> Result<(), NodeError> {
        let mut job_manager = job_manager.lock().await;
        match job_manager.process_job_message(shinkai_message).await {
            Ok(_) => Ok(()),
            Err(err) => Err(NodeError {
                message: format!("Error with process job message: {}", err),
            }),
        }
    }

    pub async fn internal_add_agent(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        agent: SerializedAgent,
        profile: &ShinkaiName,
    ) -> Result<(), NodeError> {
        match db.add_agent(agent.clone(), profile) {
            Ok(()) => {
                let mut subidentity_manager = identity_manager.lock().await;
                match subidentity_manager.add_agent_subidentity(agent).await {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        error!("Failed to add subidentity: {}", err);
                        Err(NodeError {
                            message: format!("Failed to add device subidentity: {}", err),
                        })
                    }
                }
            }
            Err(e) => Err(NodeError::from(e)),
        }
    }

    pub async fn ping_all(
        node_name: ShinkaiName,
        encryption_secret_key: EncryptionStaticKey,
        identity_secret_key: SigningKey,
        peers: CHashMap<(SocketAddr, String), chrono::DateTime<Utc>>,
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        listen_address: SocketAddr,
    ) -> Result<(), NodeError> {
        info!("{} > Pinging all peers {} ", listen_address, peers.len());
        for (peer, _) in peers.clone() {
            let sender = node_name.get_node_name_string();
            let receiver_profile_identity = identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&peer.1.clone())
                .await
                .unwrap();
            let receiver = receiver_profile_identity.full_identity_name.get_node_name_string();
            let receiver_public_key = receiver_profile_identity.node_encryption_public_key;

            // Important: the receiver doesn't really matter per se as long as it's valid because we are testing the connection
            let _ = ping_pong(
                peer,
                PingPong::Ping,
                clone_static_secret_key(&encryption_secret_key),
                clone_signature_secret_key(&identity_secret_key),
                receiver_public_key,
                sender,
                receiver,
                Arc::clone(&db),
                identity_manager.clone(),
            )
            .await;
        }
        Ok(())
    }

    pub async fn internal_scan_ollama_models() -> Result<Vec<String>, NodeError> {
        let client = reqwest::Client::new();
        let res = client
            .get("http://localhost:11434/api/tags")
            .send()
            .await
            .map_err(|e| NodeError {
                message: format!("Failed to send request: {}", e),
            })?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| NodeError {
                message: format!("Failed to parse response: {}", e),
            })?;

        let models = res["models"].as_array().ok_or_else(|| NodeError {
            message: "Unexpected response format".to_string(),
        })?;

        let names = models
            .iter()
            .filter_map(|model| model["name"].as_str().map(String::from))
            .collect();

        Ok(names)
    }

    pub async fn internal_add_ollama_models(
        db: Arc<ShinkaiDB>,
        node_name: String,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_models: Vec<String>,
    ) -> Result<(), String> {
        {
            db.main_profile_exists(node_name.as_str())
                .map_err(|e| format!("Failed to check if main profile exists: {}", e))?;
        }

        let available_models = Self::internal_scan_ollama_models().await.map_err(|e| e.message)?;

        // Ensure all input models are available
        for model in &input_models {
            if !available_models.contains(model) {
                return Err(format!("Model '{}' is not available.", model));
            }
        }

        // Assuming global_identity is available
        let global_identity = ShinkaiName::from_node_and_profile_names(node_name, "main".to_string()).unwrap();
        let external_url = "http://localhost:11434"; // Common URL for all Ollama models

        let agents: Vec<SerializedAgent> = input_models
            .iter()
            .map(|model| {
                // Replace non-alphanumeric characters with underscores for full_identity_name
                let sanitized_model = Regex::new(r"[^a-zA-Z0-9]").unwrap().replace_all(model, "_").to_string();

                SerializedAgent {
                    id: format!("o_{}", sanitized_model), // Uses the extracted model name as id
                    full_identity_name: ShinkaiName::new(format!(
                        "{}/agent/o_{}",
                        global_identity.full_name, sanitized_model
                    ))
                    .expect("Failed to create ShinkaiName"),
                    perform_locally: false,
                    external_url: Some(external_url.to_string()),
                    api_key: Some("".to_string()),
                    model: AgentLLMInterface::Ollama(Ollama {
                        model_type: model.clone(),
                    }), // Creates the Ollama model
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                }
            })
            .collect();

        // Iterate over each agent and add it using internal_add_agent
        for agent in agents {
            let profile_name = agent.full_identity_name.clone(); // Assuming the profile name is the full identity name of the agent
            Self::internal_add_agent(db.clone(), identity_manager.clone(), agent, &profile_name)
                .await
                .map_err(|e| format!("Failed to add agent: {}", e))?;
        }

        Ok(())
    }
}
