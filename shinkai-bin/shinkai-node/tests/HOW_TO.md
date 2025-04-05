# Shinkai Node Testing Guide

This document provides guidance on writing and running tests for the Shinkai Node project.

## Test Structure

Tests in Shinkai Node follow these common patterns:

1. **Setup**: Initialize test environment, create mock servers, and set required variables
2. **Node Creation**: Create nodes with specific configurations
3. **Registration**: Register profiles, devices, and agents
4. **Job Creation**: Create jobs for testing specific functionality
5. **Message Exchange**: Send and receive messages through the job
6. **Verification**: Validate results and response data

## Common Test Patterns

### Single Node Tests

For tests involving a single node:

```rust
run_test_one_node_network(|env| {
    Box::pin(async move {
        // Test implementation
    })
});
```

### Mock External Services

Use the `mockito` crate to create mock servers:

```rust
let mut server = Server::new();
let _m = server
    .mock("POST", "/v1/chat/completions")
    .match_header("authorization", "Bearer mockapikey")
    .with_status(200)
    .with_header("content-type", "application/json")
    .with_body(r#"{ ... }"#)
    .create();
```

### Register Profiles and Devices

```rust
api_initial_registration_with_no_code_for_device(
    node_commands_sender.clone(),
    profile_name,
    identity_name,
    encryption_pk,
    device_encryption_sk.clone(),
    device_identity_sk,
    profile_encryption_sk.clone(),
    profile_identity_sk,
    device_name,
).await;

// Wait for default tools to be ready
let tools_ready = wait_for_default_tools(
    node_commands_sender.clone(),
    api_key_bearer.clone(),
    20, // Wait up to 20 seconds
)
.await
.expect("Failed to check for default tools");
assert!(tools_ready, "Default tools should be ready within 20 seconds");
```

### Register LLM Providers (Agents)

```rust
// Create agent name
let agent_name = ShinkaiName::new(
    format!(
        "{}/{}/agent/{}",
        identity_name,
        profile_name,
        llm_provider
    )
    .to_string(),
).unwrap();

// Create provider configuration
let provider = SerializedLLMProvider {
    id: llm_provider.to_string(),
    full_identity_name: agent_name,
    external_url: Some(server.url()),
    api_key: Some("apikey"),
    model: LLMProviderInterface::OpenAI(model_config),
};

// Register the provider
api_llm_provider_registration(
    node_commands_sender.clone(),
    clone_static_secret_key(&profile_encryption_sk),
    encryption_pk,
    clone_signature_secret_key(&profile_identity_sk),
    identity_name,
    profile_name,
    provider,
).await;
```

### Create and Use Jobs

```rust
// Create a job
let job_id = api_create_job(
    node_commands_sender.clone(),
    clone_static_secret_key(&profile_encryption_sk),
    encryption_pk,
    clone_signature_secret_key(&profile_identity_sk),
    identity_name,
    profile_name,
    &agent_subidentity,
).await;

// Send a message to the job
api_message_job(
    node_commands_sender.clone(),
    clone_static_secret_key(&profile_encryption_sk),
    encryption_pk,
    clone_signature_secret_key(&profile_identity_sk),
    identity_name,
    profile_name,
    &agent_subidentity,
    &job_id,
    "Your message here",
    &[], // file paths
    "", // parent message ID (empty for first message)
).await;
```

### Working with Files and Vector Database

Use the following helper functions:

```rust
// Create folders
create_folder(
    &node_commands_sender,
    "/",
    "folder_name",
    profile_encryption_sk.clone(),
    clone_signature_secret_key(&profile_identity_sk),
    encryption_pk,
    identity_name,
    profile_name,
).await;

// Upload files
upload_file(
    &node_commands_sender,
    "/folder_path",
    file_path,
    &api_key_bearer,
).await;

// Upload files to a job
upload_file_to_job(
    &node_commands_sender,
    &job_id,
    file_path,
    &api_key_bearer
).await;
```

### Waiting for Results

Create helper functions to wait for responses:

```rust
async fn wait_for_response(node_commands_sender: async_channel::Sender<NodeCommand>) {
    let start = Instant::now();
    loop {
        let (res_sender, res_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::FetchLastMessages {
                limit: 2,
                res: res_sender,
            })
            .await
            .unwrap();
        let messages = res_receiver.recv().await.unwrap();
        
        if /* condition to check if response is ready */ {
            break;
        }

        if start.elapsed() > Duration::from_secs(10) {
            panic!("Timeout waiting for response");
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
```

## Complete Example Test

Here's a complete example of a modern test that:
1. Sets up a single node
2. Registers a profile and LLM provider
3. Creates a job
4. Sends a message
5. Verifies the response
6. Properly terminates the node process

First, it defines a helper function to wait for a response:

```rust
async fn wait_for_response(node1_commands_sender: async_channel::Sender<NodeCommand>) {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node1_commands_sender
        .send(NodeCommand::FetchLastMessages {
            limit: 1,
            res: res_sender,
        })
        .await
        .unwrap();
    let node1_last_messages = res_receiver.recv().await.unwrap();
    let msg_hash = node1_last_messages[0].calculate_message_hash_for_pagination();

    let start = Instant::now();
    loop {
        let (res_sender, res_receiver) = async_channel::bounded(1);
        node1_commands_sender
            .send(NodeCommand::FetchLastMessages {
                limit: 2, // Get the last 2 messages (query and response)
                res: res_sender,
            })
            .await
            .unwrap();
        let node1_last_messages = res_receiver.recv().await.unwrap();

        if node1_last_messages.len() == 2 && node1_last_messages[1].calculate_message_hash_for_pagination() == msg_hash {
            break;
        }

        if start.elapsed() > Duration::from_secs(10) {
            panic!("Test failed: 10 seconds have passed without receiving the response");
        }

        tokio::time::sleep(Duration::from_millis(200)).await; // Short sleep to prevent tight looping
    }
}
```

Then, it implements the test using a structured approach with code blocks:

```rust
#[test]
fn simple_job_message_test() {
    // Set required environment variables
    std::env::set_var("WELCOME_MESSAGE", "false");

    // Create a mock server for OpenAI API
    let mut server = Server::new();

    run_test_one_node_network(|env| {
        Box::pin(async move {
            // Extract environment variables from the test setup
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_llm_provider.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_api_key = env.node1_api_key.clone();
            let node1_abort_handler = env.node1_abort_handler;

            {
                // 1. Setup mock OpenAI response
                eprintln!("\n\nSetting up mock OpenAI server");
                let _m = server
                    .mock("POST", "/v1/chat/completions")
                    .match_header("authorization", "Bearer mockapikey")
                    .with_status(200)
                    .with_header("content-type", "application/json")
                    .with_body(
                        r#"{
                            "id": "chatcmpl-123",
                            "object": "chat.completion",
                            "created": 1677652288,
                            "choices": [{
                                "index": 0,
                                "message": {
                                    "role": "assistant",
                                    "content": "This is a test response from the mock server"
                                },
                                "finish_reason": "stop"
                            }],
                            "usage": {
                                "prompt_tokens": 9,
                                "completion_tokens": 12,
                                "total_tokens": 21
                            }
                        }"#,
                    )
                    .create();
            }

            {
                // 2. Register device and profile
                eprintln!("\n\nRegistering device and profile");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    node1_device_identity_sk,
                    node1_profile_encryption_sk.clone(),
                    node1_profile_identity_sk.clone(),
                    node1_device_name.as_str(),
                )
                .await;
            }

            {
                // 3. Wait for default tools to be ready
                eprintln!("\n\nWaiting for default tools to be ready");
                let tools_ready = wait_for_default_tools(
                    node1_commands_sender.clone(),
                    node1_api_key.clone(),
                    20, // Wait up to 20 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 20 seconds");
            }

            {
                // 4. Check that Rust tools are installed
                eprintln!("\n\nWaiting for Rust tools installation");
                match wait_for_rust_tools(node1_commands_sender.clone(), 20).await {
                    Ok(retry_count) => {
                        eprintln!("Rust tools were installed after {} retries", retry_count);
                    }
                    Err(e) => {
                        panic!("{}", e);
                    }
                }
            }

            {
                // 5. Register an LLM provider (agent)
                eprintln!("\n\nRegistering LLM provider");
                let agent_name = ShinkaiName::new(
                    format!("{}/{}/agent/{}", node1_identity_name, node1_profile_name, node1_agent).to_string(),
                )
                .unwrap();

                let open_ai = OpenAI {
                    model_type: "gpt-4-turbo".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_agent.to_string(),
                    full_identity_name: agent_name,
                    external_url: Some(server.url()),
                    api_key: Some("mockapikey".to_string()),
                    model: LLMProviderInterface::OpenAI(open_ai),
                };

                api_llm_provider_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    node1_profile_identity_sk.clone(),
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    agent,
                )
                .await;
            }

            let mut job_id: String;
            let agent_subidentity = format!("{}/agent/{}", node1_profile_name, node1_agent);

            {
                // 6. Create a job
                eprintln!("\n\nCreating a job");
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    node1_profile_identity_sk.clone(),
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    &agent_subidentity,
                )
                .await;
            }

            {
                // 7. Update job config to turn off streaming
                eprintln!("\n\nUpdating job config to turn off streaming");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiUpdateJobConfig {
                        bearer: node1_api_key.clone(),
                        job_id: job_id.clone(),
                        config: JobConfig {
                            stream: Some(false),
                            ..JobConfig::empty()
                        },
                        res: res_sender,
                    })
                    .await
                    .unwrap();
                let result = res_receiver.recv().await.unwrap();
                assert!(result.is_ok(), "Failed to update job config: {:?}", result);
            }

            {
                // 8. Send a message to the job
                eprintln!("\n\nSending a message to the job");
                let message_content = "This is a test message";
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    node1_profile_identity_sk.clone(),
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    &agent_subidentity,
                    &job_id,
                    message_content,
                    &[], // No file paths
                    "",  // No parent message
                )
                .await;
            }

            {
                // 9. Wait for and verify the response
                eprintln!("\n\nWaiting for response");
                wait_for_response(node1_commands_sender.clone()).await;

                // Verify the response content
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::FetchLastMessages {
                        limit: 2,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let messages = res_receiver.recv().await.unwrap();
                let response_content = messages[0]
                    .get_message_content()
                    .expect("Failed to get message content");

                assert!(
                    response_content.contains("This is a test response from the mock server"),
                    "Response content did not match expected: {}",
                    response_content
                );

                eprintln!("Test completed successfully");
                
                // 10. Terminate the node process
                node1_abort_handler.abort();
                return;
            }
        })
    });
}
```

Note the key improvements in this test structure:
1. Code is organized into logical blocks with clear comments
2. Helper functions (`wait_for_response`, `wait_for_default_tools`, `wait_for_rust_tools`) are used to encapsulate common patterns
3. The test properly terminates the node process with `node1_abort_handler.abort()`
4. Each step is clearly numbered and labeled with appropriate logging

## Test Types

### API and Command Tests

Tests that verify node commands and API endpoints work correctly:

- **Native Tool Tests**: Test the integration with native tools like vector database
- **Job Tests**: Test job creation and message processing
- **Fork and Branch Tests**: Test job forking and branching capabilities

### Image Processing Tests

Tests for image analysis capabilities:

```rust
// Upload an image file
let file_path = Path::new("../../files/image.png");
upload_file_to_job(&node_commands_sender, &job_id, file_path, &api_key_bearer).await;

// Send a message referencing the image
api_message_job(
    node_commands_sender.clone(),
    /* other parameters */
    "describe the image",
    &file_paths_str, // Array of file paths
    "",
).await;
```

## Best Practices

1. **Mock External Services**: Always mock external services
2. **Clean Up**: Clean up test resources after completion
3. **Error Handling**: Use proper error handling and assertions
4. **Timeouts**: Implement timeouts for async operations
5. **Logging**: Add clear log messages to help with debugging

## Running Tests

Run a specific test:
```bash
cargo test --test it job_image_analysis -- --nocapture
```

Run all tests:
```bash
cargo test --test it -- --nocapture
```

Skip long-running tests:
```bash
cargo test --test it -- --skip job_image_analysis
```
