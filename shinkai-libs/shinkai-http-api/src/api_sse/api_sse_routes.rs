use crate::api_sse::api_sse_handlers::{sse_handler, post_event_handler, McpState, IoError, PayloadTooLarge};
use crate::api_sse::mcp_tools_service::McpToolsService;
use crate::node_commands::NodeCommand;
use async_channel::Sender;
use std::sync::Arc;
use warp::{http::StatusCode, Filter, Rejection, Reply};
use rmcp::{ServiceExt, RoleServer};

/// Handle rejections from the routes
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    if err.is_not_found() {
        tracing::warn!("SSE route rejection: Resource not found");
        Ok(warp::reply::with_status(
            "Session not found",
            StatusCode::NOT_FOUND,
        ))
    } else if let Some(_) = err.find::<PayloadTooLarge>() {
        tracing::warn!("SSE route rejection: Payload too large");
        Ok(warp::reply::with_status(
            "Payload too large",
            StatusCode::PAYLOAD_TOO_LARGE,
        ))
    } else if let Some(_) = err.find::<IoError>() {
        tracing::error!("SSE route rejection: IO error occurred");
        Ok(warp::reply::with_status(
            "Internal server error",
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    } else {
        tracing::error!(rejection = ?err, "SSE route unknown rejection type");
        Ok(warp::reply::with_status(
            "Unknown error occurred",
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

/// Create the Warp routes for MCP SSE endpoints
pub fn mcp_sse_routes(
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    tracing::info!("Setting up MCP SSE routes with node name: {}", node_name);
    
    // Create the tools service
    let tools_service = Arc::new(McpToolsService::new(node_commands_sender, node_name));
    
    // Create the state
    let state = Arc::new(McpState::new());
    tracing::info!("Created MCP state and tools service");
    
    // SSE endpoint
    let sse = warp::path("sse")
        .and(warp::get())
        .and(with_state(state.clone()))
        .and(with_tools_service(tools_service.clone()))
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
        .and(with_state(state.clone()))
        .and_then(post_event_handler);
    tracing::info!("Set up POST /sse endpoint for client messages");

    // Combine the routes and add rejection handling
    tracing::info!("MCP SSE routes configured successfully");
    sse.or(post_event).recover(handle_rejection)
}

/// Helper to pass the state to handlers
fn with_state(
    state: Arc<McpState>,
) -> impl Filter<Extract = (Arc<McpState>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// Helper to pass the tools service to handlers
fn with_tools_service(
    service: Arc<McpToolsService>,
) -> impl Filter<Extract = (Arc<McpToolsService>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || service.clone())
}

/// Query parameter for the session ID
#[derive(serde::Deserialize)]
pub struct SessionQuery {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}
