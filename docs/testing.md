# Testing Guide for Shinkai Node

This guide explains how to write tests for the Shinkai Node project, focusing on the core test structure and helper functions.

## Core Test Structure

Most tests in Shinkai Node follow this fundamental structure:

```rust
#[test]
fn test_name() {
    run_test_one_node_network(|env| {
        Box::pin(async move {
            // 1. Get environment variables from env
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_encryption_pk = env.node1_encryption_pk.clone();
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_abort_handler = env.node1_abort_handler;

            // 2. Test Steps using helper functions
            {
                // Step 1: Register a Profile and Device
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name.as_str(),
                ).await;
            }

            // Additional test steps...

            // 3. Cleanup
            node1_abort_handler.abort();
        })
    });
}
```

## Key Components

### 1. Test Environment Setup

The `run_test_one_node_network` helper provides a pre-configured test environment with:
- Node configuration
- Command channels
- Encryption keys
- Identity information
- Abort handlers

```rust
run_test_one_node_network(|env| {
    Box::pin(async move {
        // Access environment variables via env struct
        let node1_commands_sender = env.node1_commands_sender.clone();
        // ... other env variables
    })
});
```

### 2. Common Helper Functions

#### Registration Functions
```rust
// Register a device and profile
api_initial_registration_with_no_code_for_device(
    commands_sender,
    profile_name,
    identity_name,
    encryption_pk,
    device_encryption_sk,
    device_identity_sk,
    profile_encryption_sk,
    profile_identity_sk,
    device_name,
).await;

// Register an LLM provider
api_llm_provider_registration(
    commands_sender,
    profile_encryption_sk,
    encryption_pk,
    profile_identity_sk,
    identity_name,
    profile_name,
    agent,
).await;
```

#### Job Management Functions
```rust
// Create a job
api_create_job(
    commands_sender,
    profile_encryption_sk,
    encryption_pk,
    profile_identity_sk,
    identity_name,
    profile_name,
    agent_subidentity,
).await;

// Send message to job
api_message_job(
    commands_sender,
    profile_encryption_sk,
    encryption_pk,
    profile_identity_sk,
    identity_name,
    profile_name,
    agent_subidentity,
    job_id,
    message_content,
    attachments,
    parent_message_id,
).await;
```

## Test Patterns

### 1. Single Node Test
```rust
run_test_one_node_network(|env| {
    Box::pin(async move {
        // 1. Setup registration
        api_initial_registration_with_no_code_for_device(...).await;
        
        // 2. Register capabilities (e.g., LLM provider)
        api_llm_provider_registration(...).await;
        
        // 3. Create and use resources (e.g., jobs)
        let job_id = api_create_job(...).await;
        api_message_job(...).await;
        
        // 4. Verify results
        verify_job_completion(...).await;
        
        // 5. Cleanup
        env.node1_abort_handler.abort();
    })
});
```

### 2. Multi-Node Test
```rust
run_test_two_node_network(|env| {
    Box::pin(async move {
        // 1. Setup both nodes
        api_initial_registration_with_no_code_for_device(env.node1_...).await;
        api_initial_registration_with_no_code_for_device(env.node2_...).await;
        
        // 2. Node interaction tests
        // ... test steps
        
        // 3. Cleanup
        env.node1_abort_handler.abort();
        env.node2_abort_handler.abort();
    })
});
```

## Common Test Scenarios

### 1. Job Tree Usage Test
```rust
// 1. Register device and profile
api_initial_registration_with_no_code_for_device(...).await;

// 2. Register LLM provider
api_llm_provider_registration(...).await;

// 3. Create job
let job_id = api_create_job(...).await;

// 4. Send messages
api_message_job(...).await;
```

### 2. Micropayment Flow Test
```rust
// 1. Register nodes
api_registration_device_node_profile_main(...).await;  // Provider
local_registration_profile_node(...).await;            // Subscriber

// 2. Setup tool offering
api_set_tool_offering(...).await;

// 3. Payment flow
let invoice_id = api_request_invoice(...).await;
api_pay_invoice(...).await;
verify_payment_completion(...).await;
```

## Best Practices

1. **Test Structure**
   - Use appropriate test environment helper (`run_test_one_node_network`, `run_test_two_node_network`)
   - Group related operations in blocks using curly braces
   - Add descriptive comments for each test phase

2. **Helper Functions**
   - Use existing helper functions for common operations
   - Create new helpers for repeated patterns
   - Document helper function parameters

3. **Verification**
   - Add proper verification steps after each operation
   - Use waiting patterns for async operations
   - Add descriptive error messages

4. **Resource Management**
   - Always use the abort handlers at the end of tests
   - Clean up any additional resources created
   - Handle errors appropriately

## Common Issues and Solutions

1. **Test Environment Setup**
   ```rust
   // Wrong: Missing environment variables
   run_test_one_node_network(|_env| {
       Box::pin(async move {
           // Test code
       })
   });

   // Correct: Properly extract environment variables
   run_test_one_node_network(|env| {
       Box::pin(async move {
           let node1_commands_sender = env.node1_commands_sender.clone();
           // ... other variables
       })
   });
   ```

2. **Async Operation Handling**
   ```rust
   // Wrong: No verification
   api_create_job(...).await;
   api_message_job(...).await;

   // Correct: Add verification
   let job_id = api_create_job(...).await;
   verify_job_creation(job_id).await;
   api_message_job(...).await;
   ```

## Test Organization

Tests are organized in the `tests/it/` directory:
```
tests/it/
├── utils/                          # Helper functions and utilities
├── job_tree_usage_tests.rs         # Job-related tests
├── a3_micropayment_flow_tests.rs   # Payment flow tests
└── ...
```

## Contributing New Tests

1. Choose appropriate test environment helper
2. Follow existing test patterns
3. Use helper functions for common operations
4. Add proper verification steps
5. Clean up resources
6. Add descriptive comments 