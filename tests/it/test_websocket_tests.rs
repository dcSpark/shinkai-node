use std::sync::Arc;
use futures::StreamExt;
use futures::SinkExt;
use shinkai_node::network::{ws_manager::WebSocketManager, ws_routes::run_ws_api};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite;

#[tokio::test]
async fn test_websocket() {
    // Start the WebSocket server
    let manager = Arc::new(Mutex::new(WebSocketManager::new()));
    let ws_address = "127.0.0.1:8080".parse().expect("Failed to parse WebSocket address");
    tokio::spawn(run_ws_api(ws_address, Arc::clone(&manager)));

    // Give the server a little time to start
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Connect to the server
    let (mut ws_stream, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:8080/ws")
        .await
        .expect("Failed to connect");

    // Send a message
    ws_stream
        .send(tungstenite::Message::Text("Hello, world!".into()))
        .await
        .expect("Failed to send message");

    // Check the response
    let msg = ws_stream
        .next()
        .await
        .expect("Failed to read message")
        .expect("Failed to read message");
    assert_eq!(msg.to_text().unwrap(), "A smart inbox update occurred!");
}
