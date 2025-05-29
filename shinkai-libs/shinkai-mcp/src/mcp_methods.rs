use crate::{command::CommandWrappedInShellBuilder, utils::disect_command};
use anyhow::Result;
use rmcp::{
    model::{CallToolRequestParam, CallToolResult, ClientCapabilities, ClientInfo, Implementation, Tool}, transport::{SseTransport, TokioChildProcess}, ServiceExt
};
use std::collections::HashMap;
use tokio::process::Command;

pub async fn list_tools_via_command(cmd_str: &str, config: Option<HashMap<String, String>>) -> Result<Vec<Tool>> {
    let (env_vars, cmd_executable, cmd_args) = disect_command(cmd_str.to_string());
    let (adapted_program, adapted_args, adapted_envs) =
        CommandWrappedInShellBuilder::wrap_in_shell_as_values(cmd_executable, Some(cmd_args), Some(env_vars));
    let mut cmd = Command::new(adapted_program);
    cmd.kill_on_drop(true);
    cmd.envs(adapted_envs);
    cmd.envs(config.unwrap_or_default());
    cmd.args(adapted_args);

    // Retain the TokioChildProcess so we can wait on it after cancellation
    let mut child_process = TokioChildProcess::new(&mut cmd)?;
    let service = ().serve(&mut child_process).await?;
    // 2. Initialize the MCP server
    service.peer_info();

    // 3. Call the standard MCP `list_tools` method
    let tools = service
        .list_all_tools()
        .await
        .inspect_err(|e| log::error!("error listing tools: {:?}", e));

    // 4. Gracefully shut down the service (drops stdio, child should exit)
    let _ = service
        .cancel()
        .await
        .inspect_err(|e| log::error!("error cancelling sse service: {:?}", e));

    // 5. Wait for the child process to exit to avoid orphaning
    let _ = child_process.wait().await;

    Ok(tools.unwrap())
}

pub async fn list_tools_via_sse(sse_url: &str, _config: Option<HashMap<String, String>>) -> Result<Vec<Tool>> {
    // TODO: The config parameter is not currently used by SseTransport or ClientInfo setup in the example.
    // It might be used in the future for authentication headers or other SSE-specific configurations.
    let transport = SseTransport::start(sse_url).await?;
    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "shinkai_node_sse_client".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };
    let client = client_info
        .serve(transport)
        .await
        .map_err(|e| anyhow::anyhow!("SSE client connection error: {:?}", e))?;

    // Initialize and log server info (optional, but good for debugging)
    let _ = client.peer_info();

    // List tools
    let tools_result = client
        .list_all_tools()
        .await
        .inspect_err(|e| log::error!("error listing tools: {:?}", e));

    // Gracefully shut down the client
    let _ = client
        .cancel()
        .await
        .inspect_err(|e| log::error!("error cancelling sse service: {:?}", e));

    Ok(tools_result.unwrap())
}

pub async fn run_tool_via_command(
    command: String,
    tool: String,
    env_vars: HashMap<String, String>,
    parameters: serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<CallToolResult> {
    let (_, cmd_executable, cmd_args) = disect_command(command);

    println!("cmd_executable: {}", cmd_executable);
    println!("env_vars: {:?}", env_vars);
    println!("cmd_args: {:?}", cmd_args);

    // Use the wrap_in_shell_as_values function to prepare the command
    let (adapted_program, adapted_args, adapted_envs) =
        CommandWrappedInShellBuilder::wrap_in_shell_as_values(cmd_executable, Some(cmd_args), Some(env_vars.clone()));

    let mut cmd = Command::new(adapted_program);
    cmd.kill_on_drop(true);
    cmd.envs(adapted_envs);
    cmd.envs(env_vars);
    cmd.args(adapted_args);

    let service = ().serve(TokioChildProcess::new(&mut cmd)?).await?;
    service.peer_info();

    let call_tool_result = service
        .call_tool(CallToolRequestParam {
            name: tool.into(),
            arguments: Some(parameters),
        })
        .await;

    let _ = service
        .cancel()
        .await
        .inspect_err(|e| log::error!("error cancelling stdio service: {:?}", e));
    Ok(call_tool_result?)
}

pub async fn run_tool_via_sse(
    url: String,
    tool: String,
    parameters: serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<CallToolResult> {
    let transport = SseTransport::start(url)
        .await
        .inspect_err(|e| log::error!("error starting sse transport: {:?}", e))?;

    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "Shinkai Node Client".to_string(),
            version: "0.0.1".to_string(),
        },
    };
    let client = client_info.serve(transport).await.inspect_err(|e| {
        log::error!("client error: {:?}", e);
    })?;

    // Initialize
    let server_info = client.peer_info();
    log::info!("connected to server: {server_info:#?}");

    let call_tool_result = client
        .call_tool(CallToolRequestParam {
            name: tool.into(),
            arguments: Some(parameters),
        })
        .await
        .inspect_err(|e| log::error!("error calling tool: {:?}", e));
    let _ = client
        .cancel()
        .await
        .inspect_err(|e| log::error!("error cancelling sse service: {:?}", e));
    Ok(call_tool_result?)
}

#[cfg(test)]
pub mod tests_mcp_manager {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_run_tool_via_command() {
        let params = json!({
            "a": 1,
            "b": 2,
        });
        let params_map = params.as_object().unwrap().clone();

        let result = run_tool_via_command(
            "npx -y @modelcontextprotocol/server-everything".to_string(),
            "add".to_string(),
            HashMap::new(),
            params_map,
        )
        .await
        .inspect_err(|e| {
            println!("error {:?}", e);
        });

        assert!(result.is_ok());
        let unwrapped = result.unwrap();
        assert_eq!(unwrapped.content.len(), 1);
        assert!(unwrapped.content[0].as_text().unwrap().text.contains("3"));
    }

    #[tokio::test]
    async fn test_run_tool_via_sse() {
        let mut envs = HashMap::new();
        envs.insert("PORT".to_string(), "8000".to_string());
        let (adapted_program, adapted_args, adapted_envs) = CommandWrappedInShellBuilder::wrap_in_shell_as_values(
            "npx".to_string(),
            Some(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-everything".to_string(),
                "sse".to_string(),
            ]) as Option<Vec<String>>,
            Some(envs),
        );

        let _child_result = Command::new(adapted_program)
            .args(adapted_args)
            .envs(adapted_envs)
            .kill_on_drop(true)
            .spawn()
            .inspect_err(|e| {
                println!("error {:?}", e);
            });
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let params = json!({
            "a": 1,
            "b": 2,
        });
        let params_map = params.as_object().unwrap().clone();

        let result = run_tool_via_sse("http://localhost:8000/sse".to_string(), "add".to_string(), params_map)
            .await
            .inspect_err(|e| {
                println!("error {:?}", e);
            });
        match result {
            Ok(result) => {
                assert!(result.content.len() == 1);
                assert!(result.content[0].as_text().unwrap().text.contains("3"));
            }
            Err(e) => {
                println!("error {:?}", e);
                assert!(false);
            }
        }
    }

    #[tokio::test]
    async fn test_list_tools_via_command() {
        let result = list_tools_via_command("npx -y @modelcontextprotocol/server-everything", None).await;
        assert!(result.is_ok());
        let unwrapped = result.unwrap();
        assert!(unwrapped.len() == 8);
        let tools = [
            "echo",
            "add",
            "longRunningOperation",
            "sampleLLM",
            "getTinyImage",
            "printEnv",
            "annotatedMessage",
            "getResourceReference",
        ];
        for tool in tools {
            assert!(unwrapped.iter().any(|t| t.name == tool));
        }
    }

    #[tokio::test]
    async fn test_list_tools_via_sse() {
        let mut envs = HashMap::new();
        envs.insert("PORT".to_string(), "8001".to_string());
        let (adapted_program, adapted_args, adapted_envs) = CommandWrappedInShellBuilder::wrap_in_shell_as_values(
            "npx".to_string(),
            Some(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-everything".to_string(),
                "sse".to_string(),
            ]) as Option<Vec<String>>,
            Some(envs),
        );

        let _child_result = Command::new(adapted_program)
            .args(adapted_args)
            .envs(adapted_envs)
            .kill_on_drop(true)
            .spawn()
            .inspect_err(|e| {
                println!("error {:?}", e);
            });

        // Wait for server to be ready
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let result = list_tools_via_sse("http://localhost:8001/sse", None)
            .await
            .inspect_err(|e| {
                println!("error {:?}", e);
            });
        assert!(result.is_ok());
        let unwrapped = result.unwrap();
        assert!(unwrapped.len() == 8);
        let tools = [
            "echo",
            "add",
            "longRunningOperation",
            "sampleLLM",
            "getTinyImage",
            "printEnv",
            "annotatedMessage",
            "getResourceReference",
        ];
        for tool in tools {
            assert!(unwrapped.iter().any(|t| t.name == tool));
        }
    }
    /* TODO: Uncomment these tests when we have a way to test them, right now the credentials expire so it does not work consistently
    #[tokio::test]
    async fn test_list_tools_composio_github() {
        let result = list_tools_via_sse("https://mcp.composio.dev/partner/composio/github?customerId=51fcb8d4-16c2-4e33-8a4d-898e54e68fb6&agent=cursor", None).await;
        assert!(result.is_ok());
        let unwrapped = result.unwrap();
        println!("tools: {:?}", unwrapped);
        assert!(unwrapped.len() > 1);
    }

    #[tokio::test]
    async fn test_list_tools_composio_gmail() {
        let result = list_tools_via_sse(
            "https://mcp.composio.dev/partner/composio/gmail?customerId=future-gorgeous-girl-OMlvSA&agent=cursor",
            None,
        )
        .await
        .inspect_err(|e| {
            println!("error {:?}", e);
        });
        assert!(result.is_ok());
        let unwrapped = result.unwrap();
        println!("tools: {:?}", unwrapped);
        assert!(unwrapped.len() > 1);
    }
    */
}
