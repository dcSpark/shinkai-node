use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::Aead;
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_trait::async_trait;
use futures::stream::SplitSink;
use futures::SinkExt;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::identity::Identity;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::ws_types::MessageQueue;
use shinkai_message_primitives::schemas::ws_types::MessageType;
use shinkai_message_primitives::schemas::ws_types::WSMessagePayload;
use shinkai_message_primitives::schemas::ws_types::WSMessageType;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::schemas::ws_types::WebSocketManagerError;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
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
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use super::node_shareable_logic::validate_message_main_logic;
use super::Node;
use crate::managers::identity_manager::IdentityManagerTrait;

pub struct WebSocketManager {
    connections: HashMap<String, Arc<Mutex<SplitSink<WebSocket, Message>>>>,
    // TODO: maybe the first string should be a ShinkaiName? or at least a shinkai name string
    subscriptions: HashMap<String, HashMap<String, bool>>,
    shared_keys: HashMap<String, String>,
    shinkai_db: Weak<ShinkaiDB>,
    node_name: ShinkaiName,
    identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
    encryption_secret_key: EncryptionStaticKey,
    message_queue: MessageQueue,
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
            encryption_secret_key: self.encryption_secret_key.clone(),
            message_queue: Arc::clone(&self.message_queue),
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

impl WebSocketManager {
    pub async fn new(
        shinkai_db: Weak<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        encryption_secret_key: EncryptionStaticKey,
    ) -> Arc<Mutex<Self>> {
        let manager = Arc::new(Mutex::new(Self {
            connections: HashMap::new(),
            subscriptions: HashMap::new(),
            shared_keys: HashMap::new(),
            shinkai_db,
            node_name,
            identity_manager_trait,
            encryption_secret_key,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
        }));

        let manager_clone = Arc::clone(&manager);

        // Spawn the message sender task
        let message_queue_clone = Arc::clone(&manager.lock().await.message_queue);
        tokio::spawn(Self::start_message_sender(manager_clone, message_queue_clone));

        manager
    }

    pub async fn start_message_sender(manager: Arc<Mutex<Self>>, message_queue: MessageQueue) {
        loop {
            let message = {
                let mut queue = message_queue.lock().await;
                queue.pop_front()
            };

            match message {
                Some((topic, subtopic, update, metadata, is_stream)) => {
                    shinkai_log(
                        ShinkaiLogOption::WsAPI,
                        ShinkaiLogLevel::Debug,
                        format!("Sending update to topic: {}", topic).as_str(),
                    );
                    manager
                        .lock()
                        .await
                        .handle_update(topic, subtopic, update, metadata, is_stream)
                        .await;
                }
                None => {
                    // Sleep only when there are no messages in the queue
                    sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }

    pub async fn user_validation(
        &self,
        shinkai_name: ShinkaiName,
        message: &ShinkaiMessage,
    ) -> Result<(ShinkaiMessage, Identity), APIError> {
        let cloned_enc_sk = clone_static_secret_key(&self.encryption_secret_key);
        let identity_manager_clone = self.identity_manager_trait.clone();
        validate_message_main_logic(
            &cloned_enc_sk,
            identity_manager_clone,
            &shinkai_name.clone(),
            message.clone(),
            None,
        )
        .await
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
            WSTopic::Sheet => true,
            WSTopic::SheetList => true,
            WSTopic::Widget => true,
        }
    }

    pub async fn manage_connections(
        &mut self,
        sender_shinkai_name: ShinkaiName,
        potentially_encrypted_msg: ShinkaiMessage,
        connection: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    ) -> Result<(), WebSocketManagerError> {
        shinkai_log(
            ShinkaiLogOption::WsAPI,
            ShinkaiLogLevel::Info,
            format!("Adding connection for shinkai_name: {}", sender_shinkai_name).as_str(),
        );

        let validation_result = self
            .user_validation(self.node_name.clone(), &potentially_encrypted_msg)
            .await;
        let (validated_message, _sender_identity) = match validation_result {
            Ok((msg, identity)) => (msg, identity),
            Err(api_error) => {
                shinkai_log(
                    ShinkaiLogOption::WsAPI,
                    ShinkaiLogLevel::Error,
                    format!(
                        "User validation failed for shinkai_name: {}: {:?}",
                        sender_shinkai_name, api_error
                    )
                    .as_str(),
                );
                return Err(WebSocketManagerError::UserValidationFailed(format!(
                    "User validation failed for shinkai_name: {}: {:?}",
                    sender_shinkai_name, api_error
                )));
            }
        };

        let sender_shinkai_name =
            ShinkaiName::from_shinkai_message_using_sender_subidentity(&validated_message.clone()).map_err(|e| {
                WebSocketManagerError::UserValidationFailed(format!("Failed to get ShinkaiName: {}", e))
            })?;

        let content_str = validated_message.get_message_content().map_err(|e| {
            WebSocketManagerError::UserValidationFailed(format!("Failed to get message content: {}", e))
        })?;

        let ws_message = serde_json::from_str::<WSMessage>(&content_str).map_err(|e| {
            WebSocketManagerError::UserValidationFailed(format!("Failed to deserialize WSMessage: {}", e))
        })?;

        // eprintln!("ws_message: {:?}", ws_message);

        // Validate shared_key if it exists
        if let Some(shared_key) = &ws_message.shared_key {
            if !Self::is_valid_hex_key(shared_key) {
                return Err(WebSocketManagerError::InvalidSharedKey(
                    "Provided shared_key is not a valid hexadecimal string".to_string(),
                ));
            }
        }

        // Decrypt Message

        let shinkai_profile_name = sender_shinkai_name.to_string();
        let shared_key = ws_message.shared_key.clone();

        // Initialize the topic map for the new connection
        let mut topic_map = HashMap::new();

        // Iterate over the subscriptions to check access and add them to the topic map
        for subscription in ws_message.subscriptions.iter() {
            if !self
                .has_access(
                    sender_shinkai_name.clone(),
                    subscription.topic.clone(),
                    subscription.subtopic.clone(),
                )
                .await
            {
                eprintln!(
                    "Access denied for shinkai_name: {} on topic: {:?} and subtopic: {:?}",
                    sender_shinkai_name, subscription.topic, subscription.subtopic
                );
                // TODO: should we send a ShinkaiMessage with an error inside back?
                return Err(WebSocketManagerError::AccessDenied(format!(
                    "Access denied for shinkai_name: {} on topic: {:?} and subtopic: {:?}",
                    sender_shinkai_name, subscription.topic, subscription.subtopic
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
        }
        // Note: uncomment this enforce having a shared encryption key
        // else if !self.shared_keys.contains_key(&shinkai_profile_name) {
        //     return Err(WebSocketManagerError::MissingSharedKey(format!(
        //         "Missing shared key for shinkai_name: {}",
        //         shinkai_profile_name
        //     )));
        // }

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
            format!(
                "Successfully added connection for shinkai_name: {}",
                sender_shinkai_name
            )
            .as_str(),
        );

        Ok(())
    }

    fn is_valid_hex_key(key: &str) -> bool {
        // Check if the key is a valid hexadecimal string
        if key.len() != 64 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }

        // Attempt to decode the key
        match hex::decode(key) {
            Ok(decoded) => decoded.len() == 32, // 32 bytes for AES-256
            Err(_) => false,
        }
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

    pub async fn handle_update(
        &self,
        topic: WSTopic,
        subtopic: String,
        update: String,
        metadata: WSMessageType,
        is_stream: bool,
    ) {
        let topic_subtopic = format!("{}:::{}", topic, subtopic);
        shinkai_log(
            ShinkaiLogOption::WsAPI,
            ShinkaiLogLevel::Debug,
            format!("Sending update to topic: {}", topic_subtopic).as_str(),
        );

        // Determine the message type
        let message_type = match metadata {
            WSMessageType::Sheet(_) => MessageType::Sheet,
            WSMessageType::Widget(_) => MessageType::Widget,
            _ => {
                if is_stream {
                    MessageType::Stream
                } else {
                    MessageType::ShinkaiMessage
                }
            }
        };

        // Create the WSMessagePayload
        let payload = WSMessagePayload {
            message_type,
            inbox: subtopic.clone(),
            message: Some(update.clone()),
            error_message: None,
            metadata: match metadata.clone() {
                WSMessageType::Metadata(meta) => Some(meta),
                _ => None,
            },
            widget: match metadata {
                WSMessageType::Widget(widget_metadata) => {
                    Some(serde_json::to_value(widget_metadata).expect("Failed to serialize WidgetMetadata"))
                }
                _ => None,
            },
            is_stream,
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

            let is_subscribed_to_sheets = self
                .subscriptions
                .get(id)
                .unwrap()
                .get(&format!("{}:::{}", WSTopic::Sheet, ""))
                .is_some();

            if is_subscribed_to_smart_inboxes
                || is_subscribed_to_topic
                || (is_subscribed_to_sheets && topic == WSTopic::Sheet)
            {
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

                let message_to_send = if let Some(shared_key) = self.shared_keys.get(id) {
                    // Encrypt the update using the shared key
                    let shared_key_bytes = match hex::decode(shared_key) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Error,
                                format!("Failed to decode shared key for connection {}: {}", id, e).as_str(),
                            );
                            continue;
                        }
                    };
                    let cipher = Aes256Gcm::new(GenericArray::from_slice(&shared_key_bytes));
                    let nonce = GenericArray::from_slice(&[0u8; 12]);
                    let encrypted_update = cipher
                        .encrypt(nonce, payload_json.as_ref())
                        .expect("encryption failure!");
                    hex::encode(&encrypted_update)
                } else {
                    // If no shared key, send the message without encryption
                    payload_json.clone()
                };

                match connection.send(Message::text(message_to_send.clone())).await {
                    Ok(_) => shinkai_log(
                        ShinkaiLogOption::WsAPI,
                        ShinkaiLogLevel::Debug,
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
    async fn queue_message(
        &self,
        topic: WSTopic,
        subtopic: String,
        update: String,
        metadata: WSMessageType,
        is_stream: bool,
    ) {
        let mut queue = self.message_queue.lock().await;
        queue.push_back((topic, subtopic, update, metadata, is_stream));
    }
}
