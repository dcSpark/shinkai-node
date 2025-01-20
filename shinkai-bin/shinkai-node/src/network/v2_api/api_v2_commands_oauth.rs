use crate::network::node_error::NodeError;
use crate::network::Node;

use async_channel::Sender;
use chrono::Utc;
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_sqlite::SqliteManager;

use std::sync::Arc;

use reqwest::Client;
use shinkai_http_api::node_api_router::APIError;

impl Node {
    pub async fn v2_api_get_oauth_token(
        db: Arc<SqliteManager>,
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
        match db.get_oauth_token(connection_name, tool_key) {
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
        db: Arc<SqliteManager>,
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

    async fn v2_api_set_oauth_token_cmd(db: Arc<SqliteManager>, code: String, state: String) -> Result<(), APIError> {
        let oauth_data = db.get_oauth_token_by_state(&state);
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
        let mut request_body = serde_json::json!({
            "client_id": oauth_data.client_id.as_deref().unwrap_or_default(),
            // "client_secret": oauth_data.client_secret.as_deref().unwrap_or_default(),
            "code": code,
            "redirect_uri": oauth_data.redirect_url.as_deref().unwrap_or_default(),
            "grant_type": "authorization_code"
        });

        // Add code_verifier if PKCE is enabled
        if oauth_data.pkce_type.is_some() {
            // TODO For now we only support plain.
            if let Some(verifier) = &oauth_data.pkce_code_verifier {
                request_body["code_verifier"] = serde_json::Value::String(verifier.clone());
            }
        }
        let url = &oauth_data.clone().token_url.unwrap_or_default();

        println!("[OAuth] Calling {} with params {:?}", url, request_body);
        let response = client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&request_body)
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
        println!("[OAuth] Response {}", response.clone().to_string());

        // Response example
        // {
        //   "access_token":"ODc1WmJXZH....",
        //   "expires_in":7200,
        //   "refresh_token":"RXBFWHQ2NUN....",
        //   "scope":"tweet.write users.read tweet.read offline.access",
        //   "token_type":"bearer"
        // }
        // Update the token with the new code and OAuth response data
        oauth_data.code = Some(code);
        if let Some(access_token) = response["access_token"].as_str() {
            oauth_data.access_token = Some(access_token.to_string());
        }
        if let Some(expires_in) = response["expires_in"].as_i64() {
            oauth_data.access_token_expires_at = Some(Utc::now() + chrono::Duration::seconds(expires_in));
        }
        if let Some(refresh_token) = response["refresh_token"].as_str() {
            oauth_data.refresh_token = Some(refresh_token.to_string());
            if let Some(expires_in) = response["expires_in"].as_i64() {
                oauth_data.refresh_token_expires_at = Some(Utc::now() + chrono::Duration::seconds(expires_in));
            }
        }
        if let Some(scope) = response["scope"].as_str() {
            oauth_data.scope = Some(scope.to_string());
        }

        let update_result = db.update_oauth_token(&oauth_data.clone());
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
