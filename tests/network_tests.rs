use tokio::task;
use tokio::time::{sleep, Duration};
use shinkai_node::network::{start_server, start_client, Opt};

#[tokio::test]
async fn test_message_exchange() {
    // Spawn the server task
    let server = task::spawn(start_server());

    // Give the server a moment to start up
    sleep(Duration::from_millis(100)).await;

    let opt = Opt {
        retries: 3, // adjust number of retries as needed
        delay: 1,   // adjust delay as needed
    };

    // Spawn the client task and send a message
    let client = task::spawn(async {
        let result = start_client(opt).await;
        assert!(result.is_ok(), "Failed to start client");
        assert_eq!(result.unwrap(), "Pong");
    });

    // Wait for both tasks to complete
    let _ = server.await;
    let _ = client.await;
}
