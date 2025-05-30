use log::info;
use regex::Regex;
use reqwest::Client;
use std::collections::HashSet;
use serde_yaml::Value as YamlValue;

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
pub async fn fetch_github_file(
    client: &Client,
    owner: &str,
    repo: &str,
    path: &str,
) -> Result<String, String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/main/{}",
        owner, repo, path
    );

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
                let master_url = format!(
                    "https://raw.githubusercontent.com/{}/{}/master/{}",
                    owner, repo, path
                );

                match client.get(&master_url).send().await {
                    Ok(master_response) => {
                        if master_response.status().is_success() {
                            match master_response.text().await {
                                Ok(content) => Ok(content),
                                Err(e) => Err(format!("Failed to read response content: {}", e)),
                            }
                        } else {
                            Err(format!(
                                "Failed to fetch file: HTTP {}",
                                master_response.status()
                            ))
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

/// Extract the start command from a smithery.yaml file if available
pub fn extract_start_command_from_smithery_yaml(yaml_content: &str) -> Option<String> {
    if let Ok(yaml) = serde_yaml::from_str::<YamlValue>(yaml_content) {
        if let Some(start_cmd) = yaml.get("startCommand") {
            if let Some(cmd_fn) = start_cmd.get("commandFunction") {
                if let Some(fn_str) = cmd_fn.as_str() {
                    let re = Regex::new(
                        r"command:\s*['\"](?P<cmd>[^'\"]+)['\"].*?args:\s*\[(?P<args>[^\]]*)\]",
                    )
                    .ok()?;

                    if let Some(caps) = re.captures(fn_str) {
                        let cmd = caps.name("cmd")?.as_str();
                        let args_str = caps.name("args").map(|m| m.as_str()).unwrap_or("");
                        let args: Vec<String> = args_str
                            .split(',')
                            .map(|a| a.trim().trim_matches('"').trim_matches('\''))
                            .filter(|a| !a.is_empty())
                            .map(|s| s.to_string())
                            .collect();
                        let full_cmd = if args.is_empty() {
                            cmd.to_string()
                        } else {
                            format!("{} {}", cmd, args.join(" "))
                        };
                        return Some(full_cmd);
                    }
                }
            }
        }
    }
    None
}

/// Attempt to extract a Python package name from README instructions
pub fn extract_python_package_from_readme(readme_content: &str) -> Option<String> {
    let re_uv = Regex::new(r"uv\s+pip\s+install\s+([\w\-]+)").ok();
    if let Some(re) = re_uv {
        if let Some(caps) = re.captures(readme_content) {
            return Some(caps.get(1)?.as_str().to_string());
        }
    }

    let re_pip = Regex::new(r"pip\s+install\s+([\w\-]+)").ok()?;
    if let Some(caps) = re_pip.captures(readme_content) {
        return Some(caps.get(1)?.as_str().to_string());
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
    fn test_extract_start_command_from_smithery_yaml() {
        let yaml = r#"
version: 1
startCommand:
  type: stdio
  commandFunction: |
    (config) => ({ command: 'python', args: ['-m', 'example.server'] })
"#;

        let cmd = extract_start_command_from_smithery_yaml(yaml).unwrap();
        assert_eq!(cmd, "python -m example.server");
    }

    #[test]
    fn test_extract_python_package_from_readme() {
        let readme = "Install via:\n```bash\nuv pip install sample-package\n```";
        let pkg = extract_python_package_from_readme(readme).unwrap();
        assert_eq!(pkg, "sample-package");
    }
}
