# Creating New API Endpoints

This document outlines the process of creating new API endpoints in the Shinkai Node system. The process involves several files and follows a specific pattern to maintain consistency and type safety throughout the codebase.

## Overview

Creating a new endpoint involves modifying four main files:

1. `api_v2_handlers_general.rs` - Defines the HTTP endpoint and request/response handling
2. `node_commands.rs` - Defines the command enum variant
3. `handle_commands_list.rs` - Adds command handling logic
4. `api_v2_commands.rs` - Implements the actual business logic

## Step-by-Step Guide

### 1. Define the HTTP Endpoint (`api_v2_handlers_general.rs`)

First, define your endpoint in the handlers file:

```rust
// 1. Add the route to the general_routes function
let my_new_endpoint_route = warp::path("my_new_endpoint")
    .and(warp::post())  // or get(), depending on your needs
    .and(with_sender(node_commands_sender.clone()))
    .and(warp::header::<String>("authorization"))
    .and(warp::body::json())  // if you need request body
    .and_then(my_new_endpoint_handler);

// 2. Add it to the routes combination
public_keys_route
    .or(health_check_route)
    // ... other routes ...
    .or(my_new_endpoint_route)

// 3. Define the handler function
#[utoipa::path(
    post,
    path = "/v2/my_new_endpoint",
    request_body = MyNewEndpointRequest,  // Your request type
    responses(
        (status = 200, description = "Success response", body = MyNewEndpointResponse),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn my_new_endpoint_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: MyNewEndpointRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiMyNewEndpoint {
            bearer,
            payload,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}
```

### 2. Define the Command (`node_commands.rs`)

Add a new variant to the `NodeCommand` enum:

```rust
pub enum NodeCommand {
    // ... existing commands ...
    
    V2ApiMyNewEndpoint {
        bearer: String,
        payload: MyNewEndpointRequest,
        res: Sender<Result<MyNewEndpointResponse, APIError>>,
    },
}
```

### 3. Add Command Handling (`handle_commands_list.rs`)

Implement the command handling in the `handle_command` function:

```rust
impl Node {
    pub async fn handle_command(&self, command: NodeCommand) {
        match command {
            // ... existing command matches ...
            
            NodeCommand::V2ApiMyNewEndpoint { bearer, payload, res } => {
                let db_clone = Arc::clone(&self.db);
                // Clone any other required resources
                
                tokio::spawn(async move {
                    let _ = Node::v2_api_my_new_endpoint(
                        db_clone,
                        bearer,
                        payload,
                        res
                    ).await;
                });
            }
        }
    }
}
```

### 4. Implement Business Logic (`api_v2_commands.rs`)

Implement the actual endpoint logic:

```rust
impl Node {
    pub async fn v2_api_my_new_endpoint(
        db: Arc<SqliteManager>,
        bearer: String,
        payload: MyNewEndpointRequest,
        res: Sender<Result<MyNewEndpointResponse, APIError>>,
    ) -> Result<(), NodeError> {
        // 1. Validate bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // 2. Implement your business logic
        match do_something_with_payload(db, payload).await {
            Ok(result) => {
                let response = MyNewEndpointResponse { data: result };
                let _ = res.send(Ok(response)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to process request: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
```

## Best Practices

1. **Error Handling**
   - Always validate the bearer token first
   - Use appropriate HTTP status codes
   - Provide meaningful error messages
   - Handle all potential error cases

2. **Type Safety**
   - Define proper request/response types
   - Use strong typing for all parameters
   - Implement proper serialization/deserialization

3. **Documentation**
   - Use `#[utoipa::path]` for OpenAPI documentation
   - Document all public functions and types
   - Include example requests/responses

4. **Testing**
   - Write unit tests for the business logic
   - Add integration tests for the endpoint
   - Test error cases and edge conditions

## Example: Adding a Get Agent Endpoint

Here's a real example from the codebase showing how the get agent endpoint is implemented:

```rust
// 1. Handler (api_v2_handlers_general.rs)
#[utoipa::path(
    get,
    path = "/v2/get_agent/{agent_id}",
    responses(
        (status = 200, description = "Successfully retrieved agent", body = Agent),
        (status = 404, description = "Agent not found", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_agent_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    agent_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    
    sender
        .send(NodeCommand::V2ApiGetAgent {
            bearer,
            agent_id,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(agent) => Ok(warp::reply::json(&agent)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

// 2. Command (node_commands.rs)
pub enum NodeCommand {
    V2ApiGetAgent {
        bearer: String,
        agent_id: String,
        res: Sender<Result<Agent, APIError>>,
    },
}

// 3. Command Handler (handle_commands_list.rs)
impl Node {
    pub async fn handle_command(&self, command: NodeCommand) {
        match command {
            NodeCommand::V2ApiGetAgent { bearer, agent_id, res } => {
                let db_clone = Arc::clone(&self.db);
                tokio::spawn(async move {
                    let _ = Node::v2_api_get_agent(db_clone, bearer, agent_id, res).await;
                });
            }
        }
    }
}

// 4. Implementation (api_v2_commands.rs)
impl Node {
    pub async fn v2_api_get_agent(
        db: Arc<SqliteManager>,
        bearer: String,
        agent_id: String,
        res: Sender<Result<Agent, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the agent from the database
        match db.get_agent(&agent_id) {
            Ok(Some(agent)) => {
                let _ = res.send(Ok(agent)).await;
            }
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "Agent not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve agent: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
``` 