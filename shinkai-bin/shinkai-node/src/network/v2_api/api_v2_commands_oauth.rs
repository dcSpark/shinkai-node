use crate::network::node_error::NodeError;
use crate::network::Node;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_sqlite::SqliteManager;

use std::sync::Arc;
use tokio::sync::RwLock;

use reqwest::Client;
use shinkai_http_api::node_api_router::APIError;

impl Node {
    pub async fn v2_api_get_oauth_token(
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        connection_name: String,
        tool_key: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Get the OAuth token
        match db.read().await.get_oauth_token(connection_name, tool_key) {
            Ok(Some(token)) => match serde_json::to_value(token) {
                Ok(response) => {
                    let _ = res.send(Ok(response)).await;
                }
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to serialize OAuth token: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                }
            },
            Ok(None) => {
                let api_error = APIError {
                    code: StatusCode::NOT_FOUND.as_u16(),
                    error: "Not Found".to_string(),
                    message: "OAuth token not found".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
            }
            Err(err) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get OAuth token: {}", err),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn v2_api_set_oauth_token(
        db: Arc<RwLock<SqliteManager>>,
        bearer: String,
        code: String,
        state: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match Node::v2_api_set_oauth_token_cmd(db, code, state).await {
            Ok(_) => {
                let _ = res.send(Ok(Value::String("OAuth token set".to_string()))).await;
            }
            Err(e) => {
                let _ = res.send(Err(e)).await;
            }
        }
        Ok(())
    }

    async fn v2_api_set_oauth_token_cmd(
        db: Arc<RwLock<SqliteManager>>,
        code: String,
        state: String,
    ) -> Result<(), APIError> {
        let oauth_data = db.read().await.get_oauth_token_by_state(&state);
        if oauth_data.is_err() {
            return Err(APIError {
                code: StatusCode::NOT_FOUND.as_u16(),
                error: "Not Found".to_string(),
                message: "OAuth token not found for the given state".to_string(),
            });
        }
        let oauth_data = oauth_data.unwrap();
        if oauth_data.is_none() {
            return Err(APIError {
                code: StatusCode::NOT_FOUND.as_u16(),
                error: "Not Found".to_string(),
                message: "OAuth token not found for the given state".to_string(),
            });
        }
        let mut oauth_data = oauth_data.unwrap();

        let client = Client::new();
        let response = client
            .post("https://github.com/login/oauth/access_token")
            .query(&[
                ("client_id", "Ov23liXMvcIH8Wu38M3F"),
                ("client_secret", "f91323d3b04ec0511e888aa8da07ec2dc548d262"),
                ("code", &code),
                ("redirect_uri", "https://secrets.shinkai.com/redirect"),
            ])
            .header("Accept", "application/json")
            .send()
            .await;
        if response.is_err() {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to get OAuth token: {}", response.err().unwrap()),
            });
        }
        let response = response.unwrap();
        let response = response.json::<serde_json::Value>().await;
        if response.is_err() {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to parse OAuth response: {}", response.err().unwrap()),
            });
        }
        let response = response.unwrap();

        // Update the token with the new code and OAuth response data
        oauth_data.code = Some(code);
        if let Some(access_token) = response["access_token"].as_str() {
            oauth_data.access_token = Some(access_token.to_string());
        }
        if let Some(token_type) = response["token_type"].as_str() {
            oauth_data.token_type = Some(token_type.to_string());
        }
        if let Some(scope) = response["scope"].as_str() {
            oauth_data.scope = Some(scope.to_string());
        }

        let update_result = db.read().await.update_oauth_token(&oauth_data);
        if update_result.is_err() {
            return Err(APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to update OAuth token: {}", update_result.err().unwrap()),
            });
        }
        // let update_result = update_result.unwrap();
        Ok(())
    }
}
