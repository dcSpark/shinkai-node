use std::sync::Arc;

use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::NetworkMessageError;

#[derive(Debug)]
pub struct NetworkMessage {
    pub identity: String,
    pub message_type: NetworkMessageType,
    pub payload: Vec<u8>,
}

impl NetworkMessage {
    pub async fn read_from_socket(
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        _log_with_identity: Option<String>,
    ) -> Result<Self, NetworkMessageError> {
        let mut reader = reader.lock().await;

        async fn read_exact(reader: &mut ReadHalf<TcpStream>, buf: &mut [u8]) -> Result<(), NetworkMessageError> {
            let mut total_read = 0;
            while total_read < buf.len() {
                match reader.read(&mut buf[total_read..]).await {
                    Ok(0) => return Err(NetworkMessageError::ConnectionClosed),
                    Ok(n) => total_read += n,
                    Err(e) => return Err(NetworkMessageError::IoError(e)),
                }
            }
            Ok(())
        }

        let mut length_bytes = [0u8; 4];
        read_exact(&mut reader, &mut length_bytes).await?;
        let total_length = u32::from_be_bytes(length_bytes) as usize;

        let mut identity_length_bytes = [0u8; 4];
        read_exact(&mut reader, &mut identity_length_bytes).await?;
        let identity_length = u32::from_be_bytes(identity_length_bytes) as usize;

        let mut identity_bytes = vec![0u8; identity_length];
        read_exact(&mut reader, &mut identity_bytes).await?;
        let identity = String::from_utf8(identity_bytes)?;

        let mut header_byte = [0u8; 1];
        read_exact(&mut reader, &mut header_byte).await?;
        let message_type = match header_byte[0] {
            0x01 => NetworkMessageType::ShinkaiMessage,
            0x02 => NetworkMessageType::VRKaiPathPair,
            0x03 => NetworkMessageType::ProxyMessage,
            _ => return Err(NetworkMessageError::UnknownMessageType(header_byte[0])),
        };

        let msg_length = total_length - 1 - 4 - identity_length;
        let mut buffer = vec![0u8; msg_length];

        read_exact(&mut reader, &mut buffer).await?;

        Ok(NetworkMessage {
            identity,
            message_type,
            payload: buffer,
        })
    }
}