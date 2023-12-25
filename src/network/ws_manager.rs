use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use warp::ws::WebSocket;
use futures::stream::SplitSink;
use warp::ws::Message;

pub struct WebSocketManager {
    connections: HashMap<String, Arc<Mutex<SplitSink<WebSocket, Message>>>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn add_connection(&mut self, id: String, connection: SplitSink<WebSocket, Message>) {
        self.connections.insert(id, Arc::new(Mutex::new(connection)));
    }

    pub fn get_all_connections(&self) -> Vec<Arc<Mutex<SplitSink<WebSocket, Message>>>> {
        self.connections.values().cloned().collect()
    }
}

// Shared reference to WebSocketManager
pub type SharedWebSocketManager = Arc<Mutex<WebSocketManager>>;