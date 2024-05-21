use std::sync::Arc;
use std::time::Duration;

use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::NetworkMessageError;

#[derive(Debug)]
pub struct NetworkMessage {
    pub identity: String,
    pub message_type: NetworkMessageType,
    pub payload: Vec<u8>,
}

impl NetworkMessage {
    pub async fn read_from_socket(
        socket: Arc<Mutex<TcpStream>>,
        log_with_identity: Option<String>,
    ) -> Result<Self, NetworkMessageError> {
        eprintln!(
            "\n\nread_from_socket> Reading message {}",
            log_with_identity.unwrap_or_default()
        );
        let mut socket = socket.lock().await;

        async fn read_with_timeout(socket: &mut TcpStream, buf: &mut [u8]) -> Result<(), NetworkMessageError> {
            let mut total_read = 0;
            while total_read < buf.len() {
                let read_result = timeout(Duration::from_millis(500), socket.read(&mut buf[total_read..])).await;
                match read_result {
                    Ok(Ok(0)) => return Err(NetworkMessageError::ConnectionClosed),
                    Ok(Ok(n)) => total_read += n,
                    Ok(Err(e)) => return Err(NetworkMessageError::IoError(e)),
                    Err(_) => return Err(NetworkMessageError::Timeout),
                }
            }
            Ok(())
        }

        let mut length_bytes = [0u8; 4];
        read_with_timeout(&mut socket, &mut length_bytes).await?;
        let total_length = u32::from_be_bytes(length_bytes) as usize;
        println!("read_from_socket> Read total length: {}", total_length);

        let mut identity_length_bytes = [0u8; 4];
        read_with_timeout(&mut socket, &mut identity_length_bytes).await?;
        let identity_length = u32::from_be_bytes(identity_length_bytes) as usize;
        println!("read_from_socket> Read identity length: {}", identity_length);

        let mut identity_bytes = vec![0u8; identity_length];
        read_with_timeout(&mut socket, &mut identity_bytes).await?;
        println!("read_from_socket> Read identity bytes length: {}", identity_bytes.len());
        let identity = String::from_utf8(identity_bytes)?;
        eprintln!("read_from_socket> Read identity: {}", identity);

        let mut header_byte = [0u8; 1];
        read_with_timeout(&mut socket, &mut header_byte).await?;
        let message_type = match header_byte[0] {
            0x01 => NetworkMessageType::ShinkaiMessage,
            0x02 => NetworkMessageType::VRKaiPathPair,
            0x03 => NetworkMessageType::ProxyMessage,
            _ => return Err(NetworkMessageError::UnknownMessageType(header_byte[0])),
        };
        println!("read_from_socket> Read message type: {}", header_byte[0]);

        let msg_length = total_length - 1 - 4 - identity_length;
        let mut buffer = vec![0u8; msg_length];
        println!("read_from_socket> Calculated payload length: {}", msg_length);

        read_with_timeout(&mut socket, &mut buffer).await?;
        println!("read_from_socket> Read payload length: {}", buffer.len());

        Ok(NetworkMessage {
            identity,
            message_type,
            payload: buffer,
        })
    }
}
