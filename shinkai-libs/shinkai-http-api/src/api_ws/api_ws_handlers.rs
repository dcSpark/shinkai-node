use std::sync::Arc;
use warp::filters::ws::{Message, WebSocket};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tokio::sync::Mutex;
use futures::{StreamExt, SinkExt};
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use bytes::Bytes;

pub async fn ws_handler(
    ws: WebSocket,
    ws_address: std::net::SocketAddr,
) {
    // Get the target WebSocket server URL
    let target_url = format!("ws://{}:{}/ws", ws_address.ip(), ws_address.port());
    
    // Connect to the target WebSocket server
    match tokio_tungstenite::connect_async(target_url).await {
        Ok((target_ws, _)) => {
            // Split the client WebSocket into sender and receiver
            let (client_tx, mut client_rx) = ws.split();
            let client_tx = Arc::new(Mutex::new(client_tx));
            
            // Split the target WebSocket into sender and receiver
            let (target_tx, mut target_rx) = target_ws.split();
            let target_tx = Arc::new(Mutex::new(target_tx));

            // Forward messages from client to target
            let target_tx_clone = Arc::clone(&target_tx);
            tokio::spawn(async move {
                while let Some(result) = client_rx.next().await {
                    match result {
                        Ok(msg) => {
                            let tungstenite_msg = if msg.is_text() {
                                TungsteniteMessage::Text(msg.to_str().unwrap_or("").to_string().into())
                            } else if msg.is_binary() {
                                TungsteniteMessage::Binary(Bytes::from(msg.as_bytes().to_vec()))
                            } else {
                                // Ignore other message types for now
                                continue;
                            };
                            let mut lock = target_tx_clone.lock().await;
                            if let Err(e) = lock.send(tungstenite_msg).await {
                                shinkai_log(
                                    ShinkaiLogOption::WsAPI,
                                    ShinkaiLogLevel::Error,
                                    &format!("Error forwarding message to target: {}", e),
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Error,
                                &format!("Error receiving message from client: {}", e),
                            );
                            break;
                        }
                    }
                }
            });

            // Forward messages from target to client
            let client_tx_clone = Arc::clone(&client_tx);
            tokio::spawn(async move {
                while let Some(result) = target_rx.next().await {
                    match result {
                        Ok(msg) => {
                            if let TungsteniteMessage::Text(txt) = msg {
                                let warp_msg = Message::text(txt.as_str());
                                let mut lock = client_tx_clone.lock().await;
                                if let Err(e) = lock.send(warp_msg).await {
                                    shinkai_log(
                                        ShinkaiLogOption::WsAPI,
                                        ShinkaiLogLevel::Error,
                                        &format!("Error forwarding message to client: {}", e),
                                    );
                                    break;
                                }
                            } else if let TungsteniteMessage::Binary(bin) = msg {
                                let warp_msg = Message::binary(bin);
                                let mut lock = client_tx_clone.lock().await;
                                if let Err(e) = lock.send(warp_msg).await {
                                    shinkai_log(
                                        ShinkaiLogOption::WsAPI,
                                        ShinkaiLogLevel::Error,
                                        &format!("Error forwarding message to client: {}", e),
                                    );
                                    break;
                                }
                            } else {
                                continue;
                            }
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::WsAPI,
                                ShinkaiLogLevel::Error,
                                &format!("Error receiving message from target: {}", e),
                            );
                            break;
                        }
                    }
                }
            });
        }
        Err(e) => {
            shinkai_log(
                ShinkaiLogOption::WsAPI,
                ShinkaiLogLevel::Error,
                &format!("Failed to connect to target WebSocket server: {}", e),
            );
            let (mut tx, _) = ws.split();
            let _ = tx.send(Message::text("Failed to connect to target WebSocket server")).await;
        }
    }
}
