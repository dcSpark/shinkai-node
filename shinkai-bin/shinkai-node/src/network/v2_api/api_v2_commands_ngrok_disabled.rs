use crate::network::node_error::NodeError;
use crate::network::Node;
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_http_api::node_api_router::APIError;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;

impl Node {
    pub async fn v2_api_clear_ngrok_auth_token(
        _db: Arc<SqliteManager>,
        _bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let _ = res
            .send(Err(APIError::new(
                StatusCode::NOT_IMPLEMENTED,
                "Not Implemented",
                "Ngrok feature is disabled",
            )))
            .await;
        Ok(())
    }

    pub async fn v2_api_set_ngrok_auth_token(
        _db: Arc<SqliteManager>,
        _bearer: String,
        _auth_token: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let _ = res
            .send(Err(APIError::new(
                StatusCode::NOT_IMPLEMENTED,
                "Not Implemented",
                "Ngrok feature is disabled",
            )))
            .await;
        Ok(())
    }

    pub async fn v2_api_get_ngrok_status(
        _db: Arc<SqliteManager>,
        _bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let _ = res
            .send(Err(APIError::new(
                StatusCode::NOT_IMPLEMENTED,
                "Not Implemented",
                "Ngrok feature is disabled",
            )))
            .await;
        Ok(())
    }

    pub async fn v2_api_set_ngrok_enabled(
        _db: Arc<SqliteManager>,
        _bearer: String,
        _enabled: bool,
        _api_listen_address: std::net::SocketAddr,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let _ = res
            .send(Err(APIError::new(
                StatusCode::NOT_IMPLEMENTED,
                "Not Implemented",
                "Ngrok feature is disabled",
            )))
            .await;
        Ok(())
    }
}
