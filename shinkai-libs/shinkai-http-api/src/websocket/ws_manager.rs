use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, RwLock, Weak};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use async_trait::async_trait;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use warp::ws::{Message, WebSocket};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use shinkai_message_primitives::{
    schemas::{
        identity::Identity,
        inbox_name::InboxName,
        shinkai_name::ShinkaiName,
        ws_types::{MessageQueue, MessageType, WSMessagePayload, WSMessageType, WebSocketManagerError, WSUpdateHandler},
    },
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{WSTopic, WSMessage},
    },
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};

use crate::{
    error::APIError,
    identity::IdentityManagerTrait,
    node::{validate_message_main_logic, Node},
};

// Helper function for encryption
fn encrypt_with_shared_key(data: &str, key: &[u8]) -> Result<String, WebSocketManagerError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| WebSocketManagerError::EncryptionError(e.to_string()))?;
    let nonce = Nonce::from_slice(&[0u8; 12]);
    let ciphertext = cipher
        .encrypt(nonce, data.as_bytes())
        .map_err(|e| WebSocketManagerError::EncryptionError(e.to_string()))?;
    Ok(hex::encode(ciphertext))
}

pub struct WebSocketManager {
    pub connections: Arc<Mutex<HashMap<String, SplitSink<WebSocket, Message>>>>,
    pub subscriptions: Arc<Mutex<HashMap<String, Vec<(WSTopic, String)>>>>,
    pub shared_keys: Arc<Mutex<HashMap<String, String>>>,
    pub shinkai_db: Weak<RwLock<dyn Node + Send + Sync>>,
    pub node_name: String,
    pub identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send + Sync>>,
    pub encryption_secret_key: EncryptionStaticKey,
    pub message_queue: MessageQueue,
    pub node: Arc<dyn Node + Send + Sync>,
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
    pub fn new(
        node: Arc<dyn Node + Send + Sync>,
        node_name: String,
        identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send + Sync>>,
        encryption_secret_key: EncryptionStaticKey,
    ) -> Arc<Self> {
        let manager = Arc::new(WebSocketManager {
            connections: Arc::new(Mutex::new(HashMap::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            shared_keys: Arc::new(Mutex::new(HashMap::new())),
            shinkai_db: Arc::downgrade(&node),
            node_name,
            identity_manager_trait,
            encryption_secret_key,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            node,
        });

        let message_queue = manager.message_queue.clone();
        tokio::spawn(Self::start_message_sender(manager.clone(), message_queue));
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
        let identity_manager_clone = self.identity_manager_trait.clone();
        validate_message_main_logic(
            message,
            identity_manager_clone,
            &shinkai_name,
            message.clone(),
            None,
        )?;

        let sender_identity = self.get_sender_identity(shinkai_name).await?;
        Ok((message.clone(), sender_identity))
    }

    pub async fn has_access(
        &self,
        inbox_name: &InboxName,
        sender_identity: &Identity,
    ) -> Result<bool, WebSocketManagerError> {
        let node = self.node.clone();
        match node.has_inbox_access(inbox_name, sender_identity).await {
            Ok(has_access) => Ok(has_access),
            Err(e) => Err(WebSocketManagerError::AccessDenied(e.to_string())),
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
        let (validated_message, sender_identity) = match validation_result {
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

        let shinkai_profile_name = sender_shinkai_name.to_string();
        let shared_key = ws_message.shared_key.clone();

        // Initialize the topic map for the new connection
        let mut topic_map = HashMap::new();

        // Iterate over the subscriptions to check access and add them to the topic map
        for subscription in ws_message.subscriptions.iter() {
            let inbox_name = InboxName::new(subscription.subtopic.clone().unwrap_or_default())?;
            let has_access = self.has_access(&inbox_name, &sender_identity).await?;

            if !has_access {
                shinkai_log(
                    ShinkaiLogOption::WsAPI,
                    ShinkaiLogLevel::Debug,
                    format!(
                        "Access denied for shinkai_name: {} on topic: {:?} and subtopic: {:?}",
                        sender_shinkai_name, subscription.topic, subscription.subtopic
                    ).as_str(),
                );
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
        let subscriptions_to_add: Vec<String> = topic_map
            .iter()
            .map(|(topic, _)| topic.clone())
            .collect();
        let subscriptions_to_remove: Vec<String> = self
            .subscriptions
            .get(&shinkai_profile_name)
            .map(|existing| {
                existing
                    .iter()
                    .filter(|(topic, _)| !topic_map.contains_key(*topic))
                    .map(|(topic, _)| topic.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.update_subscriptions(&shinkai_profile_name, &subscriptions_to_add, &subscriptions_to_remove)
            .await?;

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

    pub async fn update_subscriptions(
        &mut self,
        shinkai_name: &str,
        subscriptions_to_add: &[String],
        subscriptions_to_remove: &[String],
    ) -> Result<(), WebSocketManagerError> {
        // We already checked that the user is allowed to have those subscriptions
        let profile_subscriptions = self.subscriptions.entry(shinkai_name.to_string()).or_default();

        // Add new subscriptions
        for topic in subscriptions_to_add {
            profile_subscriptions.insert(topic.clone(), true);
        }

        // Remove specified subscriptions
        for topic in subscriptions_to_remove {
            profile_subscriptions.remove(topic);
        }

        let current_subscriptions: Vec<String> = profile_subscriptions.keys().cloned().collect();
        shinkai_log(
            ShinkaiLogOption::WsAPI,
            ShinkaiLogLevel::Info,
            format!("current_subscriptions: {:?}", current_subscriptions).as_str(),
        );

        Ok(())
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
                .map(|subscriptions| subscriptions.contains_key(&format!("{}:::{}", WSTopic::SmartInboxes, "")))
                .unwrap_or(false);

            let is_subscribed_to_sheets = self
                .subscriptions
                .get(id)
                .map(|subscriptions| subscriptions.contains_key(&format!("{}:::{}", WSTopic::Sheet, "")))
                .unwrap_or(false);

            let is_subscribed_to_topic = self
                .subscriptions
                .get(id)
                .map(|subscriptions| subscriptions.contains_key(&topic_subtopic))
                .unwrap_or(false);

            if is_subscribed_to_smart_inboxes
                || is_subscribed_to_topic
                || (is_subscribed_to_sheets && topic == WSTopic::Sheet)
            {
                let mut connection = connection.lock().await;

                let message_to_send = if let Some(shared_key) = self.shared_keys.get(id) {
                    // If we have a shared key, encrypt the payload
                    let shared_key_bytes = hex::decode(shared_key)
                        .map_err(|e| WebSocketManagerError::EncryptionError(e.to_string()))?;

                    let encrypted_update = encrypt_with_shared_key(&payload_json, &shared_key_bytes)
                        .map_err(|e| WebSocketManagerError::EncryptionError(e.to_string()))?;

                    Message::text(encrypted_update)
                } else {
                    // Otherwise, send as plain text
                    Message::text(payload_json.clone())
                };

                if let Err(e) = connection.send(message_to_send).await {
                    shinkai_log(
                        ShinkaiLogOption::WsAPI,
                        ShinkaiLogLevel::Error,
                        format!("Failed to send message to {}: {}", id, e).as_str(),
                    );
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
        let result = identity_manager_lock.find_by_identity_name(&shinkai_name).await;
        match result {
            Ok(Some(identity)) => Ok(identity.clone()),
            Ok(None) => Err(WebSocketManagerError::IdentityNotFound(shinkai_name.to_string())),
            Err(e) => Err(WebSocketManagerError::IdentityManagerError(e.to_string())),
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
