use crate::api_sse::mcp_tools_service::McpToolsService;
use bytes::Bytes;
use futures::{Stream, StreamExt, sink::Sink};
use std::{convert::Infallible, sync::Arc, pin::Pin, task::{Context, Poll}};
use rmcp::{model::{ClientJsonRpcMessage, ServerJsonRpcMessage}, ServiceExt};
use uuid::Uuid;
use warp::{
    http::StatusCode,
    sse::Event,
    Rejection,
};
use async_channel::Receiver;

/// Maximum size for event payloads in bytes (4MB)
const BODY_BYTES_LIMIT: usize = 1 << 22;

/// Generate a unique session ID
fn session_id() -> String {
    Uuid::new_v4().to_string()
}

/// State for the MCP SSE connections
#[derive(Default)]
pub struct McpState {
    // Map of session IDs to client message senders
    client_senders: tokio::sync::RwLock<std::collections::HashMap<String, async_channel::Sender<ClientJsonRpcMessage>>>,
}

impl McpState {
    /// Create a new state
    pub fn new() -> Self {
        Self {
            client_senders: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
    
    /// Register a client sender for a session
    pub async fn register_client(&self, session: String, sender: async_channel::Sender<ClientJsonRpcMessage>) {
        tracing::info!(session = %session, "Registering client sender in state");
        let mut client_senders = self.client_senders.write().await;
        client_senders.insert(session, sender);
        tracing::info!(active_clients = client_senders.len(), "Active client count after registration");
    }
    
    /// Get a client sender for a session
    pub async fn get_client(&self, session: &str) -> Option<async_channel::Sender<ClientJsonRpcMessage>> {
        let client_senders = self.client_senders.read().await;
        let result = client_senders.get(session).cloned();
        if result.is_none() {
            tracing::warn!(session = %session, "Client sender not found in state");
        } else {
            tracing::debug!(session = %session, "Found client sender in state");
        }
        result
    }
    
    /// Remove a client sender for a session
    pub async fn remove_client(&self, session: &str) {
        tracing::info!(session = %session, "Removing client sender from state");
        let mut client_senders = self.client_senders.write().await;
        client_senders.remove(session);
        tracing::info!(active_clients = client_senders.len(), "Active client count after removal");
    }
}

/// Handler for SSE connections
pub async fn sse_handler(
    state: Arc<McpState>,
    tools_service: Arc<McpToolsService>,
) -> Result<impl warp::Reply, Rejection> {
    // Generate a session ID
    let session = session_id();
    tracing::info!(session = %session, "New SSE connection started");
    
    // Log important connection info
    tracing::info!(
        session = %session,
        "SSE connection details: Content-Type should be text/event-stream, Cache-Control: no-cache"
    );

    // Create message channels
    let (tx_client, rx_client) = async_channel::bounded::<ClientJsonRpcMessage>(100);
    let (tx_server, rx_server) = async_channel::bounded::<ServerJsonRpcMessage>(100);
    
    // Register the client sender in the state
    state.register_client(session.clone(), tx_client).await;
    tracing::info!(session = %session, "Registered client sender in state");
    
    // Initialize the MCP service with the transport
    tracing::info!(session = %session, "Initializing service with transport");
    
    // Create a stream that receives from our async_channel
    let stream = futures::stream::unfold(rx_client, |rx| async move {
        match rx.recv().await {
            Ok(item) => Some((item, rx)),
            Err(_) => None,
        }
    });
    
    // Create a sink that forwards to our async_channel
    let sink = futures::sink::unfold(tx_server.clone(), |tx, item| async move {
        match tx.send(item).await {
            Ok(()) => Ok(tx),
            Err(e) => {
                tracing::error!("Failed to send server message: {:?}", e);
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed"))
            }
        }
    });
    
    let service = tools_service.clone();
    let session_copy = session.clone();
    
    // Spawn a task to handle the service
    let service_task = tokio::spawn(async move {
        tracing::info!(session = %session_copy, "Server task started");
        
        // Use ServiceExt::serve with the service and transport
        let service = (*service).clone();
        if let Err(e) = ServiceExt::serve(service, (sink, stream)).await {
            tracing::error!(session = %session_copy, error = ?e, "Service error");
        }
        
        tracing::info!(session = %session_copy, "Server task completed");
    });
    
    // Add cleanup task
    let session_copy = session.clone();
    let state_copy = state.clone();
    tokio::spawn(async move {
        let _ = service_task.await;
        tracing::info!(session = %session_copy, "Cleaning up session");
        state_copy.remove_client(&session_copy).await;
        tracing::info!(session = %session_copy, "Session cleanup completed");
    });
    
    // Create the event stream
    tracing::debug!(session = %session, "Creating SSE event stream");
    let event_stream = create_event_stream(session.clone(), rx_server);

    // Return the SSE response with appropriate headers
    tracing::info!(session = %session, "Returning SSE response to client");
    let sse_reply = warp::sse::reply(event_stream);
    let resp = warp::reply::with_header(sse_reply, "Cache-Control", "no-cache");
    let resp = warp::reply::with_header(resp, "Connection", "keep-alive");
    let resp = warp::reply::with_header(resp, "X-Accel-Buffering", "no");
        
    Ok(resp)
}

/// Create an event stream from server messages
fn create_event_stream(
    session: String,
    server_rx: Receiver<ServerJsonRpcMessage>,
) -> impl Stream<Item = Result<Event, Infallible>> + Send {
    // First, send the endpoint information with the session ID
    let endpoint_data = format!("?sessionId={}", session);
    tracing::info!(session = %session, endpoint_data = %endpoint_data, "Creating endpoint event with sessionId");
    
    // Create the endpoint event
    // The event type is "endpoint" and the data is the query string with sessionId
    let endpoint_event = Event::default()
        .event("endpoint")
        .data(endpoint_data.clone());
    
    // Log the raw event to help debug
    let endpoint_event_str = format!("event: endpoint\ndata: {}\n\n", endpoint_data);
    tracing::info!(session = %session, raw_event = %endpoint_event_str, "Raw endpoint event data");

    // Send a heartbeat comment every 30 seconds to keep the connection alive
    let session_clone = session.clone();
    let heartbeat = futures::stream::unfold(0, move |n| {
        let session = session_clone.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let comment_event = Event::default().comment("heartbeat");
            tracing::debug!(session = %session, heartbeat_num = n, "Sending heartbeat");
            Some((Ok(comment_event), n + 1))
        }
    });

    // Create the server message stream
    let session_clone = session.clone();
    let message_stream = futures::stream::unfold(server_rx, move |rx| {
        let session = session_clone.clone();
        async move {
            match rx.recv().await {
                Ok(message) => {
                    let json = serde_json::to_string(&message).unwrap_or_default();
                    tracing::debug!(session = %session, message_length = json.len(), "Sending SSE message");
                    
                    // Create the event with type "message" and JSON data
                    let event = Event::default()
                        .event("message")
                        .data(json.clone());
                    
                    // Log the raw event for debugging
                    let raw_event = format!("event: message\ndata: {}\n\n", json);
                    tracing::trace!(session = %session, event_length = raw_event.len(), "Raw message event data");
                    
                    Some((Ok(event), rx))
                }
                Err(e) => {
                    tracing::error!(session = %session, error = ?e, "Error receiving message for SSE stream");
                    None
                }
            }
        }
    });

    // Combine the initial endpoint event, heartbeats, and message events
    // Use select to combine the streams in a way that works with the current futures version
    futures::stream::once(futures::future::ready(Ok(endpoint_event)))
        .chain(futures::stream::select(heartbeat, message_stream))
}

/// Handler for client messages to an existing SSE connection
pub async fn post_event_handler(
    session: String,
    body: Bytes,
    state: Arc<McpState>,
) -> Result<impl warp::Reply, Rejection> {
    tracing::info!(
        session = %session,
        body_size = body.len(),
        "Received client message for session"
    );

    if body.len() > 1024 * 1024 * 4 {
        tracing::warn!(session = %session, size = body.len(), "Client payload too large");
        return Err(warp::reject::custom(PayloadTooLarge));
    }

    let client = match state.get_client(&session).await {
        Some(client) => client,
        None => {
            tracing::warn!(session = %session, "Session not found for client message");
            return Err(warp::reject::not_found());
        }
    };

    // Special handling for initialize method
    // First parse the raw JSON to see what we're dealing with
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(val) => val,
        Err(e) => {
            tracing::error!(
                session = %session,
                error = %e,
                body = ?String::from_utf8_lossy(&body),
                "Failed to parse JSON"
            );
            return Err(warp::reject::custom(IoError));
        }
    };

    // Check if this is an initialize message
    if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
        if method == "initialize" {
            tracing::info!(session = %session, "Handling initialize message");
            let id = value.get("id").cloned().unwrap_or(serde_json::json!(1));
            
            // Get the params if any are provided
            let params = value.get("params").unwrap_or(&serde_json::json!({}));
            
            // Try to extract client info if provided in vscode-json-rpc style
            let client_name = params.get("clientInfo").and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("ShinkaiClient");
            
            let client_version = params.get("clientInfo").and_then(|c| c.get("version"))
                .and_then(|v| v.as_str())
                .unwrap_or("1.0.0");
                
            let protocol_version = params.get("protocolVersion")
                .and_then(|p| p.as_str())
                .unwrap_or("2024-11-05");
            
            // Create a properly formatted initialize message
            use rmcp::model::{JsonRpcRequest, Request, InitializeResultMethod, InitializeRequestParam, ProtocolVersion, ClientCapabilities, Implementation};
            let init_request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: id.into(),
                request: Request {
                    method: InitializeResultMethod::value().to_string(),
                    params: InitializeRequestParam {
                        protocol_version: ProtocolVersion(protocol_version.to_string()),
                        capabilities: ClientCapabilities::default(),
                        client_info: Implementation {
                            name: client_name.to_string(),
                            version: client_version.to_string(),
                        },
                    },
                },
            };
            
            // Convert to ClientJsonRpcMessage
            use rmcp::model::{JsonRpcMessage, ClientJsonRpcMessage, ClientRequest};
            let message: ClientJsonRpcMessage = JsonRpcMessage::Request(init_request);
            
            tracing::info!(
                session = %session,
                message = ?serde_json::to_string(&message).unwrap_or_default(),
                "Created formatted initialize message"
            );
            
            // Send the properly formatted message
            match client.try_send(message) {
                Ok(_) => {
                    tracing::debug!(session = %session, "Successfully forwarded initialize message");
                    return Ok(StatusCode::OK);
                }
                Err(e) => {
                    tracing::error!(session = %session, error = ?e, "Failed to send initialize message");
                    return Err(warp::reject::custom(IoError));
                }
            }
        }
    }

    // For non-initialize messages, use the standard parsing
    let client_message = match serde_json::from_slice::<ClientJsonRpcMessage>(&body) {
        Ok(message) => {
            tracing::info!(
                session = %session,
                "Successfully parsed client message"
            );
            message
        }
        Err(e) => {
            tracing::error!(
                session = %session,
                error = %e,
                body = ?String::from_utf8_lossy(&body),
                "Failed to parse client message"
            );
            return Err(warp::reject::custom(IoError));
        }
    };

    match client.try_send(client_message) {
        Ok(_) => {
            tracing::debug!(session = %session, "Successfully forwarded client message to handler");
            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!(session = %session, error = ?e, "Failed to send message to client channel");
            Err(warp::reject::custom(IoError))
        }
    }
}

/// Transport that sends messages to the server
pub struct SinkTransport {
    tx: async_channel::Sender<ServerJsonRpcMessage>,
}

impl SinkTransport {
    /// Create a new sink transport
    pub fn new(tx: async_channel::Sender<ServerJsonRpcMessage>) -> Self {
        Self { tx }
    }
}

impl Sink<ServerJsonRpcMessage> for SinkTransport {
    type Error = std::io::Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: ServerJsonRpcMessage) -> Result<(), Self::Error> {
        // Try to send the item and convert the error if it fails
        match self.tx.try_send(item) {
            Ok(_) => Ok(()),
            Err(async_channel::TrySendError::Full(_)) => {
                tracing::warn!("Sink channel is full");
                Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "Channel full"))
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                tracing::error!("Sink channel is closed");
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed"))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

/// Transport that receives messages from the client
pub struct StreamTransport {
    rx: async_channel::Receiver<ClientJsonRpcMessage>,
}

impl StreamTransport {
    /// Create a new stream transport
    pub fn new(rx: async_channel::Receiver<ClientJsonRpcMessage>) -> Self {
        Self { rx }
    }
}

impl Stream for StreamTransport {
    type Item = ClientJsonRpcMessage;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Get a mutable reference to self
        let this = self.get_mut();
        
        // Poll the receiver
        match Pin::new(&mut this.rx).poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
            Poll::Ready(None) => {
                tracing::debug!("Stream channel closed");
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Custom rejection types
/// Error when payload is too large
#[derive(Debug)]
pub struct PayloadTooLarge;
impl warp::reject::Reject for PayloadTooLarge {}

/// IO Error rejection
#[derive(Debug)]
pub struct IoError;
impl warp::reject::Reject for IoError {} 