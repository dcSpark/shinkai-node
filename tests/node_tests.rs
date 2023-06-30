// use shinkai_node::network::{Node, Opt};
// // use rand_os::OsRng;
// use shinkai_node::shinkai_message::encryption::ephemeral_keys;
// use shinkai_node::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
// use shinkai_node::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
// use shinkai_node::shinkai_message_proto::Field;
// use tokio::task;
// use tokio::time::{sleep, Duration};
// use x25519_dalek::{PublicKey, StaticSecret};

// #[tokio::test]
// async fn test_message_exchange() {
//     let server_opt = Opt {
//         retries: 3, // adjust number of retries as needed
//         delay: 1,   // adjust delay as needed
//     };

//     let client_opt = Opt {
//         retries: 3, // adjust number of retries as needed
//         delay: 1,   // adjust delay as needed
//     };

//     // Create the server
//     let (server_sk, server_pk) = ephemeral_keys();
//     let server_addr = "127.0.0.1:8080".to_string();
//     let mut server = match Node::new(server_opt, server_sk, server_pk, server_addr).await {
//         Ok(node) => node,
//         Err(e) => panic!("Failed to create server: {:?}", e),
//     };
//     // Give the server a moment to start up
//     sleep(Duration::from_millis(100)).await;

//     // Create a new client
//     let (client_sk, client_pk) = ephemeral_keys();
//     let mut client = match Node::new(
//         client_opt,
//         client_sk.clone(),
//         client_pk.clone(),
//         "127.0.0.1:8081".to_string(),
//     )
//     .await
//     {
//         Ok(node) => node,
//         Err(e) => panic!("Failed to create client: {:?}", e),
//     };

//     // Connect the client to the server
//     match client.connect_to_peer("127.0.0.1", 8080).await {
//         Ok(_) => {}
//         Err(e) => panic!("Failed to connect client to server: {:?}", e),
//     }
//     let connection_count = server.connection_count();
//     println!("connection_count: {:?}", connection_count);

//     // Create and send a message
//     let fields = vec![Field {
//         name: "field1".to_string(),
//         r#type: "type1".to_string(),
//     }];

//     let client_message = ShinkaiMessageBuilder::new(client_sk, server_pk)
//         .body("body content".to_string())
//         .encryption("default".to_string())
//         .message_schema_type("schema type".to_string(), fields)
//         .topic("topic_id".to_string(), "channel_id".to_string())
//         .internal_metadata_content("internal metadata content".to_string())
//         .external_metadata(
//             client_pk,
//             "recipient".to_string(),
//             "scheduled_time".to_string(),
//             "signature".to_string(),
//         )
//         .build();

//     let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(client_message.unwrap());
//     let size_in_bytes = encoded_msg.len();
//     println!("Size of encoded_msg in bytes: {}", size_in_bytes);

//     let isUp = server.is_up_and_running();
//     println!("isUp: {:?}", isUp);

//     let connection_count = server.connection_count();
//     println!("connection_count: {:?}", connection_count);

//     match client.send(encoded_msg).await {
//         Ok(Some(response)) => {
//             let decoded_response = ShinkaiMessageHandler::decode_message(response).unwrap();
//             // Check if the response is "ACK"
//             assert_eq!(decoded_response.body.unwrap().content, "ACK");
//             // Terminate the client and server
//             client.terminate().await;
//             server.terminate().await;
//         }
//         Ok(None) => panic!("No response received from the server"),
//         Err(e) => panic!("Failed to send the message: {:?}", e),
//     }
// }

// #[tokio::test]
// async fn ping_pong_test_exchange() {
//     // print the name of the test
//     println!("ping_pong_test_exchange");

//     let server_opt = Opt {
//         retries: 3, // adjust number of retries as needed
//         delay: 1,   // adjust delay as needed
//     };

//     let client_opt = Opt {
//         retries: 3, // adjust number of retries as needed
//         delay: 1,   // adjust delay as needed
//     };

//     // Create the server
//     let (server_sk, server_pk) = ephemeral_keys();
//     let server_addr = "127.0.0.1:8080".to_string();
//     let mut server = match Node::new(server_opt, server_sk, server_pk, server_addr).await {
//         Ok(node) => node,
//         Err(e) => panic!("Failed to create server: {:?}", e),
//     };

//     // Give the server a moment to start up
//     sleep(Duration::from_millis(100)).await;

//     // Create a new client
//     let (client_sk, client_pk) = ephemeral_keys();
//     let mut client = match Node::new(
//         client_opt,
//         client_sk.clone(),
//         client_pk.clone(),
//         "127.0.0.1:8081".to_string(),
//     )
//     .await
//     {
//         Ok(node) => node,
//         Err(e) => panic!("Failed to create client: {:?}", e),
//     };

//     // Connect the client to the server
//     match client.connect_to_peer("127.0.0.1", 8080).await {
//         Ok(_) => {}
//         Err(e) => panic!("Failed to connect client to server: {:?}", e),
//     }

//     // Construct a "Ping" message using ShinkaiMessageBuilder
//     let message_result =
//         ShinkaiMessageBuilder::ping_pong_message("Ping".to_string(), client_sk, client_pk);

//     // Expecting an "Ok" from the builder, panic if not.
//     let msg = match message_result {
//         Ok(msg) => msg,
//         Err(e) => panic!("Failed to create a new message: {:?}", e),
//     };

//     // Encode the "Ping" message
//     let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(msg);

//     // Send the encoded "Ping" message to the server and get the response
//     let response = client.send(encoded_msg).await.unwrap().unwrap();

//     // Decode the response
//     let decoded_response = ShinkaiMessageHandler::decode_message(response).unwrap();

//     // Check if the response is "Pong"
//     assert_eq!(decoded_response.body.unwrap().content, "Pong");

//     // Terminate the client and server
//     client.terminate().await;
//     server.terminate().await;
// }
