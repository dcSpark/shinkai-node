use crate::llm_provider::job_manager::JobManager;
use crate::managers::identity_manager::IdentityManagerTrait;
use crate::managers::IdentityManager;
use crate::network::network_manager::network_handlers::{ping_pong, PingPong};
use crate::network::node::ProxyConnectionInfo;
use crate::network::node_error::NodeError;
use crate::network::Node;

use async_channel::Sender;
use chashmap::CHashMap;
use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use log::{error, info};
use regex::Regex;
use shinkai_fs;

use shinkai_message_primitives::schemas::identity::{Identity, StandardIdentity};
use shinkai_message_primitives::schemas::inbox_permission::InboxPermission;
use shinkai_message_primitives::schemas::smart_inbox::SmartInbox;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        llm_providers::serialized_llm_provider::{LLMProviderInterface, Ollama, SerializedLLMProvider},
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::{
        encryption::clone_static_secret_key,
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::clone_signature_secret_key,
    },
};
use shinkai_sqlite::SqliteManager;
use std::{io::Error, net::SocketAddr};
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
        db: Arc<SqliteManager>,
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
        db: Arc<SqliteManager>,
        full_profile_name: ShinkaiName,
    ) -> Vec<String> {
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
        db: Arc<SqliteManager>,
        inbox_id: String,
        new_name: String,
    ) -> Result<(), String> {
        // Parse the inbox name to check if it's a job inbox
        let inbox_name = InboxName::new(inbox_id.clone()).map_err(|e| format!("Failed to parse inbox name: {}", e))?;
        
        // Get the job ID if it's a job inbox
        let job_id = inbox_name.get_job_id();
        
        if let Some(job_id) = job_id {
            // Get the current folder name before updating
            let old_folder = db.get_job_folder_name(&job_id).map_err(|e| format!("Failed to get old folder name: {}", e))?;
            
            // Update the inbox name
            db.unsafe_update_smart_inbox_name(&inbox_id, &new_name)
                .map_err(|e| format!("Failed to update inbox name: {}", e))?;
            
            // Get the new folder name after updating
            let new_folder = db.get_job_folder_name(&job_id).map_err(|e| format!("Failed to get new folder name: {}", e))?;
            
            // Move the folder if it exists
            if old_folder.exists() {
                use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
                ShinkaiFileManager::move_folder(old_folder, new_folder, &db)
                    .map_err(|e| format!("Failed to move folder: {}", e))?;
            }
            
            Ok(())
        } else {
            // If it's not a job inbox, just update the name
            db.unsafe_update_smart_inbox_name(&inbox_id, &new_name)
                .map_err(|e| format!("Failed to update inbox name: {}", e))
        }
    }

    pub async fn internal_get_all_smart_inboxes_for_profile(
        db: Arc<SqliteManager>,
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
        db: Arc<SqliteManager>,
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
        let _ = res
            .send((identity_public_key, encryption_public_key))
            .await
            .map_err(|_| ());
        Ok(())
    }

    pub async fn fetch_and_send_last_messages(
        db: Arc<SqliteManager>,
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    ) -> Result<(), Error> {
        let messages = db.get_last_messages_from_all(limit).unwrap_or_else(|_| vec![]);
        let _ = res.send(messages).await.map_err(|_| ());
        Ok(())
    }

    pub async fn internal_mark_as_read_up_to(
        db: Arc<SqliteManager>,
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
        db: Arc<SqliteManager>,
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
            Identity::LLMProvider(_) => {
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
        db: Arc<SqliteManager>,
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

    pub async fn internal_get_llm_providers_for_profile(
        db: Arc<SqliteManager>,
        node_name: String,
        profile: String,
    ) -> Result<Vec<SerializedLLMProvider>, NodeError> {
        let profile_name = match ShinkaiName::from_node_and_profile_names(node_name, profile) {
            Ok(profile_name) => profile_name,
            Err(e) => {
                return Err(NodeError {
                    message: format!("Failed to create profile name: {}", e),
                })
            }
        };
        let result = match db.get_llm_providers_for_profile(profile_name) {
            Ok(llm_providers) => llm_providers,
            Err(e) => {
                return Err(NodeError {
                    message: format!("Failed to get llm providers for profile: {}", e),
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

    pub async fn internal_add_llm_provider(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        llm_provider: SerializedLLMProvider,
        profile: &ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), NodeError> {
        match db.add_llm_provider(llm_provider.clone(), profile) {
            Ok(()) => {
                let mut subidentity_manager = identity_manager.lock().await;
                match subidentity_manager
                    .add_llm_provider_subidentity(llm_provider.clone())
                    .await
                {
                    Ok(_) => {
                        drop(subidentity_manager);

                        let (has_job_inbox, welcome_message) = if profile.has_agent() {
                            (false, false)
                        } else {
                            let inboxes = Self::internal_get_all_inboxes_for_profile(
                                identity_manager.clone(),
                                db.clone(),
                                profile.clone(),
                            )
                            .await;

                            let has_job_inbox = inboxes.iter().any(|inbox| inbox.starts_with("job_inbox"));
                            let welcome_message =
                                std::env::var("WELCOME_MESSAGE").unwrap_or("true".to_string()) == "true";
                            (has_job_inbox, welcome_message)
                        };

                        if !has_job_inbox && welcome_message {
                            let shinkai_folder_fs = ShinkaiPath::from_string("/My Files (Private)".to_string());

                            let job_scope = MinimalJobScope {
                                vector_fs_items: vec![],
                                vector_fs_folders: vec![shinkai_folder_fs],
                                vector_search_mode: vec![],
                            };
                            let job_creation = JobCreationInfo {
                                scope: job_scope,
                                is_hidden: Some(false),
                                associated_ui: None,
                            };

                            let mut job_manager_locked = job_manager.lock().await;
                            let job_id = match job_manager_locked
                                .process_job_creation(job_creation, profile, &llm_provider.id.clone())
                                .await
                            {
                                Ok(job_id) => job_id,
                                Err(err) => {
                                    return Err(NodeError {
                                        message: format!("Failed to create job: {}", err),
                                    })
                                }
                            };

                            let subidentity_manager = identity_manager.lock().await;
                            let sender = subidentity_manager.search_identity(&profile.full_name).await.unwrap();
                            let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone())?.to_string();
                            let sender_standard = match sender {
                                Identity::Standard(std_identity) => std_identity,
                                _ => {
                                    return Err(NodeError {
                                        message: "Sender is not a StandardIdentity".to_string(),
                                    })
                                }
                            };
                            db.add_permission(&inbox_name.to_string(), &sender_standard, InboxPermission::Admin)?;
                            db.unsafe_update_smart_inbox_name(
                                &inbox_name.to_string(),
                                "Welcome to Shinkai! Brief onboarding here.",
                            )?;

                            // TODO: add back later
                            // {
                            //     // Add Two Message from "Agent"
                            //     let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);

                            //     let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                            //         job_id.to_string(),
                            //         WELCOME_MESSAGE.to_string(),
                            //         "".to_string(),
                            //         None,
                            //         identity_secret_key_clone,
                            //         profile.node_name.clone(),
                            //         profile.node_name.clone(),
                            //     )
                            //     .unwrap();

                            //     db.add_message_to_job_inbox(&job_id.clone(), &shinkai_message, None, ws_manager)
                            //         .await?;
                            // }
                        }
                        Ok(())
                    }
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

    #[allow(dead_code)]
    pub async fn internal_remove_agent(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        agent_id: String,
        profile: &ShinkaiName,
    ) -> Result<(), NodeError> {
        match db.remove_llm_provider(&agent_id, profile) {
            Ok(()) => {
                let mut subidentity_manager = identity_manager.lock().await;
                match subidentity_manager.remove_agent_subidentity(&agent_id).await {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        error!("Failed to remove subidentity: {}", err);
                        Err(NodeError {
                            message: format!("Failed to remove device subidentity: {}", err),
                        })
                    }
                }
            }
            Err(e) => Err(NodeError::from(e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn ping_all(
        node_name: ShinkaiName,
        encryption_secret_key: EncryptionStaticKey,
        identity_secret_key: SigningKey,
        peers: CHashMap<(SocketAddr, String), chrono::DateTime<Utc>>,
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        listen_address: SocketAddr,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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
                proxy_connection_info.clone(),
                ws_manager.clone(),
            )
            .await;
        }
        Ok(())
    }

    pub async fn internal_scan_ollama_models() -> Result<Vec<serde_json::Value>, NodeError> {
        let urls = vec!["http://localhost:11434/api/tags", "http://localhost:11435/api/tags"];
        let client = reqwest::Client::new();
        let mut all_models = Vec::new();

        for url in urls {
            let res = client.get(url).send().await;

            match res {
                Ok(response) => match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let models = json["models"].as_array().ok_or_else(|| NodeError {
                            message: format!("Unexpected response format from {}", url),
                        })?;

                        let models_with_port: Vec<serde_json::Value> = models
                            .iter()
                            .map(|model| {
                                let mut model_clone = model.clone();
                                if let Some(obj) = model_clone.as_object_mut() {
                                    let port = url.splitn(3, ':').nth(2).unwrap_or("").split('/').next().unwrap_or("");
                                    obj.insert("port_used".to_string(), serde_json::Value::String(port.to_string()));
                                }
                                model_clone
                            })
                            .collect();

                        all_models.extend(models_with_port);
                    }
                    Err(e) => {
                        log::error!("Failed to parse response from {}: {}", url, e);
                    }
                },
                Err(e) => {
                    log::error!("Failed to send request to {}: {}", url, e);
                }
            }
        }

        if all_models.is_empty() {
            Err(NodeError {
                message: "No models could be retrieved from any source.".to_string(),
            })
        } else {
            Ok(all_models)
        }
    }

    pub async fn internal_add_ollama_models(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        input_models: Vec<String>,
        shinkai_name: ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), String> {
        let requester_profile = shinkai_name.extract_profile().unwrap_or_else(|_| {
            ShinkaiName::from_node_and_profile_names(shinkai_name.node_name, "main".to_string())
                .expect("Failed to create main profile ShinkaiName")
        });

        let available_models = Self::internal_scan_ollama_models().await.map_err(|e| e.message)?;

        // Ensure all input models are available
        for model in &input_models {
            if !available_models.iter().any(|m| m["name"].as_str() == Some(model)) {
                return Err(format!("Model '{}' is not available.", model));
            }
        }

        let llm_providers: Vec<SerializedLLMProvider> = input_models
            .iter()
            .map(|model| {
                // Replace non-alphanumeric characters with underscores for full_identity_name
                let sanitized_model = Regex::new(r"[^a-zA-Z0-9]").unwrap().replace_all(model, "_").to_string();

                // Determine which URL to use based on the availability of the models
                let model_data = available_models
                    .iter()
                    .find(|m| m["name"].as_str() == Some(model))
                    .unwrap();
                let external_url = format!(
                    "http://localhost:{}",
                    model_data["port_used"].as_str().unwrap_or("11434")
                );

                SerializedLLMProvider {
                    id: format!("o_{}", sanitized_model), // Uses the extracted model name as id
                    full_identity_name: ShinkaiName::new(format!(
                        "{}/agent/o_{}",
                        requester_profile.full_name, sanitized_model
                    ))
                    .expect("Failed to create ShinkaiName"),
                    external_url: Some(external_url.to_string()),
                    api_key: Some("".to_string()),
                    model: LLMProviderInterface::Ollama(Ollama {
                        model_type: model.clone(),
                    }),
                }
            })
            .collect();

        // Iterate over each agent and add it using internal_add_agent
        for agent in llm_providers {
            let profile_name = agent.full_identity_name.clone(); // Assuming the profile name is the full identity name of the agent
            Self::internal_add_llm_provider(
                db.clone(),
                identity_manager.clone(),
                job_manager.clone(),
                identity_secret_key.clone(),
                agent,
                &profile_name,
                ws_manager.clone(),
            )
            .await
            .map_err(|e| format!("Failed to add agent: {}", e))?;
        }

        Ok(())
    }
}

