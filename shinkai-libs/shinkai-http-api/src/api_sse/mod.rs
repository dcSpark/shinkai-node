//! Model Context Protocol (MCP) Server-Sent Events (SSE) implementation.
//!
//! This module provides a Warp-based implementation of the MCP protocol using SSE.

mod api_sse_handlers;
pub mod api_sse_routes;
mod mcp_tools_service;

// Re-export the public components
pub use api_sse_routes::{mcp_sse_routes, SessionQuery};

// Re-export the state for custom integrations
pub use api_sse_handlers::McpState; 