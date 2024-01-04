use futures::stream::SplitSink;
use futures::SinkExt;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::fmt;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use warp::ws::Message;
use warp::ws::WebSocket;

use crate::db::ShinkaiDB;

use super::Node;
use super::node_shareable_logic::validate_message_main_logic;
use crate::managers::identity_manager::IdentityManagerTrait;

#[derive(Debug)]
pub enum WebSocketManagerError {
    UserValidationFailed(String),
    AccessDenied(String),
}

impl fmt::Display for WebSocketManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WebSocketManagerError::UserValidationFailed(msg) => write!(f, "User validation failed: {}", msg),
            WebSocketManagerError::AccessDenied(msg) => write!(f, "Access denied: {}", msg),
        }
    }
}

pub struct WebSocketManager {
    connections: HashMap<String, Arc<Mutex<SplitSink<WebSocket, Message>>>>,
    // TODO: maybe the first string should be a ShinkaiName? or at least a shinkai name string
    subscriptions: HashMap<String, HashMap<String, bool>>,
    shinkai_db: Arc<Mutex<ShinkaiDB>>,
    node_name: ShinkaiName,
    identity_manager_trait: Arc<Mutex<Box<dyn IdentityManagerTrait + Send>>>,
}

// TODO: maybe this should run on its own thread
impl WebSocketManager {
    pub fn new(
        shinkai_db: Arc<Mutex<ShinkaiDB>>,
        node_name: ShinkaiName,
        identity_manager_trait: Arc<Mutex<Box<dyn IdentityManagerTrait + Send>>>,
    ) -> Self {
        Self {
            connections: HashMap::new(),
            subscriptions: HashMap::new(),
            shinkai_db,
            node_name,
            identity_manager_trait,
        }
    }

    pub async fn user_validation(&self, shinkai_name: ShinkaiName, message: &ShinkaiMessage) -> bool {
        // Message can't be encrypted at this point
        let is_body_encrypted = message.clone().is_body_currently_encrypted();
        if is_body_encrypted {
            eprintln!("Message body is encrypted, can't validate user: {}", shinkai_name);
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

        eprintln!("user_validation result: {:?}", result);
        match result {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    // Placeholder function that always returns true
    pub async fn has_access(
        &self,
        shinkai_name: ShinkaiName,
        topic: String,
        subtopic: Option<String>,
    ) -> bool {
        eprintln!("has_access> shinkai_name: {}", shinkai_name);
        eprintln!("has_access> topic: {}", topic);
        match topic.as_str() {
            "inbox" => {
                let subtopic = subtopic.unwrap_or_default();
                eprintln!("subtopic: {}", subtopic);
                let inbox_name = InboxName::new(subtopic.clone()).unwrap(); // TODO: handle error
                let sender_subidentity = {
                    let identity_manager_lock = self.identity_manager_trait.lock().await;
                    identity_manager_lock.find_by_identity_name(shinkai_name.clone()).unwrap().clone() // TODO: handle error
                };
                eprintln!("sender_subidentity: {:?}", sender_subidentity);

                match Node::has_inbox_access(self.shinkai_db.clone(), &inbox_name, &sender_subidentity).await {
                    Ok(_) => {
                        eprintln!(
                            "Access granted for inbox: {} and sender_subidentity: {}",
                            inbox_name, shinkai_name.full_name
                        );
                        return true;
                    }
                    Err(e) => {
                        eprintln!(
                            "Access denied for inbox: {} and sender_subidentity: {}",
                            inbox_name, shinkai_name.full_name
                        );
                        return false;
                    }
                }
            }
            "smart_inboxes" => {
                eprintln!("smart_inboxes");
            }
            _ => {
                eprintln!("Unknown topic: {}", topic);
                return false;
            }
        }

        // TODO: create enum with all the different topic and subtopics
        // Check if the user has access to the topic and subtopic here...
        true
    }

    pub async fn add_connection(
        &mut self,
        shinkai_name: ShinkaiName,
        message: ShinkaiMessage,
        connection: Arc<Mutex<SplitSink<WebSocket, Message>>>,
        topic: String,
        subtopic: Option<String>,
    ) -> Result<(), WebSocketManagerError> {
        eprintln!("Adding connection for shinkai_name: {}", shinkai_name);
        eprintln!("add_connection> Message: {:?}", message);

        if !self.user_validation(shinkai_name.clone(), &message).await {
            eprintln!("User validation failed for shinkai_name: {}", shinkai_name);
            return Err(WebSocketManagerError::UserValidationFailed(format!(
                "User validation failed for shinkai_name: {}",
                shinkai_name
            )));
        }

        let shinkai_profile_name = shinkai_name.to_string();
        if !self.has_access(shinkai_name.clone(), topic.clone(), subtopic.clone()).await {
            eprintln!(
                "Access denied for shinkai_name: {} on topic: {} and subtopic: {:?}",
                shinkai_name, topic, subtopic
            );
            return Err(WebSocketManagerError::AccessDenied(format!(
                "Access denied for shinkai_name: {} on topic: {} and subtopic: {:?}",
                shinkai_name, topic, subtopic
            )));
        }

        eprintln!("topic: {:?}", topic);
        eprintln!("subtopic: {:?}", subtopic);

        self.connections.insert(shinkai_profile_name.clone(), connection);
        let mut topic_map = HashMap::new();
        let topic_subtopic = format!("{}:::{}", topic, subtopic.unwrap_or_default());
        eprintln!("topic_subtopic subscription: {:?}", topic_subtopic);
        topic_map.insert(topic_subtopic, true);
        self.subscriptions.insert(shinkai_profile_name, topic_map);

        Ok(())
    }

    pub fn get_all_connections(&self) -> Vec<Arc<Mutex<SplitSink<WebSocket, Message>>>> {
        self.connections.values().cloned().collect()
    }

    // TODO: Is topic enough? should we have topic and subtopic? e.g. type of update and inbox_name
    pub async fn handle_update(&mut self, topic: String, subtopic: String, update: String) {
        let topic_subtopic = format!("{}:::{}", topic, subtopic);
        eprintln!("\n\nSending update to topic: {}", topic_subtopic);
        // Check if the update needs to be sent
        // This is just a placeholder, replace with your actual check
        let needs_to_be_sent = true;

        if needs_to_be_sent {
            // Send the update to all active connections that are subscribed to the topic
            for (id, connection) in self.connections.iter() {
                eprintln!("Checking connection: {}", id);
                if self.subscriptions.get(id).unwrap().get(&topic_subtopic).is_some() {
                    eprintln!("Connection {} is subscribed to the topic", id);
                    let mut connection = connection.lock().await;
                    match connection.send(Message::text(update.clone())).await {
                        Ok(_) => eprintln!("Successfully sent update to connection {}", id),
                        Err(e) => eprintln!("Failed to send update to connection {}: {}", id, e),
                    }
                } else {
                    eprintln!("Connection {} is not subscribed to the topic {:?}", id, topic_subtopic);
                }
            }
        }
    }
}

// Shared reference to WebSocketManager
pub type SharedWebSocketManager = Arc<Mutex<WebSocketManager>>;
