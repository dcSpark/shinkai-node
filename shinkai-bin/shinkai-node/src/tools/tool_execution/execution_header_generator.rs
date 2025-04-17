use std::{collections::HashMap, sync::Arc};

use serde_json::{Map, Value};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{
    error::ToolError,
    parameters::{Parameters, Property},
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

    check_oauth(oauth, &tool_router_key)?;
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
    check_oauth(oauth, &tool_router_key)?;
    check_tool_config(tool_router_key, tool_config)?;
    check_tool_parameters(parameters, value)?;
    Ok(())
}

fn check_oauth(oauth: &Option<Vec<OAuth>>, tool_router_key: &str) -> Result<(), ToolError> {
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
                let fix_redirect_url = format!("shinkai://config?tool={}", urlencoding::encode(tool_router_key));
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

fn validate_type(param_name: &str, param_value: &Value, property: &Property, errors: &mut Vec<String>) {
    match property.property_type.as_str() {
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
            } else if let Some(items) = &property.items {
                // Validate each array item against the items type
                for (i, item) in param_value.as_array().unwrap().iter().enumerate() {
                    validate_type(&format!("{}[{}]", param_name, i), item, items, errors);
                }
            }
        }
        "object" => {
            if !param_value.is_object() {
                errors.push(format!("Parameter '{}' must be an object", param_name));
            } else if let Some(properties) = &property.properties {
                // Validate each property of the object
                let obj = param_value.as_object().unwrap();
                for (key, prop) in properties {
                    if let Some(value) = obj.get(key) {
                        validate_type(&format!("{}.{}", param_name, key), value, prop, errors);
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
            validate_type(param_name, param_value, property, &mut errors);
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
            key_value: Some(serde_json::Value::String("value".to_string())), // Has required value
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
            request_token_auth_header: None,
            request_token_content_type: None,
        }]);

        let result = check_tool(tool_router_key, tool_config, value, parameters, &oauth);
        assert!(result.is_err());
        if let Err(ToolError::MissingConfigError(msg)) = result {
            assert!(msg.contains("authorization_url"));
            assert!(msg.contains("token_url"));
            assert!(msg.contains("client_id"));
            assert!(msg.contains("shinkai://config?tool=test%2Ftool"));
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
            request_token_auth_header: None,
            request_token_content_type: None,
        }]);

        let result = check_tool(tool_router_key, tool_config, value, parameters, &oauth);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_tool_config_missing_required() {
        let tool_router_key = "test/tool".to_string();
        let config = vec![ToolConfig::BasicConfig(BasicConfig {
            key_name: "api_key".to_string(),
            description: "API key".to_string(),
            required: true,
            type_name: Some("string".to_string()),
            key_value: None, // Missing required value
        })];

        let result = check_tool_config(tool_router_key, config);
        assert!(result.is_err());
        if let Err(ToolError::MissingConfigError(msg)) = result {
            assert!(msg.contains("api_key"));
            assert!(msg.contains("shinkai://config?tool=test%2Ftool"));
        } else {
            panic!("Expected MissingConfigError");
        }
    }

    #[test]
    fn test_check_tool_config_optional_missing() {
        let tool_router_key = "test/tool".to_string();
        let config = vec![ToolConfig::BasicConfig(BasicConfig {
            key_name: "optional_key".to_string(),
            description: "Optional key".to_string(),
            required: false,
            type_name: Some("string".to_string()),
            key_value: None, // Missing but optional
        })];

        let result = check_tool_config(tool_router_key, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_tool_config_multiple_configs() {
        let tool_router_key = "test/tool".to_string();
        let config = vec![
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "required_key".to_string(),
                description: "Required key".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: Some(serde_json::Value::String("value".to_string())),
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "optional_key".to_string(),
                description: "Optional key".to_string(),
                required: false,
                type_name: Some("string".to_string()),
                key_value: None,
            }),
        ];

        let result = check_tool_config(tool_router_key, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_tool_parameters_array_type() {
        let mut params = Parameters::new();
        params.add_property(
            "array_param".to_string(),
            "array".to_string(),
            "An array parameter".to_string(),
            true,
        );

        let mut value = Map::new();
        value.insert("array_param".to_string(), json!(["item1", "item2"]));

        let result = check_tool_parameters(params, value);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_tool_parameters_array_type_invalid() {
        let mut params = Parameters::new();
        params.add_property(
            "array_param".to_string(),
            "array".to_string(),
            "An array parameter".to_string(),
            true,
        );

        let mut value = Map::new();
        value.insert("array_param".to_string(), json!("not an array"));

        let result = check_tool_parameters(params, value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("array_param"));
            assert!(msg.contains("must be an array"));
        } else {
            panic!("Expected InvalidFunctionArguments");
        }
    }

    #[test]
    fn test_check_tool_parameters_object_type() {
        let mut params = Parameters::new();
        params.add_property(
            "object_param".to_string(),
            "object".to_string(),
            "An object parameter".to_string(),
            true,
        );

        let mut value = Map::new();
        value.insert("object_param".to_string(), json!({"key": "value"}));

        let result = check_tool_parameters(params, value);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_tool_parameters_object_type_invalid() {
        let mut params = Parameters::new();
        params.add_property(
            "object_param".to_string(),
            "object".to_string(),
            "An object parameter".to_string(),
            true,
        );

        let mut value = Map::new();
        value.insert("object_param".to_string(), json!("not an object"));

        let result = check_tool_parameters(params, value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("object_param"));
            assert!(msg.contains("must be an object"));
        } else {
            panic!("Expected InvalidFunctionArguments");
        }
    }

    #[test]
    fn test_check_tool_parameters_integer_type() {
        let mut params = Parameters::new();
        params.add_property(
            "integer_param".to_string(),
            "integer".to_string(),
            "An integer parameter".to_string(),
            true,
        );

        let mut value = Map::new();
        value.insert("integer_param".to_string(), json!(42));
        assert!(check_tool_parameters(params.clone(), value).is_ok());

        // Test with float that should fail
        let mut value = Map::new();
        value.insert("integer_param".to_string(), json!(42.5));
        assert!(check_tool_parameters(params, value).is_err());
    }

    #[test]
    fn test_check_tool_parameters_multiple_errors() {
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

        let mut value = Map::new();
        value.insert("string_param".to_string(), json!(42)); // Wrong type
        value.insert("number_param".to_string(), json!("not a number")); // Wrong type

        let result = check_tool_parameters(params, value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("string_param"));
            assert!(msg.contains("number_param"));
            assert!(msg.contains("must be a string"));
            assert!(msg.contains("must be a number"));
        } else {
            panic!("Expected InvalidFunctionArguments");
        }
    }

    #[test]
    fn test_check_tool_parameters_null_optional() {
        let mut params = Parameters::new();
        params.add_property(
            "optional_param".to_string(),
            "string".to_string(),
            "An optional parameter".to_string(),
            false,
        );

        let mut value = Map::new();
        value.insert("optional_param".to_string(), json!(null));

        let result = check_tool_parameters(params, value);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_oauth_all_fields_valid() {
        let oauth = Some(vec![OAuth {
            name: "test_oauth".to_string(),
            authorization_url: "https://auth.example.com".to_string(),
            token_url: Some("https://token.example.com".to_string()),
            client_id: "client123".to_string(),
            client_secret: "secret123".to_string(),
            redirect_url: "https://redirect.example.com".to_string(),
            version: "2.0".to_string(),
            response_type: "code".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
            pkce_type: Some("S256".to_string()),
            refresh_token: Some("refresh123".to_string()),
            request_token_auth_header: None,
            request_token_content_type: None,
        }]);

        let result = check_oauth(&oauth, "test_oauth");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_oauth_empty_name() {
        let oauth = Some(vec![OAuth {
            name: "".to_string(), // Empty name
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
            request_token_auth_header: None,
            request_token_content_type: None,
        }]);

        let result = check_oauth(&oauth, "test/tool");
        assert!(result.is_err());
        if let Err(ToolError::MissingConfigError(msg)) = result {
            assert!(msg.contains("name"));
            assert!(msg.contains("shinkai://config?tool=test%2Ftool"));
        } else {
            panic!("Expected MissingConfigError");
        }
    }

    #[test]
    fn test_check_oauth_multiple_configs() {
        let oauth = Some(vec![
            OAuth {
                name: "oauth1".to_string(),
                authorization_url: "https://auth1.example.com".to_string(),
                token_url: Some("https://token1.example.com".to_string()),
                client_id: "client1".to_string(),
                client_secret: "secret1".to_string(),
                redirect_url: "https://redirect1.example.com".to_string(),
                version: "2.0".to_string(),
                response_type: "code".to_string(),
                scopes: vec![],
                pkce_type: None,
                refresh_token: None,
                request_token_auth_header: None,
                request_token_content_type: None,
            },
            OAuth {
                name: "oauth2".to_string(),
                authorization_url: "https://auth2.example.com".to_string(),
                token_url: Some("https://token2.example.com".to_string()),
                client_id: "client2".to_string(),
                client_secret: "secret2".to_string(),
                redirect_url: "https://redirect2.example.com".to_string(),
                version: "2.0".to_string(),
                response_type: "code".to_string(),
                scopes: vec![],
                pkce_type: None,
                refresh_token: None,
                request_token_auth_header: None,
                request_token_content_type: None,
            },
        ]);

        let result = check_oauth(&oauth, "test_oauth");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_oauth_missing_multiple_fields() {
        let oauth = Some(vec![OAuth {
            name: "test_oauth".to_string(),
            authorization_url: "".to_string(), // Empty
            token_url: None,                   // Missing
            client_id: "".to_string(),         // Empty
            client_secret: "secret123".to_string(),
            redirect_url: "".to_string(),  // Empty
            version: "".to_string(),       // Empty
            response_type: "".to_string(), // Empty
            scopes: vec![],
            pkce_type: None,
            refresh_token: None,
            request_token_auth_header: None,
            request_token_content_type: None,
        }]);

        let result = check_oauth(&oauth, "test_oauth");
        assert!(result.is_err());
        if let Err(ToolError::MissingConfigError(msg)) = result {
            assert!(msg.contains("authorization_url"));
            assert!(msg.contains("token_url"));
            assert!(msg.contains("client_id"));
            assert!(msg.contains("redirect_url"));
            assert!(msg.contains("version"));
            assert!(msg.contains("response_type"));
            assert!(msg.contains("shinkai://config?tool=test_oauth"));
        } else {
            panic!("Expected MissingConfigError");
        }
    }

    #[test]
    fn test_check_oauth_none() {
        let oauth: Option<Vec<OAuth>> = None;
        let result = check_oauth(&oauth, "test_oauth");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_oauth_empty_vec() {
        let oauth = Some(vec![]);
        let result = check_oauth(&oauth, "test_oauth");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_oauth_one_valid_one_invalid() {
        let oauth = Some(vec![
            OAuth {
                name: "valid_oauth".to_string(),
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
                request_token_auth_header: None,
                request_token_content_type: None,
            },
            OAuth {
                name: "invalid_oauth".to_string(),
                authorization_url: "".to_string(), // Invalid
                token_url: None,                   // Invalid
                client_id: "client123".to_string(),
                client_secret: "secret123".to_string(),
                redirect_url: "https://redirect.example.com".to_string(),
                version: "2.0".to_string(),
                response_type: "code".to_string(),
                scopes: vec![],
                pkce_type: None,
                refresh_token: None,
                request_token_auth_header: None,
                request_token_content_type: None,
            },
        ]);

        let result = check_oauth(&oauth, "test/tool");
        assert!(result.is_err());
        if let Err(ToolError::MissingConfigError(msg)) = result {
            assert!(msg.contains("authorization_url"));
            assert!(msg.contains("token_url"));
            assert!(msg.contains("shinkai://config?tool=test%2Ftool"));
        } else {
            panic!("Expected MissingConfigError");
        }
    }

    #[test]
    fn test_check_tool_parameters_array_validation() {
        let mut params = Parameters::new();

        // Create an array of strings property
        let string_prop = Property::new("string".to_string(), "A string item".to_string());
        let array_prop = Property::with_array_items("An array of strings".to_string(), string_prop);
        params.properties.insert("tags".to_string(), array_prop);
        params.required.push("tags".to_string());

        // Test valid array
        let mut value = Map::new();
        value.insert("tags".to_string(), json!(["tag1", "tag2"]));
        assert!(check_tool_parameters(params.clone(), value).is_ok());

        // Test invalid array item type
        let mut value = Map::new();
        value.insert("tags".to_string(), json!(["tag1", 42]));
        let result = check_tool_parameters(params.clone(), value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("'tags[1]' must be a string"));
        }

        // Test non-array value
        let mut value = Map::new();
        value.insert("tags".to_string(), json!("not an array"));
        let result = check_tool_parameters(params, value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("'tags' must be an array"));
        }
    }

    #[test]
    fn test_check_tool_parameters_nested_object_validation() {
        let mut params = Parameters::new();

        // Create nested user object property
        let mut user_props = std::collections::HashMap::new();
        user_props.insert(
            "name".to_string(),
            Property::new("string".to_string(), "The user's name".to_string()),
        );
        user_props.insert(
            "age".to_string(),
            Property::new("integer".to_string(), "The user's age".to_string()),
        );

        params.add_nested_property(
            "user".to_string(),
            "object".to_string(),
            "User information".to_string(),
            user_props,
            true,
        );

        // Test valid nested object
        let mut value = Map::new();
        value.insert(
            "user".to_string(),
            json!({
                "name": "John",
                "age": 30
            }),
        );
        assert!(check_tool_parameters(params.clone(), value).is_ok());

        // Test invalid nested property type
        let mut value = Map::new();
        value.insert(
            "user".to_string(),
            json!({
                "name": "John",
                "age": "thirty" // Should be integer
            }),
        );
        let result = check_tool_parameters(params.clone(), value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("'user.age' must be an integer"));
        }

        // Test non-object value
        let mut value = Map::new();
        value.insert("user".to_string(), json!("not an object"));
        let result = check_tool_parameters(params, value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("'user' must be an object"));
        }
    }

    #[test]
    fn test_check_tool_parameters_array_of_objects() {
        let mut params = Parameters::new();

        // Create an array of user objects
        let mut user_props = std::collections::HashMap::new();
        user_props.insert(
            "name".to_string(),
            Property::new("string".to_string(), "The user's name".to_string()),
        );
        user_props.insert(
            "age".to_string(),
            Property::new("integer".to_string(), "The user's age".to_string()),
        );

        let object_prop =
            Property::with_nested_properties("object".to_string(), "A user object".to_string(), user_props);
        let array_prop = Property::with_array_items("List of users".to_string(), object_prop);

        params.properties.insert("users".to_string(), array_prop);
        params.required.push("users".to_string());

        // Test valid array of objects
        let mut value = Map::new();
        value.insert(
            "users".to_string(),
            json!([
                {"name": "John", "age": 30},
                {"name": "Jane", "age": 25}
            ]),
        );
        assert!(check_tool_parameters(params.clone(), value).is_ok());

        // Test invalid nested property in array item
        let mut value = Map::new();
        value.insert(
            "users".to_string(),
            json!([
                {"name": "John", "age": 30},
                {"name": "Jane", "age": "twenty-five"} // Should be integer
            ]),
        );
        let result = check_tool_parameters(params.clone(), value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("'users[1].age' must be an integer"));
        }

        // Test non-array value
        let mut value = Map::new();
        value.insert("users".to_string(), json!({"name": "John", "age": 30}));
        let result = check_tool_parameters(params, value);
        assert!(result.is_err());
        if let Err(ToolError::InvalidFunctionArguments(msg)) = result {
            assert!(msg.contains("'users' must be an array"));
        }
    }
}
