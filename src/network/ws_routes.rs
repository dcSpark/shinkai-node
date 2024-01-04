use super::ws_manager::WebSocketManager;
use futures::SinkExt;
use futures::StreamExt;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSMessage;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::cors;
use warp::filters::ws::WebSocket;
use warp::Filter;

pub type SharedWebSocketManager = Arc<Mutex<WebSocketManager>>;

pub fn ws_route(
    manager: SharedWebSocketManager,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let ws_route = {
        let manager = Arc::clone(&manager);
        warp::path!("ws")
            .and(warp::ws())
            .and(warp::any().map(move || Arc::clone(&manager)))
            .map(move |ws, manager| {
                let ws: warp::ws::Ws = ws;
                let manager: SharedWebSocketManager = manager;
                ws.on_upgrade(move |socket| ws_handler(socket, manager))
            })
    };

    ws_route
}

pub async fn ws_handler(ws: WebSocket, manager: Arc<Mutex<WebSocketManager>>) {
    // Previously: topic: String, subtopic: Option<String>
    eprintln!("New WebSocket connection");
    let (ws_tx, mut ws_rx) = ws.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx));

    // Listen for the first incoming message to get the ShinkaiMessage
    if let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str() {
                    if let Ok(shinkai_message) = serde_json::from_str::<ShinkaiMessage>(text) {
                        eprintln!("ws_message: {:?}", shinkai_message);

                        let mut ws_message: Option<WSMessage> = None;

                        match shinkai_message.get_message_content() {
                            Ok(content_str) => {
                                match serde_json::from_str::<WSMessage>(&content_str) {
                                    Ok(message) => {
                                        ws_message = Some(message);
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to deserialize WSMessage: {}", e);
                                        let mut ws_tx = ws_tx.lock().await;
                                        let _ = ws_tx
                                            .send(warp::ws::Message::text(format!(
                                                "Failed to deserialize WSMessage: {}",
                                                e
                                            )))
                                            .await;
                                        let _ = ws_tx.close().await; // Close the WebSocket connection
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to get message content: {}", e);
                                let mut ws_tx = ws_tx.lock().await;
                                let _ = ws_tx
                                    .send(warp::ws::Message::text(format!("Failed to get message content: {}", e)))
                                    .await;
                                let _ = ws_tx.close().await; // Close the WebSocket connection
                                return;
                            }
                        }

                        let ws_message = ws_message.unwrap();

                        // parse topic and subtopic from type
                        eprintln!("topic: {:?}", ws_message.topic);
                        eprintln!("subtopic: {:?} \n\n", ws_message.subtopic);

                        match ShinkaiName::from_shinkai_message_using_sender_subidentity(&shinkai_message.clone()) {
                            Ok(shinkai_name) => {
                                if let Err(e) = manager
                                    .lock()
                                    .await
                                    .add_connection(shinkai_name, shinkai_message, Arc::clone(&ws_tx), ws_message.topic, ws_message.subtopic)
                                    .await
                                {
                                    eprintln!("Failed to add connection: {}", e);
                                    let mut ws_tx = ws_tx.lock().await;
                                    let _ = ws_tx
                                        .send(warp::ws::Message::text(format!("Failed to add connection: {}", e)))
                                        .await;
                                    let _ = ws_tx.close().await; // Close the WebSocket connection
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to get ShinkaiName: {}", e);
                                let mut ws_tx = ws_tx.lock().await;
                                let _ = ws_tx
                                    .send(warp::ws::Message::text(format!("Failed to get ShinkaiName: {}", e)))
                                    .await;
                                let _ = ws_tx.close().await; // Close the WebSocket connection
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("websocket error: {}", e);
            }
        }
    }

    // Continue listening for other incoming messages
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                // Handle other incoming messages here
                eprintln!("incoming message: {:?}", msg);
            }
            Err(e) => {
                eprintln!("websocket error: {}", e);
                break;
            }
        }
    }
}

pub async fn run_ws_api(ws_address: SocketAddr, manager: SharedWebSocketManager) {
    println!("Starting WebSocket server at: {}", &ws_address);

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
    eprintln!("unhandled rejection: {:?}", err);

    // Return a generic error message
    Ok(warp::reply::with_status(
        "Internal Server Error",
        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
    ))
}
