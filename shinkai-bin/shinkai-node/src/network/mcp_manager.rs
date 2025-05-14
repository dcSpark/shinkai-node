// stateless_mcp_client.rs — SPAWN → LIST TOOLS → SHUT DOWN
// -----------------------------------------------------------------------------
// A minimal helper for *stateless* interaction with an MCP server launched as a
// subprocess.  It spawns the command, performs the JSON‑RPC handshake via the
// Model Context Protocol Rust SDK, requests the available tools, returns them
// as a `Vec<Tool>`, and then cleanly terminates the child process.
// -----------------------------------------------------------------------------

use anyhow::Result;
use rmcp::{model::Tool, transport::TokioChildProcess, ServiceExt};
use tokio::process::Command;

/// Spawn a command that runs an MCP server, list its tools, return them, then
/// shut the server down.
///
/// * `cmd_str` – shell command that launches the MCP server (e.g.
///   `"npx -y @modelcontextprotocol/server-everything"`).
///
/// ## Example
/// ```rust,ignore
/// let tools = list_tools_via_command("npx -y @modelcontextprotocol/server-everything")
///     .await?;
/// for t in tools {
///     println!("{} — {}", t.name, t.description.unwrap_or_default());
/// }
/// ```
pub async fn list_tools_via_command(cmd_str: &str) -> Result<Vec<Tool>> {
    // 1. Build the child process (via shell so we support complex commands)
    // Parse the command string to handle environment variables and the executable
    let mut env_vars = std::collections::HashMap::new();
    let mut cmd_parts = cmd_str.trim().split_whitespace();
    
    // Extract environment variables (KEY=VALUE format before npx/uvx)
    let mut cmd_executable = "";
    let mut cmd_args = Vec::new();
    
    // Process each part of the command
    while let Some(part) = cmd_parts.next() {
        if part == "npx" || part == "uvx" {
            // Found the executable
            cmd_executable = part;
            // Collect the remaining parts as arguments
            cmd_args.extend(cmd_parts);
            break;
        } else if part.contains('=') {
            // This is an environment variable
            let mut kv_iter = part.splitn(2, '=');
            if let (Some(key), Some(value)) = (kv_iter.next(), kv_iter.next()) {
                env_vars.insert(key.to_string(), value.to_string());
            }
        } else {
            // If we get here, we've found the executable but it's not npx/uvx
            cmd_executable = part;
            // Collect the remaining parts as arguments
            cmd_args.extend(cmd_parts);
            break;
        }
    }
    
    // Create the command with the executable
    let mut cmd = Command::new(cmd_executable);
    
    // Add all arguments
    for arg in cmd_args {
        cmd.arg(arg);
    }
    
    // Set environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }
    let service = ()
        .serve(TokioChildProcess::new(&mut cmd)?)
        .await?;
    // 2. Initialize the MCP server
    service.peer_info();

    // 3. Call the standard MCP `list_tools` method 
    let tools = service.list_all_tools().await?;

    // 4. Gracefully shut down the service (drops stdio, child should exit)
    service.cancel().await?;

    Ok(tools)
}
