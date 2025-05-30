use log::info;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use serde_yaml::Value as YamlValue;
use std::collections::HashSet;

/// GitHub repository information
pub struct GitHubRepo {
    pub owner: String,
    pub repo: String,
    pub url: String,
}

/// Parse a GitHub URL to extract owner and repository name
pub fn parse_github_url(url: &str) -> Result<GitHubRepo, String> {
    // Handle different GitHub URL formats
    let url = url.trim_end_matches('/');

    // Extract owner and repo from URL
    if let Some(github_path) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = github_path.split('/').collect();
        if parts.len() >= 2 {
            return Ok(GitHubRepo {
                owner: parts[0].to_string(),
                repo: parts[1].to_string(),
                url: url.to_string(),
            });
        }
    }

    Err(format!("Invalid GitHub URL: {}", url))
}

/// Fetch a file from a GitHub repository
pub async fn fetch_github_file(client: &Client, owner: &str, repo: &str, path: &str) -> Result<String, String> {
    let url = format!("https://raw.githubusercontent.com/{}/{}/main/{}", owner, repo, path);

    info!("Fetching file from GitHub: {}", url);

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.text().await {
                    Ok(content) => Ok(content),
                    Err(e) => Err(format!("Failed to read response content: {}", e)),
                }
            } else {
                // Try with master branch if main fails
                let master_url = format!("https://raw.githubusercontent.com/{}/{}/master/{}", owner, repo, path);

                match client.get(&master_url).send().await {
                    Ok(master_response) => {
                        if master_response.status().is_success() {
                            match master_response.text().await {
                                Ok(content) => Ok(content),
                                Err(e) => Err(format!("Failed to read response content: {}", e)),
                            }
                        } else {
                            Err(format!("Failed to fetch file: HTTP {}", master_response.status()))
                        }
                    }
                    Err(e) => Err(format!("Failed to fetch file: {}", e)),
                }
            }
        }
        Err(e) => Err(format!("Failed to fetch file: {}", e)),
    }
}

/// Extract environment variables from README.md content
pub fn extract_mcp_env_vars_from_readme(readme_content: &str) -> HashSet<String> {
    let mut env_vars = HashSet::new();

    // Common patterns for environment variables in READMEs
    let patterns = vec![
        // Match export statements: export VAR_NAME="value"
        r"export\s+([A-Z][A-Z0-9_]+)=",
        // Match env vars in JSON configuration
        r#""([A-Z][A-Z0-9_]+)":\s*"[^"]*""#,
        // Match env vars in environment sections with clear markers
        r#"env.*?["']([A-Z][A-Z0-9_]+)["']"#,
        // Match env vars in markdown code blocks
        r"`([A-Z][A-Z0-9_]+)`",
        // Match env vars in angle brackets
        r"<([A-Z][A-Z0-9_]+)>",
    ];

    // Apply each pattern to the README content
    for pattern in patterns {
        if let Ok(regex) = Regex::new(pattern) {
            for cap in regex.captures_iter(readme_content) {
                if cap.len() > 1 {
                    if let Some(var_name) = cap.get(1) {
                        let name = var_name.as_str().to_string();
                        // Filter out common false positives
                        if !name.contains("HTTP")
                            && !name.contains("JSON")
                            && !name.contains("API")
                            && !name.contains("URL")
                            && !name.contains("HTML")
                            && !name.contains("CSS")
                            && !name.contains("README")
                            && !name.contains("TODO")
                        {
                            env_vars.insert(name);
                        }
                    }
                }
            }
        }
    }

    // Additional specific patterns for common env vars in MCP servers
    let specific_patterns = vec![
        "API_KEY",
        "TOKEN",
        "SECRET",
        "CREDENTIAL",
        "AUTH",
        "PASSWORD",
        "KEY",
        "BUNDLE",
        "ACCESS",
        "APIKEY",
    ];

    // Look for common environment variable names directly
    for pattern in specific_patterns {
        for line in readme_content.lines() {
            if line.contains(pattern) {
                // Extract words that look like environment variables
                for word in line.split_whitespace() {
                    let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    if word
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                        && word.len() > 2
                        && word.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    {
                        env_vars.insert(word.to_string());
                    }
                }
            }
        }
    }

    env_vars
}

/// Extract required environment variable names from a smithery.yaml file
pub fn extract_env_vars_from_smithery_yaml(yaml_content: &str) -> HashSet<String> {
    let mut env_vars = HashSet::new();

    if let Ok(yaml) = serde_yaml::from_str::<YamlValue>(yaml_content) {
        // Navigate to startCommand.configSchema
        if let Some(start_cmd) = yaml.get("startCommand") {
            if let Some(config_schema) = start_cmd.get("configSchema") {
                // First, try to gather required fields
                if let Some(required) = config_schema.get("required") {
                    if let Some(seq) = required.as_sequence() {
                        for item in seq {
                            if let Some(var) = item.as_str() {
                                env_vars.insert(var.to_string());
                            }
                        }
                    }
                }

                // Fallback to all property names if no required fields found
                if env_vars.is_empty() {
                    if let Some(props) = config_schema.get("properties") {
                        if let Some(map) = props.as_mapping() {
                            for (k, _) in map {
                                if let Some(key) = k.as_str() {
                                    env_vars.insert(key.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    env_vars
}

/// Extract MCP server command from JSON configuration blocks in README content
pub fn extract_command_from_json_config_in_readme(readme_content: &str) -> Option<String> {
    // Regex to find JSON code blocks
    let json_block_regex = Regex::new(r"```json\s*\n([\s\S]*?)\n```").ok()?;

    for capture in json_block_regex.captures_iter(readme_content) {
        if let Some(json_str) = capture.get(1) {
            // Try to parse the JSON
            if let Ok(json_value) = serde_json::from_str::<Value>(json_str.as_str()) {
                // Check if it has mcpServers key
                if let Some(mcp_servers) = json_value.get("mcpServers") {
                    if let Some(servers_obj) = mcp_servers.as_object() {
                        // Get the first server entry
                        if let Some((_, server_config)) = servers_obj.iter().next() {
                            // Extract command and args
                            let command = server_config.get("command").and_then(|v| v.as_str()).unwrap_or("");

                            let args = server_config
                                .get("args")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<&str>>().join(" "))
                                .unwrap_or_default();

                            // Build the complete command string
                            let full_command = if args.is_empty() {
                                command.to_string()
                            } else {
                                format!("{} {}", command, args)
                            };

                            if !full_command.trim().is_empty() {
                                return Some(full_command);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_env_vars_from_smithery_yaml() {
        let yaml = r#"
version: 1
startCommand:
  type: http
  configSchema:
    type: object
    required: ["API_KEY", "USER_ID"]
    properties:
      API_KEY:
        type: string
      USER_ID:
        type: string
      OPTIONAL:
        type: string
"#;

        let vars = extract_env_vars_from_smithery_yaml(yaml);
        assert!(vars.contains("API_KEY"));
        assert!(vars.contains("USER_ID"));
        assert!(!vars.contains("OPTIONAL"));
    }

    #[test]
    fn test_extract_command_from_json_config_in_readme() {
        let readme = r#"

        # DuckDuckGo Search MCP Server

[![smithery badge](https://smithery.ai/badge/@nickclyde/duckduckgo-mcp-server)](https://smithery.ai/server/@nickclyde/duckduckgo-mcp-server)

A Model Context Protocol (MCP) server that provides web search capabilities through DuckDuckGo, with additional features for content fetching and parsing.

<a href="https://glama.ai/mcp/servers/phcus2gcpn">
  <img width="380" height="200" src="https://glama.ai/mcp/servers/phcus2gcpn/badge" alt="DuckDuckGo Server MCP server" />
</a>

## Features

- **Web Search**: Search DuckDuckGo with advanced rate limiting and result formatting
- **Content Fetching**: Retrieve and parse webpage content with intelligent text extraction
- **Rate Limiting**: Built-in protection against rate limits for both search and content fetching
- **Error Handling**: Comprehensive error handling and logging
- **LLM-Friendly Output**: Results formatted specifically for large language model consumption

## Installation

### Installing via Smithery

To install DuckDuckGo Search Server for Claude Desktop automatically via [Smithery](https://smithery.ai/server/@nickclyde/duckduckgo-mcp-server):

```bash
npx -y @smithery/cli install @nickclyde/duckduckgo-mcp-server --client claude
```

### Installing via `uv`

Install directly from PyPI using `uv`:

```bash
uv pip install duckduckgo-mcp-server
```

## Usage

### Running with Claude Desktop

1. Download [Claude Desktop](https://claude.ai/download)
2. Create or edit your Claude Desktop configuration:
   - On macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - On Windows: `%APPDATA%\Claude\claude_desktop_config.json`

Add the following configuration:

```json
{
    "mcpServers": {
        "ddg-search": {
            "command": "uvx",
            "args": ["duckduckgo-mcp-server"]
        }
    }
}
```

3. Restart Claude Desktop

### Development

For local development, you can use the MCP CLI:

```bash
# Run with the MCP Inspector
mcp dev server.py

# Install locally for testing with Claude Desktop
mcp install server.py
```
## Available Tools

### 1. Search Tool

```python
async def search(query: str, max_results: int = 10) -> str
```

Performs a web search on DuckDuckGo and returns formatted results.

**Parameters:**
- `query`: Search query string
- `max_results`: Maximum number of results to return (default: 10)

**Returns:**
Formatted string containing search results with titles, URLs, and snippets.

### 2. Content Fetching Tool

```python
async def fetch_content(url: str) -> str
```

Fetches and parses content from a webpage.

**Parameters:**
- `url`: The webpage URL to fetch content from

**Returns:**
Cleaned and formatted text content from the webpage.

## Features in Detail

### Rate Limiting

- Search: Limited to 30 requests per minute
- Content Fetching: Limited to 20 requests per minute
- Automatic queue management and wait times

### Result Processing

- Removes ads and irrelevant content
- Cleans up DuckDuckGo redirect URLs
- Formats results for optimal LLM consumption
- Truncates long content appropriately

### Error Handling

- Comprehensive error catching and reporting
- Detailed logging through MCP context
- Graceful degradation on rate limits or timeouts

## Contributing

Issues and pull requests are welcome! Some areas for potential improvement:

- Additional search parameters (region, language, etc.)
- Enhanced content parsing options
- Caching layer for frequently accessed content
- Additional rate limiting strategies

## License

This project is licensed under the MIT License.
}
"#;

        let command = extract_command_from_json_config_in_readme(readme);
        assert_eq!(command, Some("uvx duckduckgo-mcp-server".to_string()));
    }
}
