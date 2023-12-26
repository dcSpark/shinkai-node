use futures::stream::SplitSink;
use futures::SinkExt;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use std::fmt;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use warp::ws::Message;
use warp::ws::WebSocket;

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
}

// TODO: maybe this should run on its own thread
impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            subscriptions: HashMap::new(),
        }
    }

    // Placeholder function that always returns true
    pub fn user_validation(shinkai_name: &String, message: &ShinkaiMessage) -> bool {
        // Check the signature of the message here...
        true
    }

    // Placeholder function that always returns true
    pub fn has_access(shinkai_name: &String, topic: &String, subtopic: &String) -> bool {
        // Check if the user has access to the topic and subtopic here...
        true
    }

    pub fn add_connection(
        &mut self,
        shinkai_name: String,
        message: ShinkaiMessage,
        connection: SplitSink<WebSocket, Message>,
        topic: String,
        subtopic: String,
    ) -> Result<(), WebSocketManagerError> {
        if !Self::user_validation(&shinkai_name, &message) {
            return Err(WebSocketManagerError::UserValidationFailed(format!(
                "User validation failed for shinkai_name: {}",
                shinkai_name
            )));
        }

        if !Self::has_access(&shinkai_name, &topic, &subtopic) {
            return Err(WebSocketManagerError::AccessDenied(format!(
                "Access denied for shinkai_name: {} on topic: {} and subtopic: {}",
                shinkai_name, topic, subtopic
            )));
        }

        self.connections
            .insert(shinkai_name.clone(), Arc::new(Mutex::new(connection)));
        let mut topic_map = HashMap::new();
        let topic_subtopic = format!("{}:::{}", topic, subtopic);
        topic_map.insert(topic_subtopic, true);
        self.subscriptions.insert(shinkai_name, topic_map);

        Ok(())
    }

    pub fn get_all_connections(&self) -> Vec<Arc<Mutex<SplitSink<WebSocket, Message>>>> {
        self.connections.values().cloned().collect()
    }

    // TODO: Is topic enough? should we have topic and subtopic? e.g. type of update and inbox_name
    pub async fn handle_update(&mut self, topic: String, subtopic: String, update: String) {
        let topic_subtopic = format!("{}:::{}", topic, subtopic);
        eprintln!("Sending update to topic: {}", topic_subtopic);
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
                    eprintln!("Connection {} is not subscribed to the topic", id);
                }
            }
        }
    }
}

// Shared reference to WebSocketManager
pub type SharedWebSocketManager = Arc<Mutex<WebSocketManager>>;
