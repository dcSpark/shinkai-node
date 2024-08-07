// use super::{node_api::APIError, node_error::NodeError, Node};
// use crate::{managers::sheet_manager::SheetManager, tools::tool_router::ToolRouter};
// use crate::managers::IdentityManager;

// use async_channel::Sender;

// use reqwest::StatusCode;
// use serde_json::{json, Value as JsonValue};
// use shinkai_message_primitives::{
//     schemas::shinkai_name::ShinkaiName,
//     shinkai_message::{
//         shinkai_message::ShinkaiMessage,
//         shinkai_message_schemas::{
//             APIRemoveColumnPayload, APISetCellValuePayload, APISetColumnPayload, MessageSchemaType,
//         },
//     },
// };

// use std::sync::Arc;
// use tokio::sync::Mutex;
// use x25519_dalek::StaticSecret as EncryptionStaticKey;

// impl Node {
//     pub async fn api_set_tool(
//         db: Arc<Mutex<SheetManager>>,
//         node_name: ShinkaiName,
//         identity_manager: Arc<Mutex<IdentityManager>>,
//         encryption_secret_key: EncryptionStaticKey,
//         tool_router: Arc<Mutex<ToolRouter>>,
//         potentially_encrypted_msg: ShinkaiMessage,
//         res: Sender<Result<JsonValue, APIError>>,
//     ) -> Result<(), NodeError> {
//         let (payload, requester_name) = match Self::validate_and_extract_payload::<APISetToolPayload>(
//             node_name.clone(),
//             identity_manager.clone(),
//             encryption_secret_key,
//             potentially_encrypted_msg,
//             MessageSchemaType::SetTool,
//         )
//         .await
//         {
//             Ok(data) => data,
//             Err(api_error) => {
//                 let _ = res.send(Err(api_error)).await;
//                 return Ok(());
//             }
//         };

//         if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
//             let api_error = APIError {
//                 code: StatusCode::BAD_REQUEST.as_u16(),
//                 error: "Bad Request".to_string(),
//                 message: "Invalid node name provided".to_string(),
//             };
//             let _ = res.send(Err(api_error)).await;
//             return Ok(());
//         }

//         let mut tool_router_guard = tool_router.lock().await;
//         match tool_router_guard.set_tool(&payload.tool_id, payload.tool_data).await {
//             Ok(_) => {
//                 let _ = res.send(Ok(json!({"status": "success"}))).await;
//                 Ok(())
//             }
//             Err(err) => {
//                 let api_error = APIError {
//                     code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
//                     error: "Internal Server Error".to_string(),
//                     message: format!("Failed to set tool: {}", err),
//                 };
//                 let _ = res.send(Err(api_error)).await;
//                 Ok(())
//             }
//         }
//     }

//     pub async fn api_remove_tool(
//         db: Arc<Mutex<SheetManager>>,
//         node_name: ShinkaiName,
//         identity_manager: Arc<Mutex<IdentityManager>>,
//         encryption_secret_key: EncryptionStaticKey,
//         tool_router: Arc<Mutex<ToolRouter>>,
//         potentially_encrypted_msg: ShinkaiMessage,
//         res: Sender<Result<JsonValue, APIError>>,
//     ) -> Result<(), NodeError> {
//         let (payload, requester_name) = match Self::validate_and_extract_payload::<APIRemoveToolPayload>(
//             node_name.clone(),
//             identity_manager.clone(),
//             encryption_secret_key,
//             potentially_encrypted_msg,
//             MessageSchemaType::RemoveTool,
//         )
//         .await
//         {
//             Ok(data) => data,
//             Err(api_error) => {
//                 let _ = res.send(Err(api_error)).await;
//                 return Ok(());
//             }
//         };

//         if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
//             let api_error = APIError {
//                 code: StatusCode::BAD_REQUEST.as_u16(),
//                 error: "Bad Request".to_string(),
//                 message: "Invalid node name provided".to_string(),
//             };
//             let _ = res.send(Err(api_error)).await;
//             return Ok(());
//         }

//         let mut tool_router_guard = tool_router.lock().await;
//         match tool_router_guard.remove_tool(&payload.tool_id).await {
//             Ok(_) => {
//                 let _ = res.send(Ok(json!({"status": "success"}))).await;
//                 Ok(())
//             }
//             Err(err) => {
//                 let api_error = APIError {
//                     code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
//                     error: "Internal Server Error".to_string(),
//                     message: format!("Failed to remove tool: {}", err),
//                 };
//                 let _ = res.send(Err(api_error)).await;
//                 Ok(())
//             }
//         }
//     }

//     pub async fn api_get_tool(
//         db: Arc<Mutex<SheetManager>>,
//         node_name: ShinkaiName,
//         identity_manager: Arc<Mutex<IdentityManager>>,
//         encryption_secret_key: EncryptionStaticKey,
//         tool_router: Arc<Mutex<ToolRouter>>,
//         potentially_encrypted_msg: ShinkaiMessage,
//         res: Sender<Result<JsonValue, APIError>>,
//     ) -> Result<(), NodeError> {
//         let (tool_id, requester_name) = match Self::validate_and_extract_payload::<String>(
//             node_name.clone(),
//             identity_manager.clone(),
//             encryption_secret_key,
//             potentially_encrypted_msg,
//             MessageSchemaType::GetTool,
//         )
//         .await
//         {
//             Ok(data) => data,
//             Err(api_error) => {
//                 let _ = res.send(Err(api_error)).await;
//                 return Ok(());
//             }
//         };

//         if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
//             let api_error = APIError {
//                 code: StatusCode::BAD_REQUEST.as_u16(),
//                 error: "Bad Request".to_string(),
//                 message: "Invalid node name provided".to_string(),
//             };
//             let _ = res.send(Err(api_error)).await;
//             return Ok(());
//         }

//         let tool_router_guard = tool_router.lock().await;
//         match tool_router_guard.get_tool(&tool_id).await {
//             Ok(tool) => {
//                 let response = json!(tool);
//                 let _ = res.send(Ok(response)).await;
//                 Ok(())
//             }
//             Err(err) => {
//                 let api_error = APIError {
//                     code: StatusCode::NOT_FOUND.as_u16(),
//                     error: "Not Found".to_string(),
//                     message: format!("Failed to get tool: {}", err),
//                 };
//                 let _ = res.send(Err(api_error)).await;
//                 Ok(())
//             }
//         }
//     }

//     pub async fn api_list_all_tools(
//         db: Arc<Mutex<SheetManager>>,
//         node_name: ShinkaiName,
//         identity_manager: Arc<Mutex<IdentityManager>>,
//         encryption_secret_key: EncryptionStaticKey,
//         tool_router: Arc<Mutex<ToolRouter>>,
//         potentially_encrypted_msg: ShinkaiMessage,
//         res: Sender<Result<JsonValue, APIError>>,
//     ) -> Result<(), NodeError> {
//         let requester_name = match Self::validate_and_extract_payload::<String>(
//             node_name.clone(),
//             identity_manager.clone(),
//             encryption_secret_key,
//             potentially_encrypted_msg,
//             MessageSchemaType::ListAllTools,
//         )
//         .await
//         {
//             Ok((_, requester_name)) => requester_name,
//             Err(api_error) => {
//                 let _ = res.send(Err(api_error)).await;
//                 return Ok(());
//             }
//         };

//         if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
//             let api_error = APIError {
//                 code: StatusCode::BAD_REQUEST.as_u16(),
//                 error: "Bad Request".to_string(),
//                 message: "Invalid node name provided".to_string(),
//             };
//             let _ = res.send(Err(api_error)).await;
//             return Ok(());
//         }

//         let tool_router_guard = tool_router.lock().await;
//         match tool_router_guard.list_all_tools().await {
//             Ok(tools) => {
//                 let response = json!(tools);
//                 let _ = res.send(Ok(response)).await;
//                 Ok(())
//             }
//             Err(err) => {
//                 let api_error = APIError {
//                     code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
//                     error: "Internal Server Error".to_string(),
//                     message: format!("Failed to list all tools: {}", err),
//                 };
//                 let _ = res.send(Err(api_error)).await;
//                 Ok(())
//             }
//         }
//     }
// }
