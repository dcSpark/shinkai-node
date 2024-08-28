use super::ws_manager::WebSocketManager;
use super::ws_manager::WebSocketManagerError;
use futures::stream::SplitSink;
use futures::SinkExt;
use futures::StreamExt;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
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
                    // Attempt to deserialize the text message into a ShinkaiMessage
                    if let Ok(shinkai_message) = serde_json::from_str::<ShinkaiMessage>(text) {
                        shinkai_log(
                            ShinkaiLogOption::WsAPI,
                            ShinkaiLogLevel::Info,
                            &format!("Received ShinkaiMessage: {:?}", shinkai_message),
                        );

                        // Process the ShinkaiMessage
                        if let Err(e) = process_shinkai_message(&shinkai_message, &manager, &ws_tx).await {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Error,
                                &format!("Error processing ShinkaiMessage: {}", e),
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
                            &format!("Failed to parse ShinkaiMessage. Payload: {:?}", text),
                        );
                        // Handle the parsing error
                        let mut lock = ws_tx.lock().await;
                        let _ = lock.send(Message::text("Failed to parse ShinkaiMessage")).await;
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

async fn process_shinkai_message(
    shinkai_message: &ShinkaiMessage,
    manager: &Arc<Mutex<WebSocketManager>>,
    ws_tx: &Arc<Mutex<SplitSink<WebSocket, warp::ws::Message>>>,
) -> Result<(), WebSocketManagerError> {
    eprintln!("process_shinkai_message with shinkai message: {:?}", shinkai_message);

    let shinkai_name = ShinkaiName::from_shinkai_message_only_using_sender_node_name(shinkai_message)
        .map_err(|e| WebSocketManagerError::UserValidationFailed(format!("Failed to get ShinkaiName: {}", e)))?;

    let mut manager_guard = manager.lock().await;
    manager_guard
        .manage_connections(shinkai_name, shinkai_message.clone(), Arc::clone(ws_tx))
        .await
        .map_err(|e| {
            match e {
                WebSocketManagerError::UserValidationFailed(_) => e,
                WebSocketManagerError::AccessDenied(_) => e,
                _ => WebSocketManagerError::UserValidationFailed(format!("Failed to manage connections: {}", e)),
                // Add additional error handling as needed
            }
        })
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
