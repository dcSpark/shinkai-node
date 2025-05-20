use log::info;
use regex::Regex;
use reqwest::Client;
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
