use super::ws_manager::WebSocketManager;
use futures::stream::SplitSink;
use futures::SinkExt;
use futures::StreamExt;
use shinkai_message_primitives::schemas::ws_types::WebSocketManagerError;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::AuthenticatedWSMessage;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::cors;
use warp::filters::ws::Message;
use warp::filters::ws::WebSocket;
use warp::Filter;

pub type SharedWebSocketManager = Arc<Mutex<WebSocketManager>>;

pub fn ws_route(
    manager: SharedWebSocketManager,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    {
        let manager = Arc::clone(&manager);
        warp::path!("ws")
            .and(warp::ws())
            .and(warp::any().map(move || Arc::clone(&manager)))
            .map(move |ws, manager| {
                let ws: warp::ws::Ws = ws;
                let manager: SharedWebSocketManager = manager;
                ws.on_upgrade(move |socket| ws_handler(socket, manager))
            })
    }
}

pub async fn ws_handler(ws: WebSocket, manager: Arc<Mutex<WebSocketManager>>) {
    let (ws_tx, mut ws_rx) = ws.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx));

    // Continuously listen for incoming messages
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str() {
                    // Attempt to deserialize the text message into an AuthenticatedWSMessage
                    if let Ok(authenticated_message) = serde_json::from_str::<AuthenticatedWSMessage>(text) {
                        shinkai_log(
                            ShinkaiLogOption::WsAPI,
                            ShinkaiLogLevel::Info,
                            &format!(
                                "Received AuthenticatedWSMessage: bearer_auth={}, message={:?}",
                                authenticated_message.bearer_auth, authenticated_message.message
                            ),
                        );

                        // Process the AuthenticatedWSMessage
                        if let Err(e) = process_authenticated_message(&authenticated_message, &manager, &ws_tx).await {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Error,
                                &format!("Error processing AuthenticatedWSMessage: {}", e),
                            );
                            // Send an error message back to the client
                            let mut lock = ws_tx.lock().await;
                            let _ = lock.send(Message::text(e.to_string())).await;
                            // Depending on the error, you may choose to close the connection
                            // let _ = lock.close().await;
                        }
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::WsAPI,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to parse AuthenticatedWSMessage. Payload: {:?}", text),
                        );
                        // Handle the parsing error
                        let mut lock = ws_tx.lock().await;
                        let _ = lock.send(Message::text("Failed to parse AuthenticatedWSMessage")).await;
                    }
                } else {
                    shinkai_log(
                        ShinkaiLogOption::WsAPI,
                        ShinkaiLogLevel::Error,
                        &format!("Received non-text message: {:?}", msg),
                    );
                    // Handle the case where the message is not text
                    let mut lock = ws_tx.lock().await;
                    let _ = lock.send(Message::text("Received non-text message")).await;
                }
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::WsAPI,
                    ShinkaiLogLevel::Error,
                    &format!("Websocket error: {}", e),
                );
                break; // Exit the loop and end the connection on error
            }
        }
    }

    // Optionally, you can perform any cleanup here if necessary
    shinkai_log(
        ShinkaiLogOption::WsAPI,
        ShinkaiLogLevel::Info,
        "WebSocket connection closed",
    );
}

// Helper function to check bearer token
fn check_bearer_token(api_key: &str, bearer_token: &str) -> Result<(), WebSocketManagerError> {
    // Remove "Bearer " prefix if present
    let token = if bearer_token.starts_with("Bearer ") {
        &bearer_token[7..]
    } else {
        bearer_token
    };

    if token == api_key {
        Ok(())
    } else {
        Err(WebSocketManagerError::AccessDenied("Invalid bearer token".to_string()))
    }
}

// Function to validate bearer token against API key from env or database
async fn validate_bearer_token(bearer_token: &str, manager: &WebSocketManager) -> Result<(), WebSocketManagerError> {
    // Get API key from environment variable or database
    let api_key = match env::var("API_V2_KEY") {
        Ok(api_key) => api_key,
        Err(_) => {
            // If environment variable is not set, try to get from the database
            // First, upgrade the weak reference to the database
            if let Some(db) = manager.get_db() {
                match db.read_api_v2_key() {
                    Ok(Some(key)) => key,
                    Ok(None) => {
                        return Err(WebSocketManagerError::AccessDenied(
                            "No API key found in database".to_string(),
                        ))
                    }
                    Err(e) => {
                        return Err(WebSocketManagerError::AccessDenied(format!(
                            "Error reading API key from database: {}",
                            e
                        )))
                    }
                }
            } else {
                return Err(WebSocketManagerError::AccessDenied(
                    "Database connection not available".to_string(),
                ));
            }
        }
    };

    // Validate the bearer token
    check_bearer_token(&api_key, bearer_token)?;

    // Log successful authentication
    shinkai_log(
        ShinkaiLogOption::WsAPI,
        ShinkaiLogLevel::Info,
        "Bearer token authentication successful",
    );

    Ok(())
}

async fn process_authenticated_message(
    authenticated_message: &AuthenticatedWSMessage,
    manager: &Arc<Mutex<WebSocketManager>>,
    ws_tx: &Arc<Mutex<SplitSink<WebSocket, warp::ws::Message>>>,
) -> Result<(), WebSocketManagerError> {
    // Extract the bearer token
    let bearer_auth = &authenticated_message.bearer_auth;

    // Get manager lock to access database
    {
        let manager_guard = manager.lock().await;
        // Validate the bearer token
        validate_bearer_token(bearer_auth, &manager_guard).await?;
    }

    // Get a new manager guard for the manage_connections call
    let mut manager_guard = manager.lock().await;

    // Call the updated manage_connections method with WSMessage directly
    manager_guard
        .manage_connections(authenticated_message.message.clone(), Arc::clone(ws_tx))
        .await
}

pub async fn run_ws_api(ws_address: SocketAddr, manager: SharedWebSocketManager) {
    shinkai_log(
        ShinkaiLogOption::WsAPI,
        ShinkaiLogLevel::Info,
        &format!("Starting WebSocket server at: {}", &ws_address),
    );

    let ws_routes = ws_route(Arc::clone(&manager))
        .recover(handle_rejection)
        .with(warp::log("websocket"))
        .with(cors().allow_any_origin());

    // Start the WebSocket server
    warp::serve(ws_routes).run(ws_address).await;
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, warp::Rejection> {
    if err.is_not_found() {
        return Ok(warp::reply::with_status("Not Found", warp::http::StatusCode::NOT_FOUND));
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
        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
    ))
}
