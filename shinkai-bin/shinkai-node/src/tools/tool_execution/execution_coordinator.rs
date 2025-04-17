use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_definitions::definition_generation::generate_tool_definitions;
use crate::tools::tool_execution::execute_agent_dynamic::execute_agent_tool;
use crate::tools::tool_execution::execution_custom::try_to_execute_rust_tool;
use crate::tools::tool_execution::execution_deno_dynamic::{check_deno_tool, execute_deno_tool};
use crate::tools::tool_execution::execution_header_generator::{check_tool, generate_execution_environment};
use crate::tools::tool_execution::execution_python_dynamic::execute_python_tool;
use crate::utils::environment::fetch_node_environment;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use ed25519_dalek::SigningKey;
use regex::Regex;
use reqwest::Client;
use serde_json::json;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::schemas::shinkai_tools::DynamicToolType;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_sqlite::oauth_manager::OAuthToken;
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::error::ToolError;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use shinkai_tools_primitives::tools::tool_config::{OAuth, ToolConfig};
use shinkai_tools_primitives::tools::tool_types::{OperatingSystem, RunnerType};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

pub async fn handle_oauth(
    oauth: &Option<Vec<OAuth>>,
    db: &Arc<SqliteManager>,
    app_id: String,
    tool_id: String,
    tool_router_key: String,
) -> Result<Value, ToolError> {
    let mut access_tokens: Vec<HashMap<String, String>> = vec![];
    if let Some(oauth_vec) = oauth {
        for o in oauth_vec {
            // Check if OAuth token already exists
            let existing_token = db
                .get_oauth_token(o.name.clone(), tool_router_key.clone())
                .ok()
                .unwrap_or(None);

            let (state_uuid, pkce_uuid) = if let Some(token) = existing_token.clone() {
                if let Some(_) = token.access_token.clone() {
                    // push to access_token

                    // check if token expiored
                    let mut u_token = token.clone();
                    if let Some(refresh_token_expires_at) = token.refresh_token_expires_at {
                        let now = Utc::now();
                        let five_minutes = chrono::Duration::minutes(5);

                        if now + five_minutes > refresh_token_expires_at {
                            // Need to refresh the token
                            if let Some(refresh_token) = &token.refresh_token {
                                if let Some(token_url) = &token.token_url {
                                    let client = Client::new();
                                    let request_body = serde_json::json!({
                                        "refresh_token": refresh_token,
                                        "grant_type": "refresh_token",
                                        "client_id": token.client_id.as_deref().unwrap_or_default(),
                                    });
                                    println!("[OAuth] Refresh request {}, {}", token_url, request_body.to_string());
                                    let response = client
                                        .post(token_url)
                                        .header("Accept", "application/json")
                                        .header("Content-Type", "application/x-www-form-urlencoded")
                                        .form(&request_body)
                                        .send()
                                        .await;

                                    match response {
                                        Ok(response) => {
                                            if !response.status().is_success() {
                                                return Err(ToolError::ExecutionError(format!(
                                                    "Failed to refresh OAuth token: {}",
                                                    response.status()
                                                )));
                                            }

                                            if let Ok(response_json) = response.json::<serde_json::Value>().await {
                                                println!("[OAuth] Response {}", response_json.to_string());
                                                if let Some(error) = response_json["error"].as_str() {
                                                    if !error.is_empty() {
                                                        return Err(ToolError::ExecutionError(format!(
                                                            "Failed to refresh OAuth token: {}",
                                                            response_json
                                                        )));
                                                    }
                                                }
                                                // Update token with new values
                                                let mut updated_token = token.clone();
                                                if let Some(access_token) = response_json["access_token"].as_str() {
                                                    updated_token.access_token = Some(access_token.to_string());
                                                }
                                                if let Some(expires_in) = response_json["expires_in"].as_i64() {
                                                    updated_token.access_token_expires_at =
                                                        Some(Utc::now() + chrono::Duration::seconds(expires_in));
                                                }
                                                if let Some(new_refresh_token) = response_json["refresh_token"].as_str()
                                                {
                                                    updated_token.refresh_token = Some(new_refresh_token.to_string());
                                                    if let Some(expires_in) = response_json["expires_in"].as_i64() {
                                                        updated_token.refresh_token_expires_at =
                                                            Some(Utc::now() + chrono::Duration::seconds(expires_in));
                                                    }
                                                }
                                                if let Some(refresh_token) = response_json["refresh_token"].as_str() {
                                                    updated_token.refresh_token = Some(refresh_token.to_string());
                                                }

                                                // Update token in database
                                                let _ = db.update_oauth_token(&updated_token);
                                                u_token = updated_token.clone();
                                            }
                                        }

                                        Err(e) => {
                                            println!("[OAuth] Response error {}", e.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let mut oauth = HashMap::new();
                    // TODO: Add more fields (?)
                    oauth.insert("name".to_string(), u_token.connection_name.clone());
                    oauth.insert("accessToken".to_string(), u_token.access_token.unwrap_or_default());
                    oauth.insert(
                        "expiresAt".to_string(),
                        u_token.expires_at.unwrap_or_default().to_string(),
                    );
                    access_tokens.push(oauth);
                    continue;
                }

                // Token is not setup, so pass the current state to regen the link.
                (token.state, token.pkce_code_verifier)
            } else {
                let state_uuid = uuid::Uuid::new_v4().to_string();
                let pkce_uuid = if let Some(_) = o.pkce_type.clone() {
                    let mut pkce_uuid = uuid::Uuid::new_v4().to_string();
                    // Left pad with zeros to ensure at least 64 characters
                    while pkce_uuid.len() < 64 {
                        pkce_uuid = format!("0{}", pkce_uuid);
                    }
                    Some(pkce_uuid)
                } else {
                    None
                };

                let has_refresh_token = if let Some(r) = o.refresh_token.clone() {
                    Some(r == "true".to_string())
                } else {
                    Some(false)
                };
                // Add new OAuth token record
                let oauth_token = OAuthToken {
                    id: 0, // db will set this
                    connection_name: o.name.clone(),
                    state: state_uuid.clone(),
                    code: None, // Created in instance call
                    app_id: app_id.clone(),
                    tool_id: tool_id.clone(),
                    tool_key: tool_router_key.clone(),
                    access_token: None,  // Fetched from oauth response or refresh
                    refresh_token: None, // Fetched from oauth response
                    token_secret: None,
                    response_type: o.response_type.clone(),
                    id_token: None,
                    scope: Some(o.scopes.join(" ")),
                    expires_at: None, // Fetched from oauth response or refresh
                    metadata_json: None,
                    authorization_url: Some(o.authorization_url.clone()),
                    token_url: o.token_url.clone(),
                    client_id: Some(o.client_id.clone()),
                    client_secret: Some(o.client_secret.clone()),
                    redirect_url: Some(o.redirect_url.clone()),
                    version: o.version.clone(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    access_token_expires_at: None, // Fetched from oauth response or refresh
                    refresh_token_enabled: has_refresh_token,
                    refresh_token_expires_at: None, //Fetched from oauth refresh
                    pkce_type: o.pkce_type.clone(),
                    pkce_code_verifier: pkce_uuid.clone(),
                    request_token_auth_header: o.request_token_auth_header.clone(),
                    request_token_content_type: o.request_token_content_type.clone(),
                };

                db.add_oauth_token(&oauth_token)
                    .map_err(|e| ToolError::ExecutionError(format!("Failed to store OAuth token: {}", e)))?;

                (state_uuid, pkce_uuid)
            };

            //https://twitter.com/i/oauth2/authorize?
            //response_type=code&
            //client_id=01234567890qwertyasdfzxcv1
            //redirect_uri=https://secrets.shinkai.com/redirect
            //scope=offline.access%20tweet.read%20tweet.write%20users.read
            //state=000000-111111-222222-333333
            //code_challenge=challenge
            //code_challenge_method=plain
            // Build query parameters
            let mut query_params = vec![
                ("response_type", o.response_type.clone()),
                ("client_id", o.client_id.clone()),
                ("redirect_uri", o.redirect_url.clone()),
                ("scope", o.scopes.join(" ")),
                ("state", state_uuid.clone()),
            ];

            // Add PKCE parameters if enabled
            if let Some(pkce_type) = &o.pkce_type {
                if let Some(pkce_uuid) = pkce_uuid.clone() {
                    match pkce_type.to_lowercase().as_str() {
                        "plain" => {
                            query_params.push(("code_challenge", pkce_uuid));
                            query_params.push(("code_challenge_method", "plain".to_string()));
                        }
                        "s256" => {
                            // Generate SHA256 hash of the verifier
                            let mut hasher = Sha256::new();
                            hasher.update(pkce_uuid.as_bytes());
                            let challenge = hasher.finalize();

                            // Base64url encode the challenge
                            let encoded_challenge = URL_SAFE_NO_PAD.encode(challenge);

                            query_params.push(("code_challenge", encoded_challenge));
                            query_params.push(("code_challenge_method", "S256".to_string()));
                        }
                        _ => {
                            println!("Unsupported PKCE type: {}", pkce_type);
                        }
                    }
                }
            }

            // Construct the OAuth URL by joining authorization_url with encoded query parameters
            let query_string: String = query_params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<String>>()
                .join("&");

            let oauth_login_url = format!("{}?{}", o.authorization_url, query_string);

            return Err(ToolError::OAuthError(oauth_login_url));
        }
    }
    Ok(serde_json::to_value(access_tokens).unwrap())
}

pub async fn execute_tool_cmd(
    bearer: String,
    node_name: ShinkaiName,
    db: Arc<SqliteManager>,
    tool_router_key: String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    extra_config: Vec<ToolConfig>,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
    mounts: Option<Vec<String>>,
) -> Result<Value, ToolError> {
    println!("[execute_tool] with tool_router_key: {}", tool_router_key);
    let tool = db
        .get_tool_by_key(&tool_router_key)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get tool: {}", e)))?;

    match tool {
        ShinkaiTool::Rust(_, _) => {
            try_to_execute_rust_tool(
                &tool_router_key,
                parameters,
                tool_id,
                app_id,
                extra_config,
                bearer,
                db,
                llm_provider,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
            )
            .await
        }
        ShinkaiTool::Agent(agent_tool, _) => {
            // Clone parameters and inject the agent_id
            let mut modified_parameters = parameters.clone();
            modified_parameters.insert(
                "agent_id".to_string(),
                serde_json::Value::String(agent_tool.agent_id.clone()),
            );

            // Use the dedicated execute_agent_tool function
            execute_agent_tool(
                bearer,
                db,
                modified_parameters,
                node_name,
                identity_manager,
                job_manager,
                encryption_secret_key,
                encryption_public_key,
                signing_secret_key,
            )
            .await
        }
        ShinkaiTool::Python(python_tool, _) => {
            let env = generate_execution_environment(
                db.clone(),
                llm_provider.clone(),
                app_id.clone(),
                tool_id.clone(),
                tool_router_key.clone(),
                "".to_string(), // TODO Pass data from the API
                &python_tool.oauth,
            )
            .await?;

            check_tool(
                tool_router_key.clone(),
                python_tool.config.clone(),
                parameters.clone(),
                python_tool.input_args.clone(),
                &python_tool.oauth,
            )?;

            let node_env = fetch_node_environment();
            let node_storage_path = node_env
                .node_storage_path
                .clone()
                .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
            let tools: Vec<ToolRouterKey> = db
                .clone()
                .get_all_tool_headers()
                .map_err(|_| ToolError::ExecutionError("Failed to get tool headers".to_string()))?
                .into_iter()
                .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                    Ok(tool_router_key) => Some(tool_router_key),
                    Err(_) => None,
                })
                .collect();

            let support_files = generate_tool_definitions(tools, CodeLanguage::Python, db, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            python_tool
                .run(
                    env,
                    node_env.api_listen_address.ip().to_string(),
                    node_env.api_listen_address.port(),
                    support_files,
                    parameters,
                    extra_config,
                    node_storage_path,
                    app_id.clone(),
                    tool_id.clone(),
                    node_name,
                    true,
                    Some(tool_router_key),
                    mounts,
                )
                .await
                .map(|result| json!(result.data))
        }
        ShinkaiTool::Deno(deno_tool, _) => {
            let env = generate_execution_environment(
                db.clone(),
                llm_provider.clone(),
                app_id.clone(),
                tool_id.clone(),
                tool_router_key.clone(),
                "".to_string(), // TODO Pass data from the API
                &deno_tool.oauth,
            )
            .await?;

            check_tool(
                tool_router_key.clone(),
                deno_tool.config.clone(),
                parameters.clone(),
                deno_tool.input_args.clone(),
                &deno_tool.oauth,
            )?;
            let node_env = fetch_node_environment();
            let node_storage_path = node_env
                .node_storage_path
                .clone()
                .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
            let tools: Vec<ToolRouterKey> = db
                .clone()
                .get_all_tool_headers()
                .map_err(|_| ToolError::ExecutionError("Failed to get tool headers".to_string()))?
                .into_iter()
                .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
                    Ok(tool_router_key) => Some(tool_router_key),
                    Err(_) => None,
                })
                .collect();

            let support_files = generate_tool_definitions(tools, CodeLanguage::Typescript, db, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            deno_tool
                .run(
                    env,
                    node_env.api_listen_address.ip().to_string(),
                    node_env.api_listen_address.port(),
                    support_files,
                    parameters,
                    extra_config,
                    node_storage_path,
                    app_id.clone(),
                    tool_id.clone(),
                    node_name,
                    true,
                    Some(tool_router_key),
                    mounts,
                )
                .await
                .map(|result| json!(result.data))
                .map_err(|e| ToolError::ExecutionError(e.to_string()))
        }
        _ => Err(ToolError::ExecutionError(format!("Unsupported tool type: {:?}", tool))),
    }
}

pub async fn execute_mcp_tool_cmd(
    bearer: String,
    node_name: ShinkaiName,
    db: Arc<SqliteManager>,
    tool_router_key: String,
    parameters: Map<String, Value>,
    tool_id: String,
    app_id: String,
    extra_config: Vec<ToolConfig>,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager: Arc<Mutex<JobManager>>,
    encryption_secret_key: EncryptionStaticKey,
    encryption_public_key: EncryptionPublicKey,
    signing_secret_key: SigningKey,
    mounts: Option<Vec<String>>,
) -> Result<Value, ToolError> {
    let tool = db
        .get_tool_by_key(&tool_router_key)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get tool: {}", e)))?;

    if !tool.is_mcp_enabled() {
        return Err(ToolError::ExecutionError("Tool is not MCP enabled".to_string()));
    }

    // Try to get the default LLM provider from preferences
    let preferences_llm_provider_result = match db.get_preference::<String>("default_llm_provider") {
        Ok(Some(provider_id)) => {
            // HARDCODED MAIN PROFILE NAME
            let profile_name =
                ShinkaiName::new(format!("{}/main/agent/mcp_default", node_name.get_node_name_string())).unwrap();
            match db.get_llm_provider(&provider_id, &profile_name) {
                Ok(Some(provider)) => {
                    // Successfully found the preferred provider
                    Ok(provider)
                }
                Ok(None) => {
                    // Preference ID exists but provider doesn't, trigger fallback
                    Err("Preference provider not found in DB")
                }
                Err(e) => {
                    // Error fetching preferred provider, trigger fallback
                    eprintln!("Error fetching preferred provider: {}", e);
                    Err("Error fetching preferred provider")
                }
            }
        }
        Ok(None) => {
            // Preference not set, trigger fallback
            Err("Preference not set")
        }
        Err(e) => {
            // Error getting preference, trigger fallback
            Err("Error getting preference")
        }
    };

    // Use the preferred provider if found, otherwise execute the fallback logic
    let first_llm_provider = preferences_llm_provider_result.or_else(|_reason| {
        // Fallback: Get the first provider from the list
        db.get_all_llm_providers()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to get llm providers: {}", e)))?
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::ExecutionError("No LLM providers found".to_string()))
    })?;

    let llm_provider = first_llm_provider.id.to_string();
    execute_tool_cmd(
        bearer,
        node_name,
        db,
        tool_router_key,
        parameters,
        tool_id,
        app_id,
        llm_provider,
        extra_config,
        identity_manager,
        job_manager,
        encryption_secret_key,
        encryption_public_key,
        signing_secret_key,
        mounts,
    )
    .await
}

pub async fn execute_code(
    tool_type: DynamicToolType,
    code: String,
    _tools: Vec<ToolRouterKey>,
    parameters: Map<String, Value>,
    extra_config: Vec<ToolConfig>,
    oauth: Option<Vec<OAuth>>,
    db: Arc<SqliteManager>,
    tool_id: String,
    app_id: String,
    llm_provider: String,
    bearer: String,
    node_name: ShinkaiName,
    mounts: Option<Vec<String>>,
    runner: Option<RunnerType>,
    operating_system: Option<Vec<OperatingSystem>>,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<Value, ToolError> {
    eprintln!("[execute_code] tool_type: {}", tool_type);
    // Route based on the prefix
    let tools: Vec<ToolRouterKey> = db
        .clone()
        .get_all_tool_headers()
        .map_err(|_| ToolError::ExecutionError("Failed to get tool headers".to_string()))?
        .into_iter()
        .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
            Ok(tool_router_key) => Some(tool_router_key),
            Err(_) => None,
        })
        .collect();

    match tool_type {
        DynamicToolType::DenoDynamic => {
            let support_files = generate_tool_definitions(tools, CodeLanguage::Typescript, db.clone(), false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            execute_deno_tool(
                bearer.clone(),
                db.clone(),
                node_name,
                parameters,
                extra_config,
                oauth.clone(),
                tool_id,
                app_id,
                llm_provider,
                support_files,
                code,
                mounts,
                runner,
                operating_system,
            )
            .await
        }
        DynamicToolType::PythonDynamic => {
            let support_files = generate_tool_definitions(tools, CodeLanguage::Python, db.clone(), false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            execute_python_tool(
                bearer.clone(),
                db.clone(),
                node_name,
                parameters,
                extra_config,
                oauth.clone(),
                tool_id,
                app_id,
                llm_provider,
                support_files,
                code,
                mounts,
                runner,
                operating_system,
            )
            .await
        }
        DynamicToolType::AgentDynamic => {
            execute_agent_tool(
                bearer,
                db,
                parameters,
                node_name,
                identity_manager_clone,
                job_manager_clone,
                encryption_secret_key_clone,
                encryption_public_key_clone,
                signing_secret_key_clone,
            )
            .await
        }
    }
}

pub async fn check_code(
    tool_type: DynamicToolType,
    unfiltered_code: String,
    tool_id: String,
    app_id: String,
    _tools: Vec<ToolRouterKey>,
    sqlite_manager: Arc<SqliteManager>,
) -> Result<Vec<String>, ToolError> {
    eprintln!("[check_code] tool_type: {}", tool_type);

    // Use the new function to extract fenced code blocks
    let code_blocks = extract_fenced_code_blocks(&unfiltered_code);
    let code_extracted = if !code_blocks.is_empty() {
        code_blocks.join("\n\n")
    } else {
        unfiltered_code
    };

    eprintln!("[check_code] code_extracted: {}", code_extracted);
    let tools: Vec<ToolRouterKey> = sqlite_manager
        .clone()
        .get_all_tool_headers()
        .map_err(|_| ToolError::ExecutionError("Failed to get tool headers".to_string()))?
        .into_iter()
        .filter_map(|tool| match ToolRouterKey::from_string(&tool.tool_router_key) {
            Ok(tool_router_key) => Some(tool_router_key),
            Err(_) => None,
        })
        .collect();

    match tool_type {
        DynamicToolType::DenoDynamic => {
            let support_files = generate_tool_definitions(tools, CodeLanguage::Typescript, sqlite_manager, false)
                .await
                .map_err(|_| ToolError::ExecutionError("Failed to generate tool definitions".to_string()))?;
            // Since `check_deno_tool` is synchronous, run it in a blocking task
            check_deno_tool(tool_id, app_id, support_files, code_extracted).await
        }
        DynamicToolType::PythonDynamic => Err(ToolError::ExecutionError("NYI Python".to_string())),
        DynamicToolType::AgentDynamic => Err(ToolError::ExecutionError("NYI Agent".to_string())),
    }
}

fn extract_fenced_code_blocks(unfiltered_code: &str) -> Vec<String> {
    // Updated pattern to handle both formats in the regex
    let re = Regex::new(r"```(?:\w+(?:\\n|\n))?([\s\S]*?)```").unwrap();
    let matches: Vec<String> = re
        .captures_iter(unfiltered_code)
        .map(|cap| cap[1].to_string())
        .collect();

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fenced_code_blocks() {
        let input = r#"
          Based on the provided documentation and code, I will implement a tool that downloads a website into markdown. This involves using the `deno` library to make an HTTP request to the website and then parsing the HTML response to extract relevant information.\n\nFirst, let's import the necessary libraries and define our function signature:\n```typescript\ntype CONFIG = {};\ntype INPUTS = {\n  url: string;\n};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const { url } = inputs;\n\n  // ...\n}\n```\nNext, we can use the `deno` library to make an HTTP request to the website:\n```typescript\nimport { fetch } from 'deno';\n\nconst response = await fetch(url);\nconst html = await response.text();\n```\nThen, we can parse the HTML response using a markdown parser. For this example, let's use the `marked` library, which is available on npm:\n```typescript\nimport { marked } from 'npm:marked';\n\nconst markdown = marked(html);\n```\nFinally, we can return the markdown as our output:\n```typescript\nreturn {\n  markdown,\n};\n}\n```\nPutting it all together, our `run` function would look like this:\n```typescript\nimport { fetch } from 'deno';\nimport { marked } from 'npm:marked';\n\ntype CONFIG = {};\ntype INPUTS = {\n  url: string;\n};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const { url } = inputs;\n\n  const response = await fetch(url);\n  const html = await response.text();\n  const markdown = marked(html);\n\n  return {\n    markdown,\n  };\n}\n```\nThis tool can be used to download a website into markdown by calling the `run` function with the URL of the website as an argument.\n\nHere is the complete code:\n\n```typescript\nimport { fetch } from 'deno';\nimport { marked } from 'npm:marked';\n\ntype CONFIG = {};\ntype INPUTS = {\n  url: string;\n};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const { url } = inputs;\n\n  const response = await fetch(url);\n  const html = await response.text();\n  const markdown = marked(html);\n\n  return {\n    markdown,\n  };\n}\n```\n\nPlease note that this code is a simple example and might not cover all edge cases. Depending on the complexity of the website, you might need to adjust the parsing logic accordingly.
        "#;

        let result = extract_fenced_code_blocks(input);
        let expected = vec![
            "type CONFIG = {};\\ntype INPUTS = {\\n  url: string;\\n};\\ntype OUTPUT = {};\\n\\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\\n  const { url } = inputs;\\n\\n  // ...\\n}\\n".to_string(),
            "import { fetch } from 'deno';\\n\\nconst response = await fetch(url);\\nconst html = await response.text();\\n".to_string(),
            "import { marked } from 'npm:marked';\\n\\nconst markdown = marked(html);\\n".to_string(),
            "return {\\n  markdown,\\n};\\n}\\n".to_string(),
            "import { fetch } from 'deno';\\nimport { marked } from 'npm:marked';\\n\\ntype CONFIG = {};\\ntype INPUTS = {\\n  url: string;\\n};\\ntype OUTPUT = {};\\n\\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\\n  const { url } = inputs;\\n\\n  const response = await fetch(url);\\n  const html = await response.text();\\n  const markdown = marked(html);\\n\\n  return {\\n    markdown,\\n  };\\n}\\n".to_string(),
            "import { fetch } from 'deno';\\nimport { marked } from 'npm:marked';\\n\\ntype CONFIG = {};\\ntype INPUTS = {\\n  url: string;\\n};\\ntype OUTPUT = {};\\n\\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\\n  const { url } = inputs;\\n\\n  const response = await fetch(url);\\n  const html = await response.text();\\n  const markdown = marked(html);\\n\\n  return {\\n    markdown,\\n  };\\n}\\n".to_string(),
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_extract_fenced_code_blocks_with_typescript() {
        let input = r#"Based on the provided documentation, we will implement a tool that downloads the webpage at `https://jhftss.github.io/` and converts it to plain text.

```typescript
import { getHomePath } from './shinkai-local-support.ts';

type CONFIG = {};
type INPUTS = {};
type OUTPUT = {};

export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {
  const url = 'https://jhftss.github.io/';
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const text = await response.text();
    const fileContent = text.replace(/<[^>]*>|[\n\r]/g, '');
    const filePath = `${getHomePath()}/downloaded_text.txt`;
    Deno.writeTextFileSync(filePath, fileContent);
  } catch (error) {
    console.error(error.message);
    return { error: 'Failed to download and convert webpage' };
  }
  return {};
}

```"#;

        let expected = vec![r#"import { getHomePath } from './shinkai-local-support.ts';

type CONFIG = {};
type INPUTS = {};
type OUTPUT = {};

export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {
  const url = 'https://jhftss.github.io/';
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    const text = await response.text();
    const fileContent = text.replace(/<[^>]*>|[\n\r]/g, '');
    const filePath = `${getHomePath()}/downloaded_text.txt`;
    Deno.writeTextFileSync(filePath, fileContent);
  } catch (error) {
    console.error(error.message);
    return { error: 'Failed to download and convert webpage' };
  }
  return {};
}

"#
        .to_string()];

        let result = extract_fenced_code_blocks(input);
        assert_eq!(result, expected);
    }
}
