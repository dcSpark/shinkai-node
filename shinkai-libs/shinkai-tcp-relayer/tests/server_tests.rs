use ed25519_dalek::Signer;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use shinkai_tcp_relayer::server::{handle_client, Clients, NetworkMessage};
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::sleep;
use tokio::time::Duration;

// #[tokio::test]
async fn test_handle_client_localhost() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a shared clients map
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    // Spawn a task to accept connections
    let clients_clone = clients.clone();
    let handle = task::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_client(socket, clients_clone).await;
    });

    // Connect to the listener
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

    // Send a mock identity message
    let identity = "localhost.shinkai";
    let identity_msg = NetworkMessage {
        identity: identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: b"Hello, world!".to_vec(),
    };
    send_network_message(&mut socket, &identity_msg).await;
    eprintln!("Sent identity message");
    sleep(Duration::from_millis(100)).await;

    // Check if the client was added to the clients map
    {
        let clients = clients.lock().await;
        assert!(clients.contains_key(identity));
    }

    // Clean up
    handle.abort();
}

// #[tokio::test]
async fn test_handle_client() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a shared clients map
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    // Spawn a task to accept connections
    let clients_clone = clients.clone();
    let handle = task::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_client(socket, clients_clone).await;
    });

    // Connect to the listener
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

    // Send a mock identity message
    let identity = "nico_requester.sepolia-shinkai";
    let identity_msg = NetworkMessage {
        identity: identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: b"Hello, world!".to_vec(),
    };
    send_network_message(&mut socket, &identity_msg).await;

    // Handle validation
    let mut len_buffer = [0u8; 4];
    socket.read_exact(&mut len_buffer).await.unwrap();
    let validation_data_len = u32::from_be_bytes(len_buffer) as usize;

    let mut buffer = vec![0u8; validation_data_len];
    match socket.read_exact(&mut buffer).await {
        Ok(_) => {
            let validation_data = String::from_utf8(buffer).unwrap().trim().to_string();

            // Sign the validation data
            let secret_key_bytes =
                hex::decode("df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119").unwrap();
            let secret_key_array: [u8; 32] = secret_key_bytes.try_into().expect("slice with incorrect length");
            let secret_key = ed25519_dalek::SigningKey::from_bytes(&secret_key_array);
            let signature = secret_key.sign(validation_data.as_bytes());
            let signature_hex = hex::encode(signature.to_bytes());

            // Send the length of the validation data back to the server
            let signature_len = signature_hex.len() as u32;
            let signature_len_bytes = signature_len.to_be_bytes();

            // Send the length of the signed validation data back to the server
            socket.write_all(&signature_len_bytes).await.unwrap();

            // Send the signed validation data back to the server
            match socket.write_all(signature_hex.as_bytes()).await {
                Ok(_) => eprintln!("Sent signed validation data back to server"),
                Err(e) => eprintln!("Failed to send signed validation data: {}", e),
            }

            // Wait for the server to validate the signature
            let mut len_buffer = [0u8; 4];
            socket.read_exact(&mut len_buffer).await.unwrap();
            let response_len = u32::from_be_bytes(len_buffer) as usize;

            let mut response_buffer = vec![0u8; response_len];
            socket.read_exact(&mut response_buffer).await.unwrap();
            let response = String::from_utf8(response_buffer).unwrap();
            eprintln!("Received validation response: {}", response);

            // Assert the validation response
            assert_eq!(response, "Validation successful");
        }
        Err(e) => eprintln!("Failed to read validation data: {}", e),
    }

    // Check if the client was added to the clients map
    {
        let clients = clients.lock().await;
        assert!(clients.contains_key(identity));
    }

    // Clean up
    handle.abort();
    eprintln!("Test completed and handle aborted");
}

#[tokio::test]
async fn test_message_from_localhost_to_nico_requester() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a shared clients map
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    // Spawn a task to accept connections
    let clients_clone = clients.clone();
    let handle = task::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            handle_client(socket, clients_clone.clone()).await;
        }
    });

    // Connect nico_requester client
    let mut nico_socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let nico_identity = "nico_requester.sepolia-shinkai";
    let nico_msg = NetworkMessage {
        identity: nico_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: b"Hello from nico_requester!".to_vec(),
    };
    send_network_message(&mut nico_socket, &nico_msg).await;
    eprintln!("Sent identity message for nico_requester");

    // Handle validation for nico_requester
    handle_validation(&mut nico_socket).await;
    eprintln!("nico_requester connected");

    // Connect localhost client
    let mut localhost_socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let localhost_identity = "localhost.sepolia-shinkai";
    let localhost_msg = NetworkMessage {
        identity: localhost_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: b"Hello from localhost!".to_vec(),
    };
    eprintln!("Sent identity message for localhost");
    send_network_message(&mut localhost_socket, &localhost_msg).await;
    sleep(Duration::from_millis(500)).await;

    // Confirm localhost connection
    {
        let clients = clients.lock().await;
        eprintln!("{:?}", clients.keys());
        assert!(clients.contains_key(localhost_identity));
        eprintln!("localhost connected");
    }

    // Send a message from localhost to nico_requester
    let message_to_nico = NetworkMessage {
        identity: nico_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: b"Message from localhost to nico_requester".to_vec(),
    };
    send_network_message(&mut localhost_socket, &message_to_nico).await;
    eprintln!("Sent message from localhost to nico_requester");

    // Read the message on nico_requester side
    let mut len_buffer = [0u8; 4];
    nico_socket.read_exact(&mut len_buffer).await.unwrap();
    let message_len = u32::from_be_bytes(len_buffer) as usize;

    let mut buffer = vec![0u8; message_len];
    nico_socket.read_exact(&mut buffer).await.unwrap();
    let received_message = String::from_utf8(buffer).unwrap();
    eprintln!("Received message on nico_requester: {}", received_message);

    // Assert the received message
    assert_eq!(received_message, "Message from localhost to nico_requester");

    // Clean up
    handle.abort();
    eprintln!("Test completed and handle aborted");
}

async fn send_network_message(socket: &mut tokio::net::TcpStream, msg: &NetworkMessage) {
    let encoded_msg = msg.payload.clone();
    let identity = &msg.identity;
    let identity_bytes = identity.as_bytes();
    let identity_length = (identity_bytes.len() as u32).to_be_bytes();

    // Prepare the message with a length prefix and identity length
    let total_length = (encoded_msg.len() as u32 + 1 + identity_bytes.len() as u32 + 4).to_be_bytes();

    let mut data_to_send = Vec::new();
    let header_data_to_send = vec![0x01]; // Message type identifier for ShinkaiMessage
    data_to_send.extend_from_slice(&total_length);
    data_to_send.extend_from_slice(&identity_length);
    data_to_send.extend(identity_bytes);
    data_to_send.extend(header_data_to_send);
    data_to_send.extend_from_slice(&encoded_msg);

    // // Print the name and length of each component
    // println!("Total length: {}", u32::from_be_bytes(total_length));
    // println!("Identity length: {}", u32::from_be_bytes(identity_length));
    // println!("Identity bytes length: {}", identity_bytes.len());
    // println!("Message type length: 1");
    // println!("Payload length: {}", encoded_msg.len());

    socket.write_all(&data_to_send).await.unwrap();
    socket.flush().await.unwrap();
}

async fn handle_validation(socket: &mut tokio::net::TcpStream) {
    let mut len_buffer = [0u8; 4];
    socket.read_exact(&mut len_buffer).await.unwrap();
    let validation_data_len = u32::from_be_bytes(len_buffer) as usize;

    let mut buffer = vec![0u8; validation_data_len];
    socket.read_exact(&mut buffer).await.unwrap();
    let validation_data = String::from_utf8(buffer).unwrap().trim().to_string();

    // Sign the validation data
    let secret_key_bytes = hex::decode("df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119").unwrap();
    let secret_key_array: [u8; 32] = secret_key_bytes.try_into().expect("slice with incorrect length");
    let secret_key = ed25519_dalek::SigningKey::from_bytes(&secret_key_array);
    let signature = secret_key.sign(validation_data.as_bytes());
    let signature_hex = hex::encode(signature.to_bytes());

    // Send the length of the signed validation data back to the server
    let signature_len = signature_hex.len() as u32;
    let signature_len_bytes = signature_len.to_be_bytes();
    socket.write_all(&signature_len_bytes).await.unwrap();
    socket.write_all(signature_hex.as_bytes()).await.unwrap();
    eprintln!("Sent signed validation data");

    // Wait for the server to validate the signature
    socket.read_exact(&mut len_buffer).await.unwrap();
    let response_len = u32::from_be_bytes(len_buffer) as usize;

    let mut response_buffer = vec![0u8; response_len];
    socket.read_exact(&mut response_buffer).await.unwrap();
    let response = String::from_utf8(response_buffer).unwrap();
    eprintln!("Received validation response: {}", response);
    assert_eq!(response, "Validation successful");
}
