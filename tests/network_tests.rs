#[allow(deprecated)]
use rand_os::OsRng;
use shinkai_node::network::network::ephemeral_start_server;
use shinkai_node::network::{Client, Opt};
use shinkai_node::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use shinkai_node::shinkai_message_proto::Field;
use tokio::task;
use tokio::time::{sleep, Duration};
use x25519_dalek::{PublicKey, StaticSecret};

#[tokio::test]
async fn test_message_exchange() {
    // Spawn the server task
    let server = task::spawn(ephemeral_start_server());

    // Give the server a moment to start up
    sleep(Duration::from_millis(100)).await;

    let opt = Opt {
        retries: 3, // adjust number of retries as needed
        delay: 1,   // adjust delay as needed
    };

    // Create a new mutable client
    let mut client = match Client::new(opt, "127.0.0.1", 8080).await {
        Ok(client) => client,
        Err(e) => panic!("Failed to create a new client: {:?}", e),
    };

    // Spawn the client task and send a message
    let client = task::spawn(async move {
        #[allow(deprecated)]
        let mut csprng = OsRng::new().unwrap();
        let secret_key = StaticSecret::new(&mut csprng);
        let secret_key_clone = secret_key.clone();
        let public_key = PublicKey::from(&secret_key);

        let fields = vec![Field {
            name: "field1".to_string(),
            r#type: "type1".to_string(),
        }];

        let message_result = ShinkaiMessageBuilder::new(secret_key, public_key)
            .body("body content".to_string())
            .encryption("default".to_string())
            .message_schema_type("schema type".to_string(), fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata(
                "sender".to_string(),
                "recipient".to_string(),
                "scheduled_time".to_string(),
                "signature".to_string(),
            )
            .build();
        println!("{:#?}", message_result);

        let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(message_result.unwrap());
        println!("Encoded message: {:?}", encoded_msg);

        let response = client.send(encoded_msg).await.unwrap();

        println!("Response message: {:?}", response);

        // Decode the response
        let decoded_response = ShinkaiMessageHandler::decode_message(response).unwrap();

        // Check if the response is "Pong"
        assert_eq!(decoded_response.body.unwrap().content, "ACK");

        client.terminate(secret_key_clone, public_key).await;
    });

    // Wait for both tasks to complete
    let _ = server.await;
    let _ = client.await;
}

#[tokio::test]
async fn ping_pong_test_exchange() {
    // print hte name of the test
    println!("ping_pong_test_exchange");

    // Spawn the server task
    let server = task::spawn(ephemeral_start_server());

    // Give the server a moment to start up
    sleep(Duration::from_millis(100)).await;

    let opt = Opt {
        retries: 3, // adjust number of retries as needed
        delay: 1,   // adjust delay as needed
    };

    // Create a new client
    let mut client = match Client::new(opt, "127.0.0.1", 8080).await {
        Ok(client) => client,
        Err(e) => panic!("Failed to create a new client: {:?}", e),
    };

    // Spawn the client task and send a message
    let client = task::spawn(async move {
        #[allow(deprecated)]
        let mut csprng = OsRng::new().unwrap();
        let secret_key = StaticSecret::new(&mut csprng);
        let secret_key_clone = secret_key.clone();
        let public_key = PublicKey::from(&secret_key);

        // Construct a "Ping" message using ShinkaiMessageBuilder
        let message_result =
            ShinkaiMessageBuilder::ping_pong_message("Ping".to_string(), secret_key, public_key);
        println!("{:#?}", message_result);

        // Expecting an "Ok" from the builder, panic if not.
        let msg = match message_result {
            Ok(msg) => msg,
            Err(e) => panic!("Failed to create a new message: {:?}", e),
        };

        // Encode the "Ping" message
        let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(msg);

        // Send the encoded "Ping" message to the server and get the response
        let response = client.send(encoded_msg).await.unwrap();

        // Decode the response
        let decoded_response = ShinkaiMessageHandler::decode_message(response).unwrap();

        // Check if the response is "Pong"
        assert_eq!(decoded_response.body.unwrap().content, "Pong");

        client.terminate(secret_key_clone, public_key).await;
    });

    // Wait for both tasks to complete
    let _ = server.await;
    let _ = client.await;
}
