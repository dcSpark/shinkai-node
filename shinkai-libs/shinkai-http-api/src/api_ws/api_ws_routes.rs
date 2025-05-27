use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use warp::{http::StatusCode, Filter, Rejection, Reply};
use crate::api_ws::api_ws_handlers::ws_handler;

/// Handle rejections from the routes
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    if err.is_not_found() {
        return Ok(warp::reply::with_status("Not Found", StatusCode::NOT_FOUND));
    }

    // Log the error
    shinkai_log(
        ShinkaiLogOption::WsAPI,
        ShinkaiLogLevel::Error,
        &format!("unhandled rejection: {:?}", err),
    );

    // Return a generic error message
    Ok(warp::reply::with_status(
        "Internal Server Error",
        StatusCode::INTERNAL_SERVER_ERROR,
    ))
}

/// Create the Warp routes for WebSocket endpoints
pub fn ws_routes() -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    tracing::info!("Setting up WebSocket routes");

    let root_ws = warp::path::end()
        .and(warp::ws())
        .map(move |ws| {
            let ws: warp::ws::Ws = ws;
            ws.on_upgrade(move |socket| ws_handler(socket))
        });

    root_ws
        .with(warp::cors().allow_any_origin())
        .with(warp::log("websocket"))
        .recover(handle_rejection)
}
