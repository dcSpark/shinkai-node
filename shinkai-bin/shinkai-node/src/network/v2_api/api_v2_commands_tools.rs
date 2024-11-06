use serde_json::{Map, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use std::sync::Arc;
use tokio::sync::RwLock;
use shinkai_lancedb::lance_db::shinkai_lance_db::LanceShinkaiDb;
use async_channel::Sender;
use crate::{network::{
        node_error::NodeError,
        Node,
    }, tools::{execute_tool, generate_tool_definitions}};

impl Node {
    pub async fn generate_tool_definitions(
        language: String,
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Convert the String output to a Value
        let definitions = generate_tool_definitions(&language, lance_db).await;
        let value = Value::String(definitions);
        
        // Send the result
        res.send(Ok(value)).await.map_err(|e| NodeError {
            message: format!("Failed to send response: {}", e)
        })?;

        Ok(())
    }
    
    pub async fn execute_command(
        db: Arc<ShinkaiDB>,
        bearer: String,
        lance_db: Arc<RwLock<LanceShinkaiDb>>,
        tool_router_key: String, 
        parameters: Map<String, Value>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Execute the tool directly
        let result = execute_tool(
            tool_router_key.clone(),
            parameters,
            None,
            db,
            lance_db,
            bearer
        ).await;

        match result {
            Ok(result) => {
                println!("[execute_command] Tool execution successful: {}", tool_router_key);
                if let Err(e) = res.send(Ok(result)).await {
                    eprintln!("[execute_command] Failed to send success response: {}", e);
                    return Err(NodeError {
                        message: format!("Failed to send response: {}", e)
                    });
                }
            }
            Err(e) => {
                eprintln!("[execute_command] Tool execution failed: {}", e);
                if let Err(send_err) = res.send(Err(APIError {
                    code: 500,
                    error: "Tool Execution Error".to_string(),
                    message: e.to_string()
                })).await {
                    eprintln!("[execute_command] Failed to send error response: {}", send_err);
                    return Err(NodeError {
                        message: format!("Failed to send error response: {}", send_err)
                    });
                }
            }
        }

        Ok(())
    }
}