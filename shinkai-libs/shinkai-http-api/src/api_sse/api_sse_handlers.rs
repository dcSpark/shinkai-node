use std::{collections::HashMap, convert::Infallible, sync::Arc};
use std::pin::Pin;
use bytes::Bytes;
use rand::random;
use futures::{Stream};
use futures::StreamExt as FuturesStreamExt;
use futures::TryStreamExt;
use rmcp::{
    model::{ClientJsonRpcMessage, JsonRpcError, RequestId, ServerJsonRpcMessage, InitializeRequestParam},
    service::{serve_directly},
};
use serde_json::{Value, json};
use tokio::{sync::{mpsc, RwLock}, time::Duration};
use tokio_stream::wrappers::ReceiverStream;
use warp::{http::{Response, StatusCode}, reject, Rejection, Reply};

use crate::api_sse::mcp_tools_service::McpToolsService;
use tokio_stream::StreamExt as TokioStreamExt;

// Custom rejection types
#[derive(Debug)]
pub struct IoError;
impl reject::Reject for IoError {}

#[derive(Debug)]
pub struct PayloadTooLarge;
impl reject::Reject for PayloadTooLarge {}

#[derive(Debug)]
pub struct SessionExpired;
impl reject::Reject for SessionExpired {}

type Result<T> = std::result::Result<T, Rejection>;
type SessionId = String;

/// Session information stored in the MCP state
#[derive(Clone)]
pub struct SessionInfo {
    /// Sender channel for messages to the client
    pub client_sender: mpsc::Sender<ServerJsonRpcMessage>,
    /// Creation timestamp
    pub created_at: std::time::SystemTime,
}

type ClientSender = mpsc::Sender<ServerJsonRpcMessage>;
type ServiceSender = mpsc::Sender<ClientJsonRpcMessage>;

/// MCP state for managing active sessions
pub struct McpState {
    /// Map of session IDs to session information
    sessions: RwLock<HashMap<SessionId, SessionInfo>>,
    /// Map of session IDs to service transports
    transports: RwLock<HashMap<SessionId, ServiceSender>>,
    /// Interval for keep-alive pings in seconds (None to disable)
    ping_interval: Option<Duration>,
}

impl McpState {
    /// Create a new MCP state
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            transports: RwLock::new(HashMap::new()),
            ping_interval: Some(Duration::from_secs(30)),
        }
    }

    /// Create a new MCP state with a specific ping interval
    pub fn with_ping_interval(ping_interval: Option<Duration>) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            transports: RwLock::new(HashMap::new()),
            ping_interval,
        }
    }

    /// Register a new session with client sender and auth token
    pub async fn register_session(&self, session_id: &str, sender: ClientSender) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), SessionInfo {
            client_sender: sender,
            created_at: std::time::SystemTime::now(),
        });
        
        tracing::debug!("Registered session: {}", session_id);
    }

    /// Remove a session
    pub async fn remove_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        let removed = sessions.remove(session_id).is_some();
        
        if removed {
            // Also clean up transports
            let mut transports = self.transports.write().await;
            transports.remove(session_id);
            
            tracing::debug!("Removed session: {}", session_id);
        }
        
        removed
    }

    /// Get a client sender for a session
    pub async fn get_client_sender(&self, session_id: &str) -> Option<ClientSender> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|info| info.client_sender.clone())
    }
    
    /// Get the session info for a session
    pub async fn get_session_info(&self, session_id: &str) -> Option<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }
    
    /// Get the ping interval
    pub fn ping_interval(&self) -> Option<Duration> {
        self.ping_interval
    }
    
    /// Register a service transport
    pub async fn register_service_transport(&self, session_id: &str, tx: ServiceSender) {
        let mut transports = self.transports.write().await;
        transports.insert(session_id.to_string(), tx);
        tracing::debug!("Registered service transport for session: {}", session_id);
    }
    
    /// Get service transport for a session
    pub async fn get_service_transport(&self, session_id: &str) -> Option<ServiceSender> {
        let transports = self.transports.read().await;
        transports.get(session_id).cloned()
    }
    
    /// Get all active sessions
    pub async fn get_all_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }
    
    /// Get session count
    pub async fn get_session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
    
    /// Clean up expired sessions
    pub async fn clean_expired_sessions(&self, max_age: Duration) -> usize {
        let now = std::time::SystemTime::now();
        let mut to_remove = Vec::new();
        
        // First identify sessions to remove
        {
            let sessions = self.sessions.read().await;
            for (id, info) in sessions.iter() {
                if let Ok(age) = now.duration_since(info.created_at) {
                    if age > max_age {
                        to_remove.push(id.clone());
                    }
                }
            }
        }
        
        // Then remove them
        let count = to_remove.len();
        for id in to_remove {
            self.remove_session(&id).await;
        }
        
        count
    }
}

/// Generate a random session ID
fn generate_session_id() -> String {
    format!("{:016x}", random::<u128>())
}

/// Handle SSE connections
pub async fn sse_handler(
    state: Arc<McpState>,
    tools_service: Arc<McpToolsService>,
) -> Result<impl Reply> {
    
    // Generate a unique session ID
    let session_id = generate_session_id();
    tracing::info!("New SSE connection established with sessionId: {}", session_id);

    // Create channels for bidirectional communication
    let (client_tx, client_rx) = mpsc::channel::<ServerJsonRpcMessage>(64);
    let (service_tx, service_rx) = mpsc::channel::<ClientJsonRpcMessage>(64);

    // Register the session with auth token in state
    state.register_session(&session_id, client_tx.clone()).await;
    
    // Also register the service transport
    state.register_service_transport(&session_id, service_tx).await;

    // Create and setup an MCP transport
    let transport = McpTransport {
        session_id: session_id.clone(),
        service_rx: ReceiverStream::new(service_rx),
        client_tx,
        state: state.clone(),
    };

    let session_id_clone = session_id.clone();
    let state_clone = state.clone();
    let tools_service_clone = tools_service.clone();

    // Start the MCP service - spawn to not block this function
    tokio::spawn(async move {
        match serve_directly(tools_service_clone.as_ref().clone(), transport, InitializeRequestParam::default()).await {
            Ok(running_service) => {
                tracing::info!("MCP service started for session: {}", session_id_clone);
                
                // Wait for service to complete
                if let Err(e) = running_service.waiting().await {
                    tracing::error!("MCP service error for session {}: {:?}", session_id_clone, e);
                }
                
                // Clean up using cloned state and session_id
                state_clone.remove_session(&session_id_clone).await;
            },
            Err(e) => {
                tracing::error!("Failed to start MCP service for session {}: {:?}", session_id_clone, e);
                state_clone.remove_session(&session_id_clone).await;
            }
        }
    });

    // Start building SSE stream with client_rx
    let client_rx_stream = ReceiverStream::new(client_rx);
    
    // Initial message with endpoint information
    let endpoint_event = format!(
        "event: endpoint\ndata: /mcp/sse?sessionId={}\n\n",
        session_id
    );
    
    // Base stream with endpoint and client messages
    let base_stream = tokio_stream::StreamExt::chain(
        tokio_stream::once(Ok::<_, Infallible>(endpoint_event)),
        tokio_stream::StreamExt::map(client_rx_stream, |msg| {
            tracing::debug!("sse_handler: Received message from client_rx: {:?}", msg);
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    let event_string = format!("event: message\ndata: {}\n\n", json);
                    tracing::debug!("sse_handler: Sending event: {}", event_string);
                    Ok(event_string)
                },
                Err(e) => {
                    tracing::error!("Failed to serialize message: {}", e);
                    Ok(String::new())
                }
            }
        })
    );

    // Conditionally add keep-alive pings
    let final_stream: Pin<Box<dyn Stream<Item = std::result::Result<String, Infallible>> + Send>> = if let Some(interval) = state.ping_interval() {
        let ping_stream = TokioStreamExt::map(tokio_stream::wrappers::IntervalStream::new(
            tokio::time::interval(interval)
        ), |_| Ok::<_, Infallible>(": ping\n\n".to_string()));

        // Use TokioStreamExt::merge and Box::pin the result
        Box::pin(TokioStreamExt::merge(base_stream, ping_stream))
    } else {
        Box::pin(base_stream)
    };

    // Build the response
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(warp::hyper::Body::wrap_stream(final_stream.map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "infallible stream error"))))
        .map_err(|e| {
            tracing::error!("Failed to build SSE response: {}", e);
            reject::custom(IoError)
        })?;

    Ok(resp)
}

/// Handle POST events from clients
pub async fn post_event_handler(
    session_id: String,
    body: Bytes,
    state: Arc<McpState>,
) -> Result<impl Reply> {
    let body_str = String::from_utf8_lossy(&body);
    tracing::debug!("Received raw message for session {}: {}", session_id, body_str);

    // 1. Deserialize into generic Value first
    let generic_value: Value = match serde_json::from_slice(&body) {
        Ok(val) => val,
        Err(e) => {
            tracing::warn!("Failed initial parse for session {}: {}", session_id, e);
            let error = JsonRpcError {
                jsonrpc: Default::default(),
                id: RequestId::String("parse_error".into()), // Use a generic ID
                error: rmcp::model::ErrorData::parse_error(
                    format!("Invalid JSON: {}", e),
                    Some(Value::String(body_str.to_string())),
                ),
            };
            return Ok(warp::reply::json(&error).into_response());
        }
    };

    // Keep original request ID if present
    let original_id = generic_value.get("id").cloned().unwrap_or(Value::Null);

    // 4. Attempt to deserialize the final value into ClientJsonRpcMessage
    let client_message: ClientJsonRpcMessage = match serde_json::from_value(generic_value) {
        Ok(msg) => msg,
        Err(e) => {
            tracing::warn!("Failed final parse after potential transform for session {}: {}", session_id, e);
            // Attempt to deserialize original_id, fallback to a string ID
            let req_id = serde_json::from_value(original_id).unwrap_or_else(|_| RequestId::String("final_parse_error".into()));
            let error = JsonRpcError {
                jsonrpc: Default::default(),
                id: req_id,
                error: rmcp::model::ErrorData::invalid_request(
                    format!("Invalid JSON-RPC structure after transform: {}", e),
                    None, 
                ),
            };
            return Ok(warp::reply::json(&error).into_response());
        }
    };

    // 5. Forward the message to the MCP service
    let service_tx = state.get_service_transport(&session_id).await.ok_or_else(|| {
        tracing::warn!("Transport not found for session: {}", session_id);
        reject::not_found()
    })?;

    if let Err(e) = service_tx.send(client_message).await {
        tracing::error!("Failed to forward message to service for session {}: {}", session_id, e);
        return Err(reject::custom(IoError));
    }

    // Return success
    Ok(warp::reply::with_status(warp::reply::json(&serde_json::json!({"success": true})), StatusCode::ACCEPTED).into_response())
}

/// MCP Transport implementation that bridges the SSE and MCP systems
pub struct McpTransport {
    session_id: String,
    service_rx: ReceiverStream<ClientJsonRpcMessage>,
    client_tx: mpsc::Sender<ServerJsonRpcMessage>,
    state: Arc<McpState>,
}

impl Stream for McpTransport {
    type Item = ClientJsonRpcMessage;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        FuturesStreamExt::poll_next_unpin(&mut self.service_rx, cx)
    }
}

impl futures::Sink<ServerJsonRpcMessage> for McpTransport {
    type Error = std::io::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), Self::Error>> {
        // Always ready
        std::task::Poll::Ready(Ok(()))
    }

    fn start_send(
        self: std::pin::Pin<&mut Self>,
        item: ServerJsonRpcMessage,
    ) -> std::result::Result<(), std::io::Error> {
        // Access fields directly from the pinned reference
        let session_id = self.session_id.clone();
        let client_tx = self.client_tx.clone();
        
        tokio::spawn(async move {
            tracing::debug!("McpTransport: Attempting to send item for session {}: {:?}", session_id, item);
            if let Err(e) = client_tx.send(item).await {
                tracing::error!("Failed to send message to client for session {}: {}", session_id, e);
            }
        });
        
        Ok(())
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), Self::Error>> {
        // No buffering, so always flushed
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), Self::Error>> {
        // Clean up the session when closing
        let session_id = self.session_id.clone();
        let state = self.state.clone();
        
        tokio::spawn(async move {
            // Use state to clean up instead of global variables
            state.remove_session(&session_id).await;
            tracing::info!("Closed transport for session: {}", session_id);
        });
        
        std::task::Poll::Ready(Ok(()))
    }
}

pub async fn update_tools_cache_handler(
    tools_service: Arc<McpToolsService>,
) -> Result<impl Reply> {
    tracing::info!("Received request to update tools cache");
    match tools_service.update_tools_cache().await {
        Ok(_) => {
            tracing::info!("Tools cache updated successfully via API call");
            let success_response = warp::reply::json(&json!({
                "status": "success",
                "message": "Tools cache update triggered successfully."
            }));
            Ok(warp::reply::with_status(success_response, StatusCode::OK))
        }
        Err(e) => {
            tracing::error!("Failed to update tools cache via API call: {:?}", e);
            let error_response = warp::reply::json(&json!({
                "status": "error",
                "message": "Failed to update tools cache.",
                "details": e.to_string()
            }));
            // Return 500 Internal Server Error
            Ok(warp::reply::with_status(error_response, StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}