//! Model Context Protocol (MCP) Server-Sent Events (SSE) implementation.
//!
//! This module provides a Warp-based implementation of the MCP protocol using SSE.

mod api_sse_handlers;
mod mcp_router;
pub mod api_sse_routes;

// Re-export the public components
pub use mcp_router::{McpRouter, SharedMcpRouter};
pub use api_sse_routes::{mcp_sse_routes, SessionQuery};

// Re-export the state for custom integrations
pub use api_sse_handlers::McpState; 