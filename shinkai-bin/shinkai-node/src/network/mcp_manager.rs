use anyhow::Result;
use rmcp::{model::Tool, transport::TokioChildProcess, ServiceExt};
use tokio::process::Command;
use shinkai_message_primitives::schemas::mcp_server::MCPServerConfig;

pub async fn list_tools_via_command(cmd_str: &str, config: Option<MCPServerConfig>) -> Result<Vec<Tool>> {
    // 1. Build the child process (via shell so we support complex commands)
    // Parse the command string for the executable and arguments
    let mut cmd_parts_iter = cmd_str.trim().split_whitespace();
    let cmd_executable = match cmd_parts_iter.next() {
        Some(exe) => exe,
        None => return Err(anyhow::anyhow!("Command string cannot be empty and must specify an executable.")),
    };
    let cmd_args: Vec<&str> = cmd_parts_iter.collect();
    
    // Create the command with the executable
    let mut cmd = Command::new(cmd_executable);
    
    // Add all arguments
    for arg in cmd_args {
        cmd.arg(arg);
    }
    
    // Set environment variables from config if provided
    if let Some(env_map) = &config { // config is Option<HashMap<String, String>>, so env_map is &HashMap<String, String>
        for (key, value) in env_map { // Iterate directly over the HashMap
            cmd.env(key, value);
        }
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
