use crate::api_sse::api_sse_handlers::{post_event_handler, sse_handler, McpState, PayloadTooLarge, IoError};
use crate::api_sse::mcp_router::{McpRouter, SharedMcpRouter};
use std::sync::Arc;
use warp::{http::StatusCode, Filter, Rejection, Reply};

/// Handle rejections from the routes
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    if err.is_not_found() {
        Ok(warp::reply::with_status(
            "Session not found",
            StatusCode::NOT_FOUND,
        ))
    } else if let Some(_) = err.find::<PayloadTooLarge>() {
        Ok(warp::reply::with_status(
            "Payload too large",
            StatusCode::PAYLOAD_TOO_LARGE,
        ))
    } else if let Some(_) = err.find::<IoError>() {
        Ok(warp::reply::with_status(
            "Internal server error",
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    } else {
        Err(err)
    }
}

/// Create the Warp routes for MCP SSE endpoints
pub fn mcp_sse_routes() -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    // Create the state
    let state = Arc::new(McpState::new());
    // Create a McpRouter directly, not wrapped in Arc since SharedMcpRouter is now just McpRouter
    let router = McpRouter::new();

    // SSE endpoint
    let sse = warp::path("sse")
        .and(warp::get())
        .and(with_state(state.clone()))
        .and(with_router(router.clone()))
        .and_then(sse_handler);

    // Event posting endpoint
    let post_event = warp::path("sse")
        .and(warp::post())
        .and(warp::query::<SessionQuery>())
        .map(|query: SessionQuery| query.session_id)
        .and(warp::body::content_length_limit(1024 * 1024 * 4)) // 4MB limit
        .and(warp::body::bytes())
        .and(with_state(state.clone()))
        .and_then(post_event_handler);

    // Combine the routes and add rejection handling
    sse.or(post_event).recover(handle_rejection)
}

/// Helper to pass the state to handlers
fn with_state(
    state: Arc<McpState>,
) -> impl Filter<Extract = (Arc<McpState>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// Helper to pass the router to handlers
fn with_router(
    router: SharedMcpRouter,
) -> impl Filter<Extract = (SharedMcpRouter,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || router.clone())
}

/// Query parameter for the session ID
#[derive(serde::Deserialize)]
pub struct SessionQuery {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}
