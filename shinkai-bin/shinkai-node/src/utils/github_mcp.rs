use log::info;
use regex::Regex;
use reqwest::{Client, StatusCode};
use serde_yaml::Value as YamlValue;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitHubMcpError {
    #[error("Invalid GitHub URL: {0}")]
    InvalidGitHubUrl(String),
    #[error("HTTP request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Failed to fetch file {path}: HTTP {status}")]
    HttpStatusError { path: String, status: StatusCode },
    #[error("TOML parse error: {0}")]
    TomlError(#[from] toml::de::Error),
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Missing field: {0}")]
    MissingField(String),
    #[error("{0}")]
    Other(String),
}

/// GitHub repository information
pub struct GitHubRepo {
    pub owner: String,
    pub repo: String,
    pub url: String,
}

/// Parse a GitHub URL to extract owner and repository name
pub fn parse_github_url(url: &str) -> Result<GitHubRepo, GitHubMcpError> {
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

    Err(GitHubMcpError::InvalidGitHubUrl(url.to_string()))
}

/// Fetch a file from a GitHub repository
pub async fn fetch_github_file(client: &Client, owner: &str, repo: &str, path: &str) -> Result<String, GitHubMcpError> {
    let url = format!("https://raw.githubusercontent.com/{}/{}/main/{}", owner, repo, path);

    info!("Fetching file from GitHub: {}", url);

    let response = client.get(&url).send().await.map_err(GitHubMcpError::RequestError)?;
    if response.status().is_success() {
        return response.text().await.map_err(GitHubMcpError::RequestError);
    }

    // Try with master branch if main fails
    let master_url = format!("https://raw.githubusercontent.com/{}/{}/master/{}", owner, repo, path);
    let master_response = client
        .get(&master_url)
        .send()
        .await
        .map_err(GitHubMcpError::RequestError)?;
    if master_response.status().is_success() {
        return master_response.text().await.map_err(GitHubMcpError::RequestError);
    }

    Err(GitHubMcpError::HttpStatusError {
        path: master_url,
        status: master_response.status(),
    })
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
}
