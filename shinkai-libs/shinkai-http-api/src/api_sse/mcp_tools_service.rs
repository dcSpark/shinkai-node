use async_channel::Sender;
use std::sync::{Arc, RwLock};
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
},
    service::RequestContext, tool,
    RoleServer,
    ServiceError as McpError,
};
use serde_json::{Value, Map};
use std::borrow::Cow;
use async_trait::async_trait;
use std::future::{self, Future};

#[derive(Clone)]
pub struct McpToolsService {
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
    tools_cache: Arc<RwLock<Vec<Tool>>>,
}

impl McpToolsService {
    pub fn new(node_commands_sender: Sender<NodeCommand>, node_name: String) -> Self {
        let service = Self {
            node_commands_sender,
            node_name,
            tools_cache: Arc::new(RwLock::new(Vec::new())),
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
        self.tools_cache.read()
            .expect("Failed to read tools cache")
            .clone()
    }

    /// Update the tools cache by fetching tools through the node commands
    pub async fn update_tools_cache(&self) -> anyhow::Result<()> {
        // Create a response channel
        let (tx, rx) = async_channel::bounded(1);

        // Send the command to get all tools
        self.node_commands_sender
            .send(NodeCommand::V2ApiListAllShinkaiTools {
                bearer: "debug".to_string(),
                category: None,
                res: tx,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send list tools command: {:?}", e))?;

        // Wait for the response
        let tools = rx.recv().await
            .map_err(|e| anyhow::anyhow!("Failed to receive tools response: {:?}", e))?
            .map_err(|e| anyhow::anyhow!("Failed to get tools: {:?}", e))?;

        // Convert the tools to MCP format
        let mcp_tools = tools.as_array()
            .ok_or_else(|| anyhow::anyhow!("Expected array of tools"))?
            .iter()
            .filter_map(|tool| {
                // Convert to new rmcp Tool format
                let name = tool.get("name")?.as_str()?.to_string();
                let description = tool.get("description")?.as_str()?.to_string();
                let input_schema = tool.get("input_args")?.clone();
                
                // Convert to proper types for RMCP Tool
                let schema_map = match input_schema {
                    Value::Object(map) => map,
                    _ => return None, // Skip tools with invalid schema
                };
                
                Some(Tool {
                    name: Cow::Owned(name),
                    description: Cow::Owned(description),
                    input_schema: Arc::new(schema_map),
                })
            })
            .collect::<Vec<_>>();

        // Update the cache
        *self.tools_cache.write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {:?}", e))? = mcp_tools.clone();
        tracing::info!("Updated tools cache with {} tools", mcp_tools.len());
        Ok(())
    }

    // Helper function for executing tools
    async fn execute_shinkai_tool(&self, tool_name: String, params: Value) -> Result<String, String> {
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

        // Send the command to execute the tool
        match self.node_commands_sender
            .send(NodeCommand::V2ApiExecuteTool {
                bearer: "debug".to_string(),
                tool_router_key: tool_name,
                parameters,
                tool_id: "".to_string(),
                app_id: "".to_string(),
                llm_provider: "".to_string(),
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
                Ok(output) => Ok(output.to_string()),
                Err(e) => Err(format!("Tool execution error: {:?}", e)),
            },
            Err(e) => Err(format!("Failed to receive tool response: {:?}", e)),
        }
    }
}

#[tool(tool_box)]
impl McpToolsService {
    // Define your tool using the tool macro
    #[tool(description = "Execute a Shinkai node tool")]
    async fn run_tool(
        &self, 
        #[tool(param)] tool_name: String, 
        #[tool(param)] params: Value
    ) -> Result<String, String> {
        self.execute_shinkai_tool(tool_name, params).await
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

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        tracing::debug!("Handling list_tools request");
        let tools_from_cache = self.list_tools();
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