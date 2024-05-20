use ed25519_dalek::Signer;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
use shinkai_tcp_relayer::server::NetworkMessage;
use shinkai_tcp_relayer::TCPProxy;
use std::convert::TryInto;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::task;
use tokio::time::sleep;
use tokio::time::Duration;

#[tokio::test]
async fn test_handle_client_localhost() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a TCPProxy instance
    let proxy = TCPProxy::new().await.unwrap();

    // Spawn a task to accept connections
    let handle = task::spawn({
        let proxy = proxy.clone();
        async move {
            let (socket, _) = listener.accept().await.unwrap();
            proxy.handle_client(socket).await;
        }
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
        let clients = proxy.clients.lock().await;
        assert!(clients.contains_key(identity));
    }

    // Clean up
    handle.abort();
}

#[tokio::test]
async fn test_handle_client() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a TCPProxy instance
    let proxy = TCPProxy::new().await.unwrap();

    // Spawn a task to accept connections
    let handle = task::spawn({
        let proxy = proxy.clone();
        async move {
            let (socket, _) = listener.accept().await.unwrap();
            proxy.handle_client(socket).await;
        }
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
        let clients = proxy.clients.lock().await;
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

    // Create a TCPProxy instance
    let proxy = TCPProxy::new().await.unwrap();

    // Spawn a task to accept connections
    let handle = task::spawn({
        let proxy = proxy.clone();
        async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                proxy.handle_client(socket).await;
            }
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
        let clients = proxy.clients.lock().await;
        eprintln!("{:?}", clients.keys());
        assert!(clients.contains_key(localhost_identity));
        eprintln!("localhost connected");
    }

    // Create a valid ShinkaiMessage
    let encoded_msg = create_shinkai_message(
        "@@localhost.sepolia-shinkai",
        "@@nico_requester.sepolia-shinkai",
        "Message from localhost to nico_requester",
    )
    .await;

    // Send a message from localhost to nico_requester
    let message_to_nico = NetworkMessage {
        identity: localhost_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: encoded_msg,
    };
    send_network_message(&mut localhost_socket, &message_to_nico).await;
    eprintln!("Sent message from localhost to nico_requester");

    // Read the confirmation message from the server
    let mut len_buffer = [0u8; 4];
    localhost_socket.read_exact(&mut len_buffer).await.unwrap();
    let message_len = u32::from_be_bytes(len_buffer) as usize;

    let mut buffer = vec![0u8; message_len];
    localhost_socket.read_exact(&mut buffer).await.unwrap();
    let confirmation_message = String::from_utf8(buffer).unwrap();
    eprintln!("Received confirmation on localhost: {}", confirmation_message);

    // Read the message from the server on localhost
    let mut len_buffer = [0u8; 4];
    localhost_socket.read_exact(&mut len_buffer).await.unwrap();
    let message_len = u32::from_be_bytes(len_buffer) as usize;

    let mut buffer = vec![0u8; message_len];
    localhost_socket.read_exact(&mut buffer).await.unwrap();
    let received_message = String::from_utf8(buffer).unwrap();
    eprintln!("Received message on localhost: {}", received_message);

    // Assert the received message
    assert_eq!(received_message, "OK");

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

async fn create_shinkai_message(sender: &str, recipient: &str, content: &str) -> Vec<u8> {
    let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
    let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(), my_identity_sk, node2_encryption_pk)
        .message_raw_content(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata("".to_string(), "".to_string(), EncryptionMethod::None, None)
        .external_metadata(recipient.to_string(), sender.to_string())
        .build();

    assert!(message_result.is_ok());
    let message = message_result.unwrap();
    serde_json::to_vec(&message).unwrap()
}
