use crate::api_sse::mcp_router::SharedMcpRouter;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use mcp_sdk_server::ByteTransport;
use mcp_sdk_server::Server;
use std::{convert::Infallible, io, sync::Arc};
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use warp::{
    http::StatusCode,
    sse::Event,
    Rejection,
};

/// Maximum size for event payloads in bytes (4MB)
const BODY_BYTES_LIMIT: usize = 1 << 22;

/// Generate a unique session ID
fn session_id() -> String {
    Uuid::new_v4().to_string()
}

/// State for the MCP SSE server
pub struct McpState {
    /// Map of session IDs to write streams
    pub write_streams: Arc<RwLock<std::collections::HashMap<String, Arc<Mutex<WriteHalf<tokio::io::DuplexStream>>>>>>,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            write_streams: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

/// Handler for SSE connections
pub async fn sse_handler(state: Arc<McpState>, router: SharedMcpRouter) -> Result<impl warp::Reply, Rejection> {
    // Buffer size for communication channels (4KB)
    const BUFFER_SIZE: usize = 1 << 12;

    // Generate a session ID
    let session = session_id();
    tracing::debug!(session = %session, "New SSE connection");

    // Create bidirectional channels
    let (c2s_read, c2s_write) = tokio::io::duplex(BUFFER_SIZE);
    let (s2c_read, s2c_write) = tokio::io::duplex(BUFFER_SIZE);
    
    // Split the duplex streams
    let (c2s_read_half, _) = tokio::io::split(c2s_read);
    let (_, c2s_write_half) = tokio::io::split(c2s_write);
    let (s2c_read_half, _) = tokio::io::split(s2c_read);
    let (_, s2c_write_half) = tokio::io::split(s2c_write);

    // Store the write stream for this session
    {
        let mut write_streams = state.write_streams.write().await;
        write_streams.insert(session.clone(), Arc::new(Mutex::new(c2s_write_half)));
    }

    // Clone for task
    let session_clone = session.clone();
    let state_clone = state.clone();

    // Spawn a task to handle the MCP server for this session
    tokio::spawn(async move {
        // Create the RouterService wrapper around our router
        let router_service = crate::api_sse::mcp_router::RouterService(router);
        
        // Create the server with the router service
        let server = Server::new(router_service);
        let bytes_transport = ByteTransport::new(c2s_read_half, s2c_write_half);

        let result = server.run(bytes_transport).await;
        if let Err(e) = result {
            tracing::error!(error = ?e, "MCP server error");
        }

        // Clean up
        let mut write_streams = state_clone.write_streams.write().await;
        write_streams.remove(&session_clone);
        tracing::debug!(session = %session_clone, "SSE connection closed");
    });

    // Create the event stream
    let stream = create_event_stream(session.clone(), s2c_read_half);

    // Return the SSE response
    Ok(warp::sse::reply(stream))
}

/// Create an event stream from a read half
fn create_event_stream(
    session: String,
    s2c_read: ReadHalf<tokio::io::DuplexStream>,
) -> impl Stream<Item = Result<Event, Infallible>> + Send {
    // First, send the endpoint information
    let endpoint_event = Event::default()
        .event("endpoint")
        .data(format!("?sessionId={}", session));

    // Use tokio codec to frame the messages
    let framed = tokio_util::codec::FramedRead::new(
        s2c_read,
        tokio_util::codec::LinesCodec::new(),
    );

    // Combine the initial event with the stream of message events
    futures::stream::once(futures::future::ok(endpoint_event)).chain(
        framed.map(move |line_result| {
            let line = match line_result {
                Ok(line) => line,
                Err(e) => {
                    tracing::error!(error = ?e, "Error reading from stream");
                    return Ok(Event::default().event("error").data(format!("{{\"error\": \"{}\" }}", e)));
                }
            };

            Ok(Event::default().event("message").data(line))
        }),
    )
}

/// Handler for posting events to a session
pub async fn post_event_handler(
    session_id: String,
    body: Bytes,
    state: Arc<McpState>,
) -> Result<impl warp::Reply, Rejection> {
    // Get the write stream for this session
    let write_stream = {
        let read_guard = state.write_streams.read().await;
        read_guard
            .get(&session_id)
            .cloned()
            .ok_or_else(|| warp::reject::not_found())
    }?;

    // Check payload size
    if body.len() > BODY_BYTES_LIMIT {
        return Err(warp::reject::custom(PayloadTooLarge));
    }

    // Lock the stream and write the data
    let mut write_stream = write_stream.lock().await;
    
    // Write the body and a newline
    if let Err(e) = write_stream.write_all(&body).await {
        tracing::error!(error = ?e, "Failed to write to stream");
        return Err(warp::reject::custom(IoError(e)));
    }
    
    if let Err(e) = write_stream.write_u8(b'\n').await {
        tracing::error!(error = ?e, "Failed to write newline to stream");
        return Err(warp::reject::custom(IoError(e)));
    }

    Ok(warp::reply::with_status(
        "Event accepted",
        StatusCode::ACCEPTED,
    ))
}

// Custom rejection for IO errors
#[derive(Debug)]
pub struct IoError(pub io::Error);

impl warp::reject::Reject for IoError {}

// Custom rejection for payload too large
#[derive(Debug)]
pub struct PayloadTooLarge;

impl warp::reject::Reject for PayloadTooLarge {} 