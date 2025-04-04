use async_channel::Sender;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;
use crate::node_commands::NodeCommand;
use rmcp::{ServerHandler, model::{
    ServerInfo,
    Implementation,
    ProtocolVersion,
    ServerCapabilities,
    Tool,
    InitializeRequestParam,
    InitializeResult,
    ClientRequest,
    ErrorData,
    PaginatedRequestParam,
    ListPromptsResult,
    ListResourcesResult,
    ListToolsResult,
    Content,
    CallToolRequestParam,
    CallToolResult,
},
    service::RequestContext, tool,
    RoleServer,
    model::ErrorData as McpError,
};
use serde_json::{Value, Map, to_string as json_to_string};
use std::borrow::Cow;
use async_trait::async_trait;
use std::future::{self, Future};
use std::collections::HashMap;

// Singleton for the tools cache using once_cell::sync::Lazy
pub static TOOLS_CACHE: Lazy<RwLock<Vec<Tool>>> = Lazy::new(|| RwLock::new(Vec::new()));
// Singleton map from user-facing tool name to internal tool_router_key
pub static TOOL_NAME_TO_KEY_MAP: Lazy<RwLock<HashMap<String, String>>> = Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Clone)]
pub struct McpToolsService {
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
}

impl McpToolsService {
    pub fn new(node_commands_sender: Sender<NodeCommand>, node_name: String) -> Self {
        let service = Self {
            node_commands_sender,
            node_name,
        };
        
        // Spawn a task to update the cache
        let service_clone = service.clone();
        tokio::spawn(async move {
            if let Err(e) = service_clone.update_tools_cache().await {
                tracing::error!("Failed to initialize tools cache: {:?}", e);
            }
        });
        
        service
    }

    /// Get the current list of tools from the cache
    pub fn list_tools(&self) -> Vec<Tool> {
        TOOLS_CACHE.read()
            .expect("Failed to read tools cache")
            .clone()
    }

    /// Update the tools cache and name-to-key map by fetching tools through the node commands
    pub async fn update_tools_cache(&self) -> anyhow::Result<()> {
        // Create a response channel
        let (tx, rx) = async_channel::bounded(1);

        // Send the command to get all tools
        self.node_commands_sender
            .send(NodeCommand::V2ApiListAllMcpShinkaiTools {
                category: None,
                res: tx,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send list tools command: {:?}", e))?;

        // Wait for the response
        let tools_json_value = rx.recv().await
            .map_err(|e| anyhow::anyhow!("Failed to receive tools response: {:?}", e))?
            .map_err(|e| anyhow::anyhow!("Failed to get tools: {:?}", e))?;

        // Prepare lists/maps to be populated
        let mut mcp_tools_list = Vec::new();
        let mut name_to_key_temp_map = HashMap::new(); // Temporary map to build

        // Iterate through the received tools JSON array
        if let Some(tools_array) = tools_json_value.as_array() {
            for tool_json in tools_array {
                // Extract required fields (handle potential missing fields gracefully)
                let name_opt = tool_json.get("name").and_then(Value::as_str).map(String::from);
                let key_opt = tool_json.get("tool_router_key").and_then(Value::as_str).map(String::from); // <<< Extract the key
                let description_opt = tool_json.get("description").and_then(Value::as_str).map(String::from);
                let input_schema_val_opt = tool_json.get("input_args").cloned();

                // Only proceed if we have all necessary parts
                if let (Some(name), Some(key), Some(description), Some(input_schema_val)) = (name_opt, key_opt, description_opt, input_schema_val_opt) {
                    // Convert schema to the map expected by rmcp::model::Tool
                    if let Value::Object(schema_map) = input_schema_val {
                        // Create the rmcp::model::Tool for the cache
                        let mcp_tool = Tool {
                            name: Cow::Owned(name.clone()), // Clone name for the Tool struct
                            description: Cow::Owned(description),
                            input_schema: Arc::new(schema_map),
                        };
                        mcp_tools_list.push(mcp_tool);

                        // Add entry to the temporary name->key map
                        name_to_key_temp_map.insert(name, key); // Move name into the map key

                    } else {
                        tracing::warn!("Skipping tool due to invalid input_args schema: {:?}", tool_json.get("name"));
                    }
                } else {
                    tracing::warn!("Skipping tool due to missing fields (name, tool_router_key, description, or input_args): {:?}", tool_json);
                }
            }
        } else {
            return Err(anyhow::anyhow!("Tool list response was not a JSON array"));
        }

        // --- Update the global statics ---
        let tools_count = mcp_tools_list.len();
        let map_count = name_to_key_temp_map.len();

        // Update the TOOLS_CACHE
        match TOOLS_CACHE.write() {
            Ok(mut cache_guard) => *cache_guard = mcp_tools_list,
            Err(e) => return Err(anyhow::anyhow!("Failed to acquire write lock for TOOLS_CACHE: {:?}", e)),
        }
        tracing::info!("Updated tools cache with {} tools", tools_count);

        // Update the TOOL_NAME_TO_KEY_MAP
        match TOOL_NAME_TO_KEY_MAP.write() {
             Ok(mut map_guard) => *map_guard = name_to_key_temp_map,
             Err(e) => return Err(anyhow::anyhow!("Failed to acquire write lock for TOOL_NAME_TO_KEY_MAP: {:?}", e)),
        }
        tracing::info!("Updated tool name to key map with {} entries", map_count);

        Ok(())
    }

    // Helper function for executing tools - takes tool_router_key now
    async fn execute_shinkai_tool(&self, tool_router_key: String, params: Value) -> Result<String, String> {
        // Create a response channel
        let (tx, rx) = async_channel::bounded(1);

        // Convert params to Map if it's not already
        let parameters = match params {
            Value::Object(map) => map,
            _ => {
                let mut map = Map::new();
                map.insert("value".to_string(), params);
                map
            }
        };
        
        tracing::debug!(
            target: "mcp_tools_service",
            "[execute_tool] Sending NodeCommand with tool_router_key: '{}'",
            tool_router_key
        );

        // Send the command to execute the tool
        match self.node_commands_sender
            .send(NodeCommand::V2ApiExecuteMcpTool {
                tool_router_key, // Use the passed-in key
                parameters,
                tool_id: "".to_string(),
                app_id: "".to_string(),
                extra_config: Map::new(),
                mounts: None,
                res: tx,
            })
            .await {
                Ok(_) => (),
                Err(e) => return Err(format!("Failed to send execute tool command: {:?}", e)),
            };

        // Wait for the response
        match rx.recv().await {
            Ok(result) => match result {
                Ok(output) => {
                    tracing::debug!(target: "mcp_tools_service", "--- Tool execution result: {:?}", output);
                    Ok(output.to_string())
                }
                Err(e) => {
                    tracing::error!(target: "mcp_tools_service", "--- Tool execution error: {:?}", e);
                    Err(format!("Tool execution error: {:?}", e))
                }
            },
            Err(e) => Err(format!("Failed to receive tool response: {:?}", e)),
        }
    }
}

impl ServerHandler for McpToolsService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
            server_info: Implementation {
                name: "Shinkai MCP Server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some(format!("Shinkai Node {} command interface", self.node_name)),
        }
    }

    fn initialize(
        &self,
        param: InitializeRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, ErrorData>> + Send + '_ {
        tracing::info!("Handling initialize request with protocol version: {:?}", param.protocol_version);
        
        // Wrap existing logic in std::future::ready
        let result = InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
            server_info: Implementation {
                name: "Shinkai MCP Server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some(format!("Shinkai Node {} command interface", self.node_name)),
        };
        
        future::ready(Ok(result))
    }

    fn list_prompts(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + Send + '_ {
        // Use ErrorData and ListPromptsResult::default()
        future::ready(Ok(ListPromptsResult::default())) 
    }

    fn list_resources(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + Send + '_ {
         // Use ErrorData and ListResourcesResult::default()
        future::ready(Ok(ListResourcesResult::default()))
    }

    // Override the call_tool method
    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let tool_name = request.name.to_string(); // Get the requested tool name (e.g., "network__echo")
            tracing::debug!(target: "mcp_tools_service", "Handling call_tool request for name='{}'", tool_name);

            // Extract arguments directly for the target tool
            let arguments = request.arguments.ok_or_else(|| {
                tracing::warn!("Missing arguments for tool call: {}", tool_name);
                McpError::invalid_params(format!("Missing arguments object for tool '{}'", tool_name), None)
            })?;

            // --- Look up the tool_router_key from the map using the request.name ---
            let tool_router_key = {
                let map_guard = TOOL_NAME_TO_KEY_MAP.read()
                    .map_err(|_| McpError::internal_error("Failed to acquire read lock for TOOL_NAME_TO_KEY_MAP", None))?;
                map_guard.get(&tool_name).cloned()
            };

            match tool_router_key {
                Some(key) => {
                    // Found the key, proceed to execute directly
                    tracing::debug!(target: "mcp_tools_service", "Found tool_router_key '{}' for name '{}'", key, tool_name);
                    
                    // Convert arguments JsonObject into the Value expected by execute_shinkai_tool
                    let params_value = Value::Object(arguments); 
                    
                    match self.execute_shinkai_tool(key, params_value).await {
                        Ok(output_str) => {
                            tracing::debug!("call_tool: execution successful for '{}', result: {}", tool_name, output_str);
                            Ok(CallToolResult::success(vec![Content::text(output_str)]))
                        },
                        Err(err_str) => {
                            tracing::error!("call_tool: execution failed for '{}': {}", tool_name, err_str);
                            Err(McpError::internal_error(format!("Tool '{}' execution failed: {}", tool_name, err_str), None))
                        }
                    }
                }
                None => {
                    // Key not found for the given name
                    tracing::error!(target: "mcp_tools_service", "Could not find tool_router_key for tool name: {}", tool_name);
                    Err(McpError::invalid_params(format!("Tool '{}' not found or mapping missing", tool_name), None))
                }
            }
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        tracing::debug!("Handling list_tools request (run_tool definition removed)");
        let tools_from_cache = self.list_tools(); // Get dynamic tools only

        // Remove the manual addition of run_tool
        /*
        if let Value::Object(schema_map) = run_tool_schema {
             ...
            tools_from_cache.push(run_tool_def); 
            ...
        } else {
            ...
        }
        */
       
        let result = ListToolsResult {
            tools: tools_from_cache,
            next_cursor: None,
        };
        tracing::debug!("Responding to list_tools with {} tools", result.tools.len());
        future::ready(Ok(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Add tests as needed
} 