use mcp_sdk_server::Router;
use mcp_sdk_core::{
    protocol::{JsonRpcRequest, JsonRpcResponse, ServerCapabilities, ToolsCapability, PromptsCapability, ResourcesCapability},
    Content, Resource, ToolError, handler::{ResourceError, PromptError}, TextContent,
    prompt::Prompt,
};
use serde_json::{json, Value};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use anyhow::Result;

/// A general-purpose router for MCP requests
#[derive(Clone)]
pub struct McpRouter {
    // Add any router state here
}

impl McpRouter {
    /// Create a new instance of the MCP router
    pub fn new() -> Self {
        Self {}
    }
}

impl Router for McpRouter {
    fn name(&self) -> String {
        "Shinkai MCP Router".to_string()
    }

    fn instructions(&self) -> String {
        "A general purpose router for MCP requests".to_string()
    }

    fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            tools: Some(ToolsCapability { list_changed: Some(true) }),
            resources: Some(ResourcesCapability { 
                list_changed: Some(false),
                subscribe: None
            }),
            prompts: Some(PromptsCapability { list_changed: Some(false) }),
        }
    }

    fn list_tools(&self) -> Vec<mcp_sdk_core::Tool> {
        vec![mcp_sdk_core::Tool {
            name: "text_completion".to_string(),
            description: "Generate text completions based on provided input".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The input text to generate completion for"
                    },
                    "max_tokens": {
                        "type": "integer",
                        "description": "Maximum number of tokens to generate",
                        "default": 100
                    },
                    "temperature": {
                        "type": "number",
                        "description": "Sampling temperature",
                        "minimum": 0.0,
                        "maximum": 2.0,
                        "default": 1.0
                    }
                },
                "required": ["prompt"]
            }),
        }]
    }

    fn call_tool(&self, _tool_name: &str, _params: Value) -> Pin<Box<dyn Future<Output = Result<Vec<Content>, ToolError>> + Send + 'static>> {
        Box::pin(async move {
            let text_content = TextContent {
                text: "This is a sample response".to_string(),
                annotations: None,
            };
            Ok(vec![Content::Text(text_content)])
        })
    }

    fn list_resources(&self) -> Vec<Resource> {
        vec![]
    }

    fn read_resource(&self, _resource_id: &str) -> Pin<Box<dyn Future<Output = Result<String, ResourceError>> + Send + 'static>> {
        Box::pin(async move {
            Err(ResourceError::NotFound("Resource not found".to_string()))
        })
    }

    fn list_prompts(&self) -> Vec<Prompt> {
        vec![]
    }

    fn get_prompt(&self, _prompt_id: &str) -> Pin<Box<dyn Future<Output = Result<String, PromptError>> + Send + 'static>> {
        Box::pin(async move {
            Err(PromptError::NotFound("Prompt not found".to_string()))
        })
    }
}

/// Wrapper struct to implement the Service trait for the router
#[derive(Clone)]
pub struct RouterService(pub McpRouter);

/// Implement the Service trait for the RouterService
impl hyper::service::Service<JsonRpcRequest> for RouterService {
    type Response = JsonRpcResponse;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: JsonRpcRequest) -> Self::Future {
        let router = self.0.clone();
        Box::pin(async move {
            let response = match request.method.as_str() {
                "initialize" => {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(json!({
                            "protocolVersion": "2024-11-05",
                            "capabilities": {
                                "logging": {},
                                "prompts": {
                                    "listChanged": false
                                },
                                "resources": {
                                    "listChanged": false
                                },
                                "tools": {
                                    "listChanged": true
                                }
                            },
                            "serverInfo": {
                                "name": "Shinkai MCP Service",
                                "version": "1.0.0"
                            }
                        })),
                        error: None,
                    }
                }
                "tools/list" => {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(json!({
                            "tools": router.list_tools().iter().map(|tool| {
                                json!({
                                    "name": tool.name,
                                    "description": tool.description,
                                    "inputSchema": tool.input_schema
                                })
                            }).collect::<Vec<_>>()
                        })),
                        error: None,
                    }
                }
                "resources/list" => {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(json!({ "resources": [] })),
                        error: None,
                    }
                }
                _ => {
                    // Echo the request for unhandled methods
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(json!({
                            "echo": request.params,
                            "method": request.method,
                            "message": "Method not implemented"
                        })),
                        error: None,
                    }
                }
            };

            Ok(response)
        })
    }
}

pub type SharedMcpRouter = McpRouter; 