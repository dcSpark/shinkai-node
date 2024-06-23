use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::Aead;
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_trait::async_trait;
use futures::stream::SplitSink;
use futures::SinkExt;
use serde::Deserialize;
use serde::Serialize;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::collections::VecDeque;
use std::fmt;
use std::sync::Weak;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tokio::time::sleep;
use warp::ws::Message;
use warp::ws::WebSocket;

use crate::db::ShinkaiDB;
use crate::schemas::identity::Identity;

use super::node_shareable_logic::validate_message_main_logic;
use super::Node;
use crate::managers::identity_manager::IdentityManagerTrait;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    ShinkaiMessage,
    Stream,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSMessagePayload {
    pub message_type: MessageType,
    pub inbox: String,
    pub message: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug)]
pub enum WebSocketManagerError {
    UserValidationFailed(String),
    AccessDenied(String),
    MissingSharedKey(String),
}

impl fmt::Display for WebSocketManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WebSocketManagerError::UserValidationFailed(msg) => write!(f, "User validation failed: {}", msg),
            WebSocketManagerError::AccessDenied(msg) => write!(f, "Access denied: {}", msg),
            WebSocketManagerError::MissingSharedKey(msg) => write!(f, "Missing shared key: {}", msg),
        }
    }
}

impl fmt::Debug for WebSocketManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSocketManager")
            .field("connections", &self.connections.keys()) // Only print the keys
            .field("subscriptions", &self.subscriptions)
            .field("shinkai_db", &self.shinkai_db)
            .field("node_name", &self.node_name)
            .field("identity_manager_trait", &"Box<dyn IdentityManagerTrait + Send>") // Print a placeholder
            .finish()
    }
}

#[async_trait]
pub trait WSUpdateHandler {
    async fn queue_message(&self, topic: WSTopic, subtopic: String, update: String, is_stream: bool);
}

pub struct WebSocketManager {
    connections: HashMap<String, Arc<Mutex<SplitSink<WebSocket, Message>>>>,
    // TODO: maybe the first string should be a ShinkaiName? or at least a shinkai name string
    subscriptions: HashMap<String, HashMap<String, bool>>,
    shared_keys: HashMap<String, String>,
    shinkai_db: Weak<ShinkaiDB>,
    node_name: ShinkaiName,
    identity_manager_trait: Arc<Mutex<Box<dyn IdentityManagerTrait + Send>>>,
    message_queue: Arc<Mutex<VecDeque<(WSTopic, String, String, bool)>>>,
}

impl Clone for WebSocketManager {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            subscriptions: self.subscriptions.clone(),
            shared_keys: self.shared_keys.clone(),
            shinkai_db: self.shinkai_db.clone(),
            node_name: self.node_name.clone(),
            identity_manager_trait: Arc::clone(&self.identity_manager_trait),
            message_queue: Arc::clone(&self.message_queue),
        }
    }
}

impl WebSocketManager {
    pub async fn new(
        shinkai_db: Weak<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager_trait: Arc<Mutex<Box<dyn IdentityManagerTrait + Send>>>,
    ) -> Arc<Mutex<Self>> {
        let manager = Arc::new(Mutex::new(Self {
            connections: HashMap::new(),
            subscriptions: HashMap::new(),
            shared_keys: HashMap::new(),
            shinkai_db,
            node_name,
            identity_manager_trait,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
        }));

        let manager_clone = Arc::clone(&manager);

        // Spawn the message sender task
        let message_queue_clone = Arc::clone(&manager.lock().await.message_queue);
        tokio::spawn(Self::start_message_sender(manager_clone, message_queue_clone));

        manager
    }

    pub async fn start_message_sender(
        manager: Arc<Mutex<Self>>,
        message_queue: Arc<Mutex<VecDeque<(WSTopic, String, String, bool)>>>,
    ) {
        loop {
            // Sleep for a while
            sleep(Duration::from_millis(200)).await;

            // Check if there are any messages in the queue
            let message = {
                let mut queue = message_queue.lock().await;
                queue.pop_front()
            };

            if let Some((topic, subtopic, update, bool)) = message {
                shinkai_log(
                    ShinkaiLogOption::WsAPI,
                    ShinkaiLogLevel::Debug,
                    format!("Sending update to topic: {}", topic).as_str(),
                );
                manager.lock().await.handle_update(topic, subtopic, update, bool).await;
            }
        }
    }

    pub async fn user_validation(&self, shinkai_name: ShinkaiName, message: &ShinkaiMessage) -> bool {
        // Message can't be encrypted at this point
        let is_body_encrypted = message.clone().is_body_currently_encrypted();
        if is_body_encrypted {
            shinkai_log(
                ShinkaiLogOption::DetailedAPI,
                ShinkaiLogLevel::Debug,
                format!("Message body is encrypted, can't validate user: {}", shinkai_name).as_str(),
            );
            return false;
        }

        // Note: we generate a dummy encryption key because the message is not encrypted so we don't need the real key.
        let (dummy_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);

        let identity_manager_clone = self.identity_manager_trait.clone();
        let result = validate_message_main_logic(
            &dummy_encryption_sk,
            identity_manager_clone,
            &shinkai_name.clone(),
            message.clone(),
            None,
        )
        .await;

        result.is_ok()
    }

    pub async fn has_access(&self, shinkai_name: ShinkaiName, topic: WSTopic, subtopic: Option<String>) -> bool {
        match topic {
            WSTopic::Inbox => {
                let subtopic = subtopic.unwrap_or_default();
                let inbox_name = InboxName::new(subtopic.clone()).unwrap(); // TODO: handle error
                let sender_identity = match self.get_sender_identity(shinkai_name.clone()).await {
                    Ok(identity) => identity,
                    Err(_) => return false,
                };
                let db_arc = self.shinkai_db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
                match Node::has_inbox_access(db_arc, &inbox_name, &sender_identity).await {
                    Ok(value) => {
                        if value {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Debug,
                                format!(
                                    "Access granted for inbox: {} and sender_subidentity: {}",
                                    inbox_name, shinkai_name.full_name
                                )
                                .as_str(),
                            );
                        } else {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Debug,
                                format!(
                                    "Access denied for inbox: {} and sender_subidentity: {}",
                                    inbox_name, shinkai_name.full_name
                                )
                                .as_str(),
                            );
                        }
                        value
                    }
                    Err(_) => {
                        shinkai_log(
                            ShinkaiLogOption::WsAPI,
                            ShinkaiLogLevel::Error,
                            format!(
                                "Access denied for inbox: {} and sender_subidentity: {}",
                                inbox_name, shinkai_name.full_name
                            )
                            .as_str(),
                        );
                        false
                    }
                }
            }
            WSTopic::SmartInboxes => {
                // Note: everyone has access to their inboxes.
                // But we need to be careful about *just* sharing their inboxes.
                true
            }
        }
    }

    pub async fn manage_connections(
        &mut self,
        shinkai_name: ShinkaiName,
        message: ShinkaiMessage,
        connection: Arc<Mutex<SplitSink<WebSocket, Message>>>,
        ws_message: WSMessage,
    ) -> Result<(), WebSocketManagerError> {
        shinkai_log(
            ShinkaiLogOption::WsAPI,
            ShinkaiLogLevel::Info,
            format!("Adding connection for shinkai_name: {}", shinkai_name).as_str(),
        );

        if !self.user_validation(shinkai_name.clone(), &message).await {
            shinkai_log(
                ShinkaiLogOption::WsAPI,
                ShinkaiLogLevel::Error,
                format!("User validation failed for shinkai_name: {}", shinkai_name).as_str(),
            );
            return Err(WebSocketManagerError::UserValidationFailed(format!(
                "User validation failed for shinkai_name: {}",
                shinkai_name
            )));
        }

        let shinkai_profile_name = shinkai_name.to_string();
        let shared_key = ws_message.shared_key.clone();

        // Initialize the topic map for the new connection
        let mut topic_map = HashMap::new();

        // Iterate over the subscriptions to check access and add them to the topic map
        for subscription in ws_message.subscriptions.iter() {
            if !self
                .has_access(
                    shinkai_name.clone(),
                    subscription.topic.clone(),
                    subscription.subtopic.clone(),
                )
                .await
            {
                eprintln!(
                    "Access denied for shinkai_name: {} on topic: {:?} and subtopic: {:?}",
                    shinkai_name, subscription.topic, subscription.subtopic
                );
                // TODO: should we send a ShinkaiMessage with an error inside back?
                return Err(WebSocketManagerError::AccessDenied(format!(
                    "Access denied for shinkai_name: {} on topic: {:?} and subtopic: {:?}",
                    shinkai_name, subscription.topic, subscription.subtopic
                )));
            }

            let topic_subtopic = format!(
                "{}:::{}",
                subscription.topic,
                subscription.subtopic.clone().unwrap_or_default()
            );
            topic_map.insert(topic_subtopic, true);
        }

        // Add the connection and shared key to the manager
        self.connections.insert(shinkai_profile_name.clone(), connection);

        if let Some(key) = shared_key {
            self.shared_keys.insert(shinkai_profile_name.clone(), key);
        } else if !self.shared_keys.contains_key(&shinkai_profile_name) {
            return Err(WebSocketManagerError::MissingSharedKey(format!(
                "Missing shared key for shinkai_name: {}",
                shinkai_profile_name
            )));
        }

        // Handle adding and removing subscriptions
        let subscriptions_to_add: Vec<(WSTopic, Option<String>)> = ws_message
            .subscriptions
            .iter()
            .map(|s| (s.topic.clone(), s.subtopic.clone()))
            .collect();
        let subscriptions_to_remove: Vec<(WSTopic, Option<String>)> = ws_message
            .unsubscriptions
            .iter()
            .map(|s| (s.topic.clone(), s.subtopic.clone()))
            .collect();
        self.update_subscriptions(&shinkai_profile_name, subscriptions_to_add, subscriptions_to_remove)
            .await;

        shinkai_log(
            ShinkaiLogOption::WsAPI,
            ShinkaiLogLevel::Info,
            format!("Successfully added connection for shinkai_name: {}", shinkai_name).as_str(),
        );

        Ok(())
    }

    // Method to update subscriptions
    pub async fn update_subscriptions(
        &mut self,
        shinkai_name: &str,
        subscriptions_to_add: Vec<(WSTopic, Option<String>)>,
        subscriptions_to_remove: Vec<(WSTopic, Option<String>)>,
    ) {
        // We already checked that the user is allowed to have those subscriptions
        let profile_subscriptions = self.subscriptions.entry(shinkai_name.to_string()).or_default();

        // Add new subscriptions
        for (topic, subtopic) in subscriptions_to_add {
            let key = format!("{}:::{}", topic, subtopic.unwrap_or_default());
            profile_subscriptions.insert(key, true);
        }

        // Remove specified subscriptions
        for (topic, subtopic) in subscriptions_to_remove {
            let key = format!("{}:::{}", topic, subtopic.unwrap_or_default());
            profile_subscriptions.remove(&key);
        }

        let current_subscriptions: Vec<String> = profile_subscriptions.keys().cloned().collect();
        shinkai_log(
            ShinkaiLogOption::WsAPI,
            ShinkaiLogLevel::Info,
            format!("current_subscriptions: {:?}", current_subscriptions).as_str(),
        );
    }

    pub async fn handle_update(&self, topic: WSTopic, subtopic: String, update: String, is_stream: bool) {
        let topic_subtopic = format!("{}:::{}", topic, subtopic);
        shinkai_log(
            ShinkaiLogOption::WsAPI,
            ShinkaiLogLevel::Debug,
            format!("Sending update to topic: {}", topic_subtopic).as_str(),
        );

        // Create the WSMessagePayload
        let payload = WSMessagePayload {
            message_type: if is_stream { MessageType::Stream } else { MessageType::ShinkaiMessage },
            inbox: subtopic.clone(),
            message: Some(update.clone()),
            error_message: None,
        };

        // Serialize the payload to JSON
        let payload_json = serde_json::to_string(&payload).expect("Failed to serialize WSMessagePayload");

        // Send the update to all active connections that are subscribed to the topic
        for (id, connection) in self.connections.iter() {
            let is_subscribed_to_smart_inboxes = self
                .subscriptions
                .get(id)
                .unwrap()
                .get(&format!("{}:::{}", WSTopic::SmartInboxes, ""))
                .is_some();
            let is_subscribed_to_topic = self.subscriptions.get(id).unwrap().get(&topic_subtopic).is_some();

            if is_subscribed_to_smart_inboxes || is_subscribed_to_topic {
                // If the user is subscribed to SmartInboxes, check if they have access to the specific inbox
                if is_subscribed_to_smart_inboxes {
                    match ShinkaiName::new(id.clone()) {
                        Ok(shinkai_name) => {
                            let shinkai_name_clone = shinkai_name.clone();
                            if !self
                                .has_access(shinkai_name_clone, topic.clone(), Some(subtopic.clone()))
                                .await
                            {
                                continue;
                            }
                            eprintln!(
                                "Access granted for shinkai_name: {} on topic: {:?} and subtopic: {:?}",
                                shinkai_name, topic, subtopic
                            );
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Error,
                                format!("Failed to create ShinkaiName for id {}: {}", id, e).as_str(),
                            );
                            continue;
                        }
                    }
                }

                let mut connection = connection.lock().await;

                // Encrypt the update using the shared key
                let shared_key = self.shared_keys.get(id).unwrap();
                let shared_key_bytes = hex::decode(shared_key).expect("Failed to decode shared key");
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&shared_key_bytes));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let encrypted_update = cipher.encrypt(nonce, payload_json.as_ref()).expect("encryption failure!");
                let encrypted_update_hex = hex::encode(&encrypted_update);

                match connection.send(Message::text(encrypted_update_hex.clone())).await {
                    Ok(_) => shinkai_log(
                        ShinkaiLogOption::WsAPI,
                        ShinkaiLogLevel::Info,
                        format!("Successfully sent update to connection {}", id).as_str(),
                    ),
                    Err(e) => shinkai_log(
                        ShinkaiLogOption::WsAPI,
                        ShinkaiLogLevel::Error,
                        format!("Failed to send update to connection {}: {}", id, e).as_str(),
                    ),
                }
            } else {
                shinkai_log(
                    ShinkaiLogOption::WsAPI,
                    ShinkaiLogLevel::Debug,
                    format!("Connection {} is not subscribed to the topic {:?}", id, topic_subtopic).as_str(),
                );
            }
        }
    }

    pub async fn get_sender_identity(&self, shinkai_name: ShinkaiName) -> Result<Identity, WebSocketManagerError> {
        let identity_manager_lock = self.identity_manager_trait.lock().await;
        match identity_manager_lock.find_by_identity_name(shinkai_name.clone()) {
            Some(identity) => Ok(identity.clone()),
            None => {
                shinkai_log(
                    ShinkaiLogOption::WsAPI,
                    ShinkaiLogLevel::Error,
                    format!("No identity found for shinkai_name: {}", shinkai_name).as_str(),
                );
                Err(WebSocketManagerError::UserValidationFailed(format!(
                    "No identity found for shinkai_name: {}",
                    shinkai_name
                )))
            }
        }
    }
}

#[async_trait]
impl WSUpdateHandler for WebSocketManager {
    async fn queue_message(&self, topic: WSTopic, subtopic: String, update: String, is_stream: bool) {
        let mut queue = self.message_queue.lock().await;
        queue.push_back((topic, subtopic, update, is_stream));
    }
}
