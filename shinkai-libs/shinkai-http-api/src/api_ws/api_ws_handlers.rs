//! WebSocket Proxy Handler
//!
//! This module provides a WebSocket proxy that forwards messages between a client
//! and a target WebSocket server. It includes automatic reconnection functionality
//! with exponential backoff and configurable parameters.
//!
//! # Architecture
//!
//! The proxy consists of several key components:
//! - `WebSocketProxy`: Main proxy struct that handles connection management
//! - `WebSocketProxyConfig`: Configuration parameters for the proxy behavior
//! - `ReconnectionManager`: Manages reconnection attempts with exponential backoff
//!
//! # Usage
//!
//! The main entry point is the `ws_handler` function, which creates a proxy
//! with default configuration and handles the WebSocket connection.
//! 
//! # Why this is needed?
//!
//! The original WS implementation listens on a different port than the HTTP server,
//! and the implementation is low-level and very coupled to the Shinkai Node.
//! It was easier to forward a route on the API server to the original WS implementation,
//! but we should explore integrating the WS into the API server in the future.

use std::sync::Arc;
use std::time::Duration;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{
    shinkai_log, ShinkaiLogLevel, ShinkaiLogOption,
};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use warp::filters::ws::{Message, WebSocket};

/// Configuration for the WebSocket proxy
#[derive(Debug, Clone)]
pub struct WebSocketProxyConfig {
    pub max_reconnect_attempts: u32,
    pub initial_reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub message_buffer_size: usize,
}

impl Default for WebSocketProxyConfig {
    fn default() -> Self {
        Self {
            max_reconnect_attempts: 5,
            initial_reconnect_delay: Duration::from_millis(1000),
            max_reconnect_delay: Duration::from_secs(30),
            message_buffer_size: 1000,
        }
    }
}

/// WebSocket proxy that forwards messages between client and target server
pub struct WebSocketProxy {
    config: WebSocketProxyConfig,
    target_url: String,
}

impl WebSocketProxy {
    pub fn new(target_url: String, config: Option<WebSocketProxyConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
            target_url,
        }
    }

    /// Main entry point for handling WebSocket connections
    pub async fn handle_connection(&self, ws: WebSocket) {
        let (client_tx, client_rx) = ws.split();
        let client_tx = Arc::new(Mutex::new(client_tx));

        let (message_tx, _) = broadcast::channel::<TungsteniteMessage>(self.config.message_buffer_size);
        let (reconnect_tx, reconnect_rx) = mpsc::unbounded_channel::<()>();

        // Handle incoming client messages
        self.spawn_client_message_handler(client_rx, message_tx.clone()).await;

        // Handle connection management and message forwarding
        self.spawn_connection_manager(client_tx, message_tx, reconnect_tx, reconnect_rx).await;
    }

    /// Spawns a task to handle incoming messages from the client
    async fn spawn_client_message_handler(
        &self,
        mut client_rx: futures::stream::SplitStream<WebSocket>,
        message_tx: broadcast::Sender<TungsteniteMessage>,
    ) {
        tokio::spawn(async move {
            while let Some(result) = client_rx.next().await {
                match result {
                    Ok(msg) => {
                        if let Some(tungstenite_msg) = Self::convert_warp_to_tungstenite_message(msg) {
                            if let Err(_) = message_tx.send(tungstenite_msg) {
                                Self::log_error("Failed to send client message to handler");
                                break;
                            }
                        } else {
                            // Close message received
                            Self::log_info("Client WebSocket connection closed");
                            break;
                        }
                    }
                    Err(e) => {
                        Self::log_error(&format!("Error receiving message from client: {}", e));
                        break;
                    }
                }
            }
        });
    }

    /// Spawns the connection manager that handles reconnections and message forwarding
    async fn spawn_connection_manager(
        &self,
        client_tx: Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
        message_tx: broadcast::Sender<TungsteniteMessage>,
        reconnect_tx: mpsc::UnboundedSender<()>,
        mut reconnect_rx: mpsc::UnboundedReceiver<()>,
    ) {
        let target_url = self.target_url.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut reconnect_manager = ReconnectionManager::new(config);

            loop {
                match Self::establish_target_connection(&target_url).await {
                    Ok(target_ws) => {
                        Self::log_info(&format!("Connected to target WebSocket server: {}", target_url));
                        reconnect_manager.reset();

                        if Self::handle_active_connection(
                            target_ws,
                            &client_tx,
                            &message_tx,
                            &reconnect_tx,
                            &mut reconnect_rx,
                        ).await {
                            // Connection closed gracefully, exit
                            break;
                        }
                        // Connection failed, will attempt reconnection
                    }
                    Err(e) => {
                        if !reconnect_manager.should_retry() {
                            Self::log_error(&format!(
                                "Failed to connect after {} attempts: {}",
                                reconnect_manager.config.max_reconnect_attempts, e
                            ));
                            Self::send_error_and_close(&client_tx, "Failed to connect to target server").await;
                            break;
                        }

                        let delay = reconnect_manager.get_next_delay();
                        Self::log_info(&format!(
                            "Connection failed (attempt {}/{}): {}. Retrying in {:?}...",
                            reconnect_manager.attempts,
                            reconnect_manager.config.max_reconnect_attempts,
                            e,
                            delay
                        ));

                        tokio::time::sleep(delay).await;
                        reconnect_manager.increment_attempts();
                    }
                }
            }
        });
    }

    /// Establishes connection to the target WebSocket server
    async fn establish_target_connection(
        target_url: &str,
    ) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(target_url).await?;
        Ok(ws_stream)
    }

    /// Handles an active connection between client and target
    async fn handle_active_connection(
        target_ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        client_tx: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
        message_tx: &broadcast::Sender<TungsteniteMessage>,
        reconnect_tx: &mpsc::UnboundedSender<()>,
        reconnect_rx: &mut mpsc::UnboundedReceiver<()>,
    ) -> bool {
        let (target_tx, target_rx) = target_ws.split();
        let target_tx = Arc::new(Mutex::new(target_tx));

        let client_msg_rx = message_tx.subscribe();

        // Forward messages from client to target
        let client_to_target_task = Self::spawn_client_to_target_forwarder(
            client_msg_rx,
            target_tx.clone(),
            reconnect_tx.clone(),
        );

        // Forward messages from target to client
        let target_to_client_task = Self::spawn_target_to_client_forwarder(
            target_rx,
            client_tx.clone(),
            reconnect_tx.clone(),
        );

        // Wait for completion or reconnection signal
        tokio::select! {
            result = client_to_target_task => {
                Self::log_info("Client to target forwarding completed");
                result.unwrap_or(false)
            }
            result = target_to_client_task => {
                Self::log_info("Target to client forwarding completed");
                result.unwrap_or(false)
            }
            _ = reconnect_rx.recv() => {
                Self::log_info("Reconnection signal received");
                false
            }
        }
    }

    /// Spawns task to forward messages from client to target
    fn spawn_client_to_target_forwarder(
        mut client_msg_rx: broadcast::Receiver<TungsteniteMessage>,
        target_tx: Arc<Mutex<futures::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, TungsteniteMessage>>>,
        reconnect_tx: mpsc::UnboundedSender<()>,
    ) -> tokio::task::JoinHandle<bool> {
        tokio::spawn(async move {
            while let Ok(msg) = client_msg_rx.recv().await {
                let mut lock = target_tx.lock().await;
                if let Err(e) = lock.send(msg).await {
                    Self::log_error(&format!("Error forwarding message to target: {}", e));
                    let _ = reconnect_tx.send(());
                    return false;
                }
            }
            true
        })
    }

    /// Spawns task to forward messages from target to client
    fn spawn_target_to_client_forwarder(
        mut target_rx: futures::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
        client_tx: Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
        reconnect_tx: mpsc::UnboundedSender<()>,
    ) -> tokio::task::JoinHandle<bool> {
        tokio::spawn(async move {
            while let Some(result) = target_rx.next().await {
                match result {
                    Ok(msg) => {
                        if let Some(warp_msg) = Self::convert_tungstenite_to_warp_message(msg) {
                            let mut lock = client_tx.lock().await;
                            if let Err(e) = lock.send(warp_msg).await {
                                Self::log_error(&format!("Error forwarding message to client: {}", e));
                                return false;
                            }
                        } else {
                            // Close message
                            Self::log_info("Target WebSocket connection closed");
                            let _ = reconnect_tx.send(());
                            return false;
                        }
                    }
                    Err(e) => {
                        Self::log_error(&format!("Error receiving message from target: {}", e));
                        let _ = reconnect_tx.send(());
                        return false;
                    }
                }
            }
            true
        })
    }

    /// Converts Warp WebSocket message to Tungstenite message
    fn convert_warp_to_tungstenite_message(msg: Message) -> Option<TungsteniteMessage> {
        if msg.is_text() {
            Some(TungsteniteMessage::Text(msg.to_str().unwrap_or("").to_string().into()))
        } else if msg.is_binary() {
            Some(TungsteniteMessage::Binary(Bytes::from(msg.as_bytes().to_vec())))
        } else if msg.is_close() {
            None // Signal connection close
        } else {
            Some(TungsteniteMessage::Text(String::new().into())) // Ignore other types
        }
    }

    /// Converts Tungstenite message to Warp WebSocket message
    fn convert_tungstenite_to_warp_message(msg: TungsteniteMessage) -> Option<Message> {
        match msg {
            TungsteniteMessage::Text(txt) => Some(Message::text(txt.to_string())),
            TungsteniteMessage::Binary(bin) => Some(Message::binary(bin)),
            TungsteniteMessage::Close(_) => None, // Signal connection close
            _ => Some(Message::text(String::new())), // Ignore other types
        }
    }

    /// Sends error message to client and closes connection
    async fn send_error_and_close(
        client_tx: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
        error_msg: &str,
    ) {
        let mut lock = client_tx.lock().await;
        let _ = lock.send(Message::text(error_msg)).await;
        let _ = lock.send(Message::close()).await;
    }

    /// Logs info message
    fn log_info(msg: &str) {
        shinkai_log(ShinkaiLogOption::WsAPI, ShinkaiLogLevel::Info, msg);
    }

    /// Logs error message
    fn log_error(msg: &str) {
        shinkai_log(ShinkaiLogOption::WsAPI, ShinkaiLogLevel::Error, msg);
    }
}

/// Manages reconnection attempts with exponential backoff
struct ReconnectionManager {
    config: WebSocketProxyConfig,
    attempts: u32,
    current_delay: Duration,
}

impl ReconnectionManager {
    fn new(config: WebSocketProxyConfig) -> Self {
        Self {
            current_delay: config.initial_reconnect_delay,
            config,
            attempts: 0,
        }
    }

    fn should_retry(&self) -> bool {
        self.attempts < self.config.max_reconnect_attempts
    }

    fn get_next_delay(&self) -> Duration {
        self.current_delay
    }

    fn increment_attempts(&mut self) {
        self.attempts += 1;
        self.current_delay = std::cmp::min(self.current_delay * 2, self.config.max_reconnect_delay);
    }

    fn reset(&mut self) {
        self.attempts = 0;
        self.current_delay = self.config.initial_reconnect_delay;
    }
}

/// Main WebSocket handler function - simplified public interface
pub async fn ws_handler(ws: WebSocket, ws_address: std::net::SocketAddr) {
    let target_url = format!("ws://{}:{}/ws", ws_address.ip(), ws_address.port());
    let proxy = WebSocketProxy::new(target_url, None);
    proxy.handle_connection(ws).await;
}
