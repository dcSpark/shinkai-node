use crate::api_sse::api_sse_handlers::{
    sse_handler, post_event_handler, update_tools_cache_handler,
    McpState, IoError, PayloadTooLarge, SessionExpired};
use crate::api_sse::mcp_tools_service::McpToolsService;
use crate::node_commands::NodeCommand;
use async_channel::Sender;
use std::sync::Arc;
use warp::{http::StatusCode, Filter, Rejection, Reply};

/// Handle rejections from the routes
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    let status;
    let message;

    if err.is_not_found() {
        status = StatusCode::NOT_FOUND;
        message = "Resource not found".to_string();
        tracing::warn!("SSE route rejection: {}", message);
    } else if err.find::<PayloadTooLarge>().is_some() {
        status = StatusCode::PAYLOAD_TOO_LARGE;
        message = "Payload too large".to_string();
        tracing::warn!("SSE route rejection: {}", message);
    } else if err.find::<IoError>().is_some() {
        status = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal server error".to_string();
        tracing::error!("SSE route rejection: {}", message);
    } else if err.find::<SessionExpired>().is_some() {
        status = StatusCode::NOT_FOUND; // Or perhaps GONE (410)
        message = "Session not found or expired".to_string();
        tracing::warn!("SSE route rejection: {}", message);
    } else if let Some(e) = err.find::<warp::reject::MethodNotAllowed>() {
         status = StatusCode::METHOD_NOT_ALLOWED;
         message = format!("Method not allowed: {}", e);
         tracing::warn!("SSE route rejection: {}", message);
    } else {
        status = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Unknown error occurred".to_string();
        tracing::error!(rejection = ?err, "SSE route unknown rejection type: {}", message);
    }

    Ok(warp::reply::with_status(warp::reply::json(&serde_json::json!({ "error": message })), status))
}

/// Create the Warp routes for MCP SSE endpoints
pub fn mcp_sse_routes(
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    tracing::info!("Setting up MCP SSE routes with node name: {}", node_name);
    
    // Create the tools service (used by sse_handler)
    let tools_service = Arc::new(McpToolsService::new(node_commands_sender.clone(), node_name.clone()));
    // Clone the Arc before moving it into the async block
    let tools_service_for_cache = tools_service.clone(); 
    // Update the cache immediately (can also be updated periodically or on demand)
    tokio::spawn(async move {
        if let Err(e) = tools_service_for_cache.update_tools_cache().await {
            tracing::error!("Initial tools cache update failed: {:?}", e);
        }
    });

    // Create the state
    let state = Arc::new(McpState::new());
    tracing::info!("Created MCP state");
    
    // SSE endpoint
    let sse = warp::path("sse")
        .and(warp::get())
        .and(with_state(state.clone()))
        .and(with_tools_service(tools_service.clone())) // sse_handler needs the service instance
        .and_then(sse_handler);
    tracing::info!("Set up GET /sse endpoint for SSE connections");

    // Event posting endpoint
    let post_event = warp::path("sse")
        .and(warp::post())
        .and(warp::query::<SessionQuery>())
        .map(|query: SessionQuery| {
            tracing::debug!("Received POST to /sse with sessionId: {}", query.session_id);
            query.session_id
        })
        .and(warp::body::content_length_limit(1024 * 1024 * 4))
        .and(warp::body::bytes())
        .and(with_state(state.clone())) // post_event_handler only needs state
        .and_then(post_event_handler);
    tracing::info!("Set up POST /sse endpoint for client messages");

    let update_cache_route = warp::path("update_tools_cache")
        .and(warp::post()) // Use POST for actions
        .and(with_tools_service(tools_service.clone())) // Inject the service
        .and_then(update_tools_cache_handler);
    tracing::info!("Set up POST /update_tools_cache endpoint");

    // Combine the routes and add rejection handling
    tracing::info!("MCP SSE routes configured successfully");
    sse.or(post_event)
        .or(update_cache_route)
        .recover(handle_rejection)
}

/// Helper to pass the state to handlers
fn with_state(
    state: Arc<McpState>,
) -> impl Filter<Extract = (Arc<McpState>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// Helper to pass the tools service to handlers (only needed by sse_handler now)
fn with_tools_service(
    service: Arc<McpToolsService>,
) -> impl Filter<Extract = (Arc<McpToolsService>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || service.clone())
}

/// Query parameter for the session ID
#[derive(serde::Deserialize, Debug)]
pub struct SessionQuery {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}