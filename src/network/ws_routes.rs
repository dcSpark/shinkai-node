use super::ws_manager::WebSocketManager;
use futures::StreamExt;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::cors;
use warp::Filter;
use warp::{filters::ws::WebSocket, ws::Ws};

pub type SharedWebSocketManager = Arc<Mutex<WebSocketManager>>;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WSMessage {
    pub action: String,
    pub message: ShinkaiMessage,
}

pub fn ws_route(
    manager: SharedWebSocketManager,
    topic: String,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ws")
        .and(warp::ws())
        .and(warp::any().map(move || Arc::clone(&manager)))
        .and(warp::any().map(move || topic.clone()))
        .map(|ws: Ws, manager: SharedWebSocketManager, topic: String| {
            ws.on_upgrade(move |socket| ws_handler(socket, manager, topic))
        })
}

pub async fn ws_handler(ws: WebSocket, manager: Arc<Mutex<WebSocketManager>>, topic: String) {
    eprintln!("New WebSocket connection");
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Listen for the first incoming message to get the ShinkaiMessage
    if let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str() {
                    if let Ok(ws_message) = serde_json::from_str::<WSMessage>(text) {
                        let shinkai_name = "some_id".to_string(); // Replace with actual shinkai_name
                        let subtopic = "some_subtopic".to_string(); // Replace with actual subtopic

                        match manager
                            .lock()
                            .await
                            .add_connection(shinkai_name, ws_message.message, ws_tx, topic, subtopic)
                            .await
                        {
                            Ok(_) => eprintln!("Connection added successfully"),
                            Err(e) => eprintln!("Failed to add connection: {}", e),
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("websocket error: {}", e);
                return;
            }
        }
    }

    // Continue listening for other incoming messages
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                // Handle other incoming messages here
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

    // TODO: Maybe when a new connection is requested, we need to check permissions
    let topic1_route = ws_route(Arc::clone(&manager), "topic1".to_string());
    let topic2_route = ws_route(Arc::clone(&manager), "topic2".to_string());

    let ws_routes = topic1_route
        .or(topic2_route)
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
