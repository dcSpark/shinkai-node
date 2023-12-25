use super::ws_manager::WebSocketManager;
use futures::StreamExt;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use tokio::sync::Mutex;
use std::net::SocketAddr;
use futures::SinkExt;
use std::sync::Arc;
use warp::cors;
use warp::ws::Message;
use warp::Filter;
use warp::{filters::ws::WebSocket, ws::Ws};

pub type SharedWebSocketManager = Arc<Mutex<WebSocketManager>>;

#[derive(serde::Deserialize)]
struct WSMessage {
    action: String,
    message: ShinkaiMessage,
}

async fn listen_for_smart_inbox_updates(manager: SharedWebSocketManager) {
    loop {
        // Wait for an update (this is just a placeholder, replace with your actual update detection logic)
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // When an update occurs, send a message over all WebSocket connections
        let message = "A smart inbox update occurred!";
        let message = Message::text(message);

        let connections = {
            let manager = manager.lock().await;
            manager.get_all_connections()
        };

        for ws_tx in connections {
            let mut ws_tx = ws_tx.lock().await;
            let _ = ws_tx.send(message.clone()).await;
        }
    }
}

pub fn ws_route(
    manager: SharedWebSocketManager,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ws")
        .and(warp::ws())
        .and(warp::any().map(move || Arc::clone(&manager)))
        .map(|ws: Ws, manager: SharedWebSocketManager| ws.on_upgrade(move |socket| ws_handler(socket, manager)))
}

pub async fn ws_handler(ws: WebSocket, manager: Arc<Mutex<WebSocketManager>>) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Add the WebSocket sender to the manager
    manager.lock().await.add_connection("some_id".to_string(), ws_tx);

    // Listen for incoming messages
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                // Handle incoming messages here
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

    let ws_route = ws_route(Arc::clone(&manager));

    let ws_routes = ws_route
        .recover(handle_rejection)
        .with(warp::log("websocket"))
        .with(cors().allow_any_origin());

    // Spawn the update listener
    tokio::spawn(listen_for_smart_inbox_updates(Arc::clone(&manager)));

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
