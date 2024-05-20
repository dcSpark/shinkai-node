use chrono::Utc;
use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::APIAvailableSharedItems;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
use shinkai_tcp_relayer::tcp_server::NetworkMessage;
use shinkai_tcp_relayer::TCPProxy;
use std::convert::TryInto;
use std::env;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::task;
use tokio::time::sleep;
use tokio::time::Duration;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

const RELAYER_IDENTITY: &str = "@@tcp_replay_test.sepolia-shinkai";
const RELAYER_ENCRYPTION_PRIVATE_KEY: &str = "e083ca3250c9dac7e79d0cc91fdabce9af748c38d6435fe023ad2b8cae87c155";
const RELAYER_SIGNATURE_PRIVATE_KEY: &str = "ed25dc9689b0e2876c60b65a2565cb3ec74425b6b6fca3b28d49d294368b320d";

fn get_keys() -> (SigningKey, EncryptionStaticKey) {
    let encryption_private_key_bytes = hex::decode(RELAYER_ENCRYPTION_PRIVATE_KEY).unwrap();
    let encryption_private_key_array: [u8; 32] = encryption_private_key_bytes
        .try_into()
        .expect("slice with incorrect length");
    let encryption_private_key = EncryptionStaticKey::from(encryption_private_key_array);

    let signature_private_key_bytes = hex::decode(RELAYER_SIGNATURE_PRIVATE_KEY).unwrap();
    let signature_private_key_array: [u8; 32] = signature_private_key_bytes
        .try_into()
        .expect("slice with incorrect length");
    let signature_private_key = SigningKey::from_bytes(&signature_private_key_array);

    (signature_private_key, encryption_private_key)
}

// #[tokio::test]
async fn test_handle_client_localhost() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Fetch the on-chain identity of the relayer
    let (_relayer_verifying_key, relayer_encryption_key) = get_onchain_identity(RELAYER_IDENTITY).await;

    // Create a TCPProxy instance
    let (signature_key, encryption_key) = get_keys();
    let proxy = TCPProxy::new(
        Some(signature_key),
        Some(encryption_key),
        Some(RELAYER_IDENTITY.to_string()),
    )
    .await
    .unwrap();

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

    let identity = "localhost.shinkai";
    let (identity_sk, _identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (encryption_sk, _encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    // Initial Message
    let msg = initial_shinkai_msg(
        encryption_sk,
        identity_sk,
        relayer_encryption_key,
        "".to_string(),
        identity.to_string(),
        RELAYER_IDENTITY.to_string(),
    );

    // Send a mock identity message
    let identity_msg = NetworkMessage {
        identity: identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: msg.encode_message().unwrap()
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
async fn test_message_from_localhost_to_farcaster_xyz() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a TCPProxy instance
    let (signature_key, encryption_key) = get_keys();
    let proxy = TCPProxy::new(
        Some(signature_key),
        Some(encryption_key),
        Some(RELAYER_IDENTITY.to_string()),
    )
    .await
    .unwrap();

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

    // Connect localhost client
    let mut localhost_socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let localhost_identity = "localhost.sepolia-shinkai";

    // Create the payload using create_shinkai_message_for_shared_files
    let payload = create_shinkai_message_for_shared_files(
        "@@localhost.sepolia-shinkai",
        "@@farcaster_xyz.sepolia-shinkai",
        "main",
    )
    .await;

    let localhost_msg = NetworkMessage {
        identity: localhost_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload,
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
        "@@farcaster_xyz.sepolia-shinkai",
        "Message from localhost to farcaster_xyz",
    )
    .await;

    // Send a message from localhost to farcaster_xyz
    let message_to_nico = NetworkMessage {
        identity: localhost_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: encoded_msg,
    };
    send_network_message(&mut localhost_socket, &message_to_nico).await;
    eprintln!("Sent message from localhost to farcaster_xyz");

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

// #[tokio::test]
async fn test_message_from_localhost_to_farcaster_xyz_to_localhost() {
    // Setup a TCP listener
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a TCPProxy instance
    let (signature_key, encryption_key) = get_keys();
    let proxy = TCPProxy::new(
        Some(signature_key),
        Some(encryption_key),
        Some(RELAYER_IDENTITY.to_string()),
    )
    .await
    .unwrap();

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

    // Connect localhost client
    let mut localhost_socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let localhost_identity = "localhost.sepolia-shinkai";
    let localhost_msg = NetworkMessage {
        identity: localhost_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: b"Hello from localhost!".to_vec(),
    };
    send_network_message(&mut localhost_socket, &localhost_msg).await;
    eprintln!("Sent identity message for localhost");

    // Handle validation for localhost
    handle_validation(&mut localhost_socket).await;
    eprintln!("localhost connected");

    // Connect nico_requester client
    let mut nico_socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let nico_identity = "farcaster_xyz.sepolia-shinkai";
    let nico_msg = NetworkMessage {
        identity: nico_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: b"Hello from nico_requester!".to_vec(),
    };
    send_network_message(&mut nico_socket, &nico_msg).await;
    eprintln!("Sent identity message for nico_requester");
    sleep(Duration::from_millis(500)).await;

    // Confirm nico_requester connection
    {
        let clients = proxy.clients.lock().await;
        eprintln!("{:?}", clients.keys());
        assert!(clients.contains_key(nico_identity));
        eprintln!("nico_requester connected");
    }

    // Create a valid ShinkaiMessage
    let encoded_msg = create_shinkai_message(
        "@@farcaster_xyz.sepolia-shinkai",
        "@@localhost.sepolia-shinkai",
        "Message from nico_requester to localhost",
    )
    .await;

    // Send a message from nico_requester to localhost
    let message_to_localhost = NetworkMessage {
        identity: nico_identity.to_string(),
        message_type: NetworkMessageType::ShinkaiMessage,
        payload: encoded_msg,
    };
    send_network_message(&mut nico_socket, &message_to_localhost).await;
    eprintln!("Sent message from nico_requester to localhost");

    // Read the confirmation message from the server
    let mut len_buffer = [0u8; 4];
    nico_socket.read_exact(&mut len_buffer).await.unwrap();
    let message_len = u32::from_be_bytes(len_buffer) as usize;

    let mut buffer = vec![0u8; message_len];
    nico_socket.read_exact(&mut buffer).await.unwrap();
    let confirmation_message = String::from_utf8(buffer).unwrap();
    eprintln!("Received confirmation on nico_requester: {}", confirmation_message);

    // Read the message from the server on nico_requester
    let mut len_buffer = [0u8; 4];
    nico_socket.read_exact(&mut len_buffer).await.unwrap();
    let message_len = u32::from_be_bytes(len_buffer) as usize;

    let mut buffer = vec![0u8; message_len];
    nico_socket.read_exact(&mut buffer).await.unwrap();
    let received_message = String::from_utf8(buffer).unwrap();
    eprintln!("Received message on nico_requester: {}", received_message);

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

#[allow(dead_code)]
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

#[allow(clippy::too_many_arguments)]
pub fn generate_message_with_payload<T: ToString>(
    payload: T,
    schema: MessageSchemaType,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    sender: &str,
    sender_subidentity: &str,
    recipient: &str,
    recipient_subidentity: &str,
) -> ShinkaiMessage {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%f").to_string();

    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(payload.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(schema)
        .internal_metadata_with_inbox(
            sender_subidentity.to_string(),
            recipient_subidentity.to_string(),
            "".to_string(),
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(recipient.to_string(), sender.to_string(), timestamp)
        .build()
        .unwrap()
}

async fn create_shinkai_message_for_shared_files(
    sender: &str,
    recipient: &str,
    streamer_profile_name: &str,
) -> Vec<u8> {
    let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
    let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let payload = APIAvailableSharedItems {
        path: "/".to_string(),
        streamer_node_name: recipient.to_string(),
        streamer_profile_name: streamer_profile_name.to_string(),
    };

    let message = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::AvailableSharedItems,
        my_encryption_sk,
        my_identity_sk,
        node2_encryption_pk,
        sender,
        "",
        recipient,
        streamer_profile_name,
    );

    serde_json::to_vec(&message).unwrap()
}

async fn get_onchain_identity(node_name: &str) -> (VerifyingKey, EncryptionPublicKey) {
    let rpc_url = env::var("RPC_URL").unwrap_or("https://ethereum-sepolia-rpc.publicnode.com".to_string());
    let contract_address =
        env::var("CONTRACT_ADDRESS").unwrap_or("0xDCbBd3364a98E2078e8238508255dD4a2015DD3E".to_string());

    let registry = ShinkaiRegistry::new(&rpc_url, &contract_address, None).await.unwrap();

    // Fetch the public keys from the registry
    let registry_identity = registry.get_identity_record(node_name.to_string()).await.unwrap();
    eprintln!("Registry Identity: {:?}", registry_identity);
    let registry_identity_public_key = registry_identity.signature_verifying_key().unwrap();
    let registry_encryption_public_key = registry_identity.encryption_public_key().unwrap();

    (registry_identity_public_key, registry_encryption_public_key)
}

fn initial_shinkai_msg(
    my_subidentity_encryption_sk: EncryptionStaticKey,
    my_subidentity_signature_sk: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    sender_subidentity: String,
    sender: String,
    receiver: String,
) -> ShinkaiMessage {
    ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
        my_subidentity_encryption_sk,
        my_subidentity_signature_sk,
        receiver_public_key,
        "".to_string(),
        sender_subidentity,
        sender,
        receiver,
        MessageSchemaType::TextContent,
    )
    .unwrap()
}
