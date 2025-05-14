These SSE routes are used to run Shinkai tools on an MCP client through the MCP
SSE protocol

## Connection

Connecting from clients that support SSE is straightforward, just input the URL
like this

http://localhost:9950/mcp/sse

Change the port or the base URL to match where your Shinkai node is running. You
can check the location in Shinkai Desktop by navigating to **Settings > Node
Address**.

For clients that do not support SSE, a gateway using STDIO is needed. We
recommend using `supergateway`. In the case of Claude Desktop, the configuration
for MCP is as follows

```json
{
    "mcpServers": {
        "shinkai-mcp-server": {
            "command": "npx",
            "args": [
                "-y",
                "supergateway",
                "--sse",
                "http://localhost:9950/mcp/sse"
            ]
        }
    }
}
```

## Enabling Tools

Tools intended for use with MCP must be marked as `mcp_enabled`. While there is
an API endpoint for this, the short-term goal is to manage this setting within
Shinkai Desktop.

To mark a tool as `mcp_enabled`, the tool itself must first be enabled in
Shinkai. If a tool is disabled, it also sets is `mcp_enabled` flag as `false`.

The `curl` command to enable a tool for MCP is as follows:

```sh
curl --location 'http://localhost:9950/v2/set_tool_mcp_enabled' \
--header 'Authorization: Bearer $TOKEN' \
--header 'Content-Type: application/json' \
--data '{
    "tool_router_key": "local:::__official_shinkai:::shinkai_llm_prompt_processor",
    "mcp_enabled": true
}'
```

Only tools marked as `mcp_enabled` can be listed and executed via MCP.
Attempting to execute a tool not marked as `mcp_enabled` will result in an
error.
