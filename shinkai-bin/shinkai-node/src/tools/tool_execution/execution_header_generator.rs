use std::{collections::HashMap, sync::Arc};

use serde_json::{Map, Value};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{
    error::ToolError,
    parameters::Parameters,
    tool_config::{OAuth, ToolConfig},
};

use super::execution_coordinator::handle_oauth;

pub async fn generate_execution_environment(
    db: Arc<SqliteManager>,
    llm_provider: String,
    app_id: String,
    tool_id: String,
    tool_router_key: String,
    instance_id: String,
    oauth: &Option<Vec<OAuth>>,
) -> Result<HashMap<String, String>, ToolError> {
    let mut envs = HashMap::new();

    let bearer = db.read_api_v2_key().unwrap_or_default().unwrap_or_default();
    envs.insert("BEARER".to_string(), bearer);
    envs.insert("X_SHINKAI_TOOL_ID".to_string(), tool_id.clone());
    envs.insert("X_SHINKAI_APP_ID".to_string(), app_id.clone());
    envs.insert("X_SHINKAI_INSTANCE_ID".to_string(), instance_id.clone());
    envs.insert("X_SHINKAI_LLM_PROVIDER".to_string(), llm_provider);

    let oauth = handle_oauth(oauth, &db, app_id.clone(), tool_id.clone(), tool_router_key.clone()).await?;

    envs.insert("SHINKAI_OAUTH".to_string(), oauth.to_string());

    Ok(envs)
}

pub fn check_tool(
    tool_router_key: String,
    tool_config: Vec<ToolConfig>,
    value: Map<String, Value>,
    parameters: Parameters,
    oauth: &Option<Vec<OAuth>>,
) -> Result<(), ToolError> {
    check_tool_config(tool_router_key, tool_config)?;
    check_tool_parameters(parameters, value)?;
    check_oauth(oauth)?;
    Ok(())
}

fn check_oauth(oauth: &Option<Vec<OAuth>>) -> Result<(), ToolError> {
    if let Some(oauth_configs) = oauth {
        for oauth in oauth_configs {
            // Check if required fields are empty or missing
            let mut missing_fields = Vec::new();

            if oauth.name.is_empty() {
                missing_fields.push("name");
            }
            if oauth.authorization_url.is_empty() {
                missing_fields.push("authorization_url");
            }
            if oauth.token_url.as_ref().map_or(true, |url| url.is_empty()) {
                missing_fields.push("token_url");
            }
            if oauth.client_id.is_empty() {
                missing_fields.push("client_id");
            }
            if oauth.client_secret.is_empty() {
                missing_fields.push("client_secret");
            }
            if oauth.redirect_url.is_empty() {
                missing_fields.push("redirect_url");
            }
            if oauth.version.is_empty() {
                missing_fields.push("version");
            }
            if oauth.response_type.is_empty() {
                missing_fields.push("response_type");
            }

            if !missing_fields.is_empty() {
                let fix_redirect_url = format!("shinkai://config?tool={}", urlencoding::encode(&oauth.name));
                return Err(ToolError::MissingConfigError(format!(
                    "\n\nCannot run tool, OAuth config is missing required fields: {}.\n\nClick the link to update the tool config and try again.\n\n{}",
                    missing_fields.join(", "),
                    fix_redirect_url
                )));
            }
        }
    }
    Ok(())
}

fn check_tool_config(tool_router_key: String, tool_config: Vec<ToolConfig>) -> Result<(), ToolError> {
    for config in tool_config {
        println!("config: {:?}", config);
        match config {
            ToolConfig::BasicConfig(config) => {
                if config.key_value.is_none() && config.required {
                    let fix_redirect_url = format!("shinkai://config?tool={}", urlencoding::encode(&tool_router_key));
                    return Err(ToolError::MissingConfigError(format!(
                        "\n\nCannot run tool, config is for \"{}\" is missing.\n\nClick the link to update the tool config and try again.\n\n{}",
                        config.key_name, fix_redirect_url
                    )));
                }
            }
        }
    }

    Ok(())
}

fn validate_type(param_name: &str, param_value: &Value, property_type: &str, errors: &mut Vec<String>) {
    match property_type {
        "string" => {
            if !param_value.is_string() {
                errors.push(format!("Parameter '{}' must be a string", param_name));
            }
        }
        "number" | "numeric" => {
            if !param_value.is_number() {
                errors.push(format!("Parameter '{}' must be a number", param_name));
            }
        }
        "integer" => {
            if !param_value.is_number() || !param_value.as_f64().map_or(false, |n| n.fract() == 0.0) {
                errors.push(format!("Parameter '{}' must be an integer", param_name));
            }
        }
        "boolean" => {
            if !param_value.is_boolean() {
                errors.push(format!("Parameter '{}' must be a boolean", param_name));
            }
        }
        "array" => {
            if !param_value.is_array() {
                errors.push(format!("Parameter '{}' must be an array", param_name));
            } else {
                // If array has items type specified, validate each item
                if let Some(items) = property_type.strip_prefix("array<").and_then(|s| s.strip_suffix(">")) {
                    for (i, item) in param_value.as_array().unwrap().iter().enumerate() {
                        validate_type(&format!("{}[{}]", param_name, i), item, items, errors);
                    }
                }
            }
        }
        "object" => {
            if !param_value.is_object() {
                errors.push(format!("Parameter '{}' must be an object", param_name));
            } else {
                // If object has property types specified, validate each property
                if let Some(obj) = param_value.as_object() {
                    if let Some(properties) = property_type.strip_prefix("object<").and_then(|s| s.strip_suffix(">")) {
                        let property_types: serde_json::Map<String, Value> =
                            serde_json::from_str(properties).unwrap_or_default();
                        for (key, value) in obj {
                            if let Some(prop_type) = property_types.get(key) {
                                validate_type(
                                    &format!("{}.{}", param_name, key),
                                    value,
                                    prop_type.as_str().unwrap_or("any"),
                                    errors,
                                );
                            }
                        }
                    }
                }
            }
        }
        _ => {} // Skip validation for unknown types
    }
}

fn check_tool_parameters(parameters: Parameters, value: Map<String, Value>) -> Result<(), ToolError> {
    let mut errors = Vec::new();

    // Check if all required parameters are present and not null
    for required_param in &parameters.required {
        match value.get(required_param) {
            None => {
                errors.push(format!("Required parameter '{}' is missing", required_param));
            }
            Some(val) if val.is_null() => {
                errors.push(format!("Required parameter '{}' cannot be null", required_param));
            }
            Some(val) if val.is_string() && val.as_str().unwrap().is_empty() => {
                errors.push(format!(
                    "Required parameter '{}' cannot be an empty string",
                    required_param
                ));
            }
            _ => {}
        }
    }

    // Check parameter types
    for (param_name, param_value) in value.iter() {
        if let Some(property) = parameters.properties.get(param_name) {
            // Skip type checking for null values on optional parameters
            if param_value.is_null() && !parameters.required.contains(param_name) {
                continue;
            }
            validate_type(param_name, param_value, &property.property_type, &mut errors);
        }
    }

    if !errors.is_empty() {
        return Err(ToolError::InvalidFunctionArguments(format!(
            "Parameter validation failed:\n{}",
            errors.join("\n")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_tools_primitives::tools::tool_config::BasicConfig;

    fn create_test_parameters() -> Parameters {
        let mut params = Parameters::new();
        params.add_property(
            "string_param".to_string(),
            "string".to_string(),
            "A string parameter".to_string(),
            true,
        );
        params.add_property(
            "number_param".to_string(),
            "number".to_string(),
            "A number parameter".to_string(),
            true,
        );
        params.add_property(
            "optional_bool".to_string(),
            "boolean".to_string(),
            "An optional boolean parameter".to_string(),
            false,
        );
        params
    }

    #[test]
    fn test_check_tool_missing_required_config() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![ToolConfig::BasicConfig(BasicConfig {
            key_name: "required_config".to_string(),
            description: "A required config".to_string(),
            required: true,
            type_name: Some("string".to_string()),
            key_value: None, // Missing required value
        })];
        let value = Map::new();
        let parameters = Parameters::new();
        let oauth: Option<Vec<OAuth>> = None;

        let result = check_tool(tool_router_key, tool_config, value, parameters, &oauth);
        assert!(result.is_err());
        if let Err(ToolError::MissingConfigError(msg)) = result {
            assert!(msg.contains("required_config"));
            assert!(msg.contains("shinkai://config?tool=test%2Ftool"));
        } else {
            panic!("Expected MissingConfigError");
        }
    }

    #[test]
    fn test_check_tool_valid_config() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![ToolConfig::BasicConfig(BasicConfig {
            key_name: "required_config".to_string(),
            description: "A required config".to_string(),
            required: true,
            type_name: Some("string".to_string()),
            key_value: Some("value".to_string()), // Has required value
        })];
        let value = Map::new();
        let parameters = Parameters::new();
        let oauth: Option<Vec<OAuth>> = None;

        let result = check_tool(tool_router_key, tool_config, value, parameters, &oauth);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_tool_missing_required_parameter() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![];
        let mut value = Map::new();
        // Not providing the required string_param
        value.insert("number_param".to_string(), json!(42));
        let oauth: Option<Vec<OAuth>> = None;

        let result = check_tool(tool_router_key, tool_config, value, create_test_parameters(), &oauth);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("string_param"));
            assert!(msg.contains("missing"));
        } else {
            panic!("Expected InvalidFunctionArguments");
        }
    }

    #[test]
    fn test_check_tool_invalid_parameter_type() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![];
        let mut value = Map::new();
        value.insert("string_param".to_string(), json!("valid string"));
        value.insert("number_param".to_string(), json!("not a number")); // Wrong type
        let oauth: Option<Vec<OAuth>> = None;

        let result = check_tool(tool_router_key, tool_config, value, create_test_parameters(), &oauth);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("number_param"));
            assert!(msg.contains("must be a number"));
        } else {
            panic!("Expected InvalidFunctionArguments");
        }
    }

    #[test]
    fn test_check_tool_valid_parameters() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![];
        let mut value = Map::new();
        value.insert("string_param".to_string(), json!("valid string"));
        value.insert("number_param".to_string(), json!(42));
        value.insert("optional_bool".to_string(), json!(true));
        let oauth: Option<Vec<OAuth>> = None;

        let result = check_tool(tool_router_key, tool_config, value, create_test_parameters(), &oauth);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_tool_empty_required_string() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![];
        let mut value = Map::new();
        value.insert("string_param".to_string(), json!("")); // Empty string
        value.insert("number_param".to_string(), json!(42));
        let oauth: Option<Vec<OAuth>> = None;

        let result = check_tool(tool_router_key, tool_config, value, create_test_parameters(), &oauth);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("string_param"));
            assert!(msg.contains("cannot be an empty string"));
        } else {
            panic!("Expected InvalidFunctionArguments");
        }
    }

    #[test]
    fn test_check_tool_null_required_parameter() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![];
        let mut value = Map::new();
        value.insert("string_param".to_string(), json!(null)); // Null value
        value.insert("number_param".to_string(), json!(42));
        let oauth: Option<Vec<OAuth>> = None;

        let result = check_tool(tool_router_key, tool_config, value, create_test_parameters(), &oauth);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("string_param"));
            assert!(msg.contains("cannot be null"));
        } else {
            panic!("Expected InvalidFunctionArguments");
        }
    }

    #[test]
    fn test_check_oauth_missing_fields() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![];
        let value = Map::new();
        let parameters = Parameters::new();

        // Create OAuth config with missing fields
        let oauth = Some(vec![OAuth {
            name: "test_oauth".to_string(),
            authorization_url: "".to_string(), // Missing
            token_url: None,                   // Missing
            client_id: "".to_string(),         // Missing
            client_secret: "secret".to_string(),
            redirect_url: "https://example.com".to_string(),
            version: "2.0".to_string(),
            response_type: "code".to_string(),
            scopes: vec![],
            pkce_type: None,
            refresh_token: None,
        }]);

        let result = check_tool(tool_router_key, tool_config, value, parameters, &oauth);
        assert!(result.is_err());
        if let Err(ToolError::MissingConfigError(msg)) = result {
            assert!(msg.contains("authorization_url"));
            assert!(msg.contains("token_url"));
            assert!(msg.contains("client_id"));
            assert!(msg.contains("shinkai://config?tool=test_oauth"));
        } else {
            panic!("Expected MissingConfigError");
        }
    }

    #[test]
    fn test_check_oauth_valid_config() {
        let tool_router_key = "test/tool".to_string();
        let tool_config = vec![];
        let value = Map::new();
        let parameters = Parameters::new();

        // Create valid OAuth config
        let oauth = Some(vec![OAuth {
            name: "test_oauth".to_string(),
            scope: None,
            authorization_url: "https://auth.example.com".to_string(),
            token_url: Some("https://token.example.com".to_string()),
            client_id: "client123".to_string(),
            client_secret: "secret123".to_string(),
            redirect_url: "https://redirect.example.com".to_string(),
            version: "2.0".to_string(),
            response_type: "code".to_string(),
            scopes: vec![],
            pkce_type: None,
            refresh_token: None,
        }]);

        let result = check_tool(tool_router_key, tool_config, value, parameters, &oauth);
        assert!(result.is_ok());
    }
}
