use crate::network::node_error::NodeError;
use crate::network::Node;

use async_channel::Sender;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::Value;
use shinkai_sqlite::SqliteManager;

use std::sync::Arc;

use lazy_static::lazy_static;
use ngrok::{
    self,
    config::ForwarderBuilder,
    forwarder::Forwarder,
    tunnel::{EndpointInfo, HttpTunnel, TunnelCloser, TunnelInfo},
    session::Session,
};
use shinkai_http_api::node_api_router::APIError;
use tokio::sync::Mutex;
use url::Url;

// This will store the active ngrok tunnel, allowing it to persist.
lazy_static! {
    static ref ACTIVE_NGROK_SESSION: Mutex<Option<Session>> = Mutex::new(None);
    static ref ACTIVE_NGROK_TUNNEL: Mutex<Option<Forwarder<HttpTunnel>>> = Mutex::new(None);
}

#[derive(Serialize)]
pub struct NgrokStatus {
    enabled: bool,
    tunnel: Option<String>,
    authtoken: Option<String>,
}

impl Node {
    pub async fn v2_api_clear_ngrok_auth_token(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        match db.read_ngrok_auth_token() {
            Ok(Some(_)) => {
                match db.set_ngrok_auth_token(None) {
                    Ok(_) => {
                        let response = NgrokStatus {
                            enabled: false,
                            tunnel: None,
                            authtoken: None,
                        };
                        let _ = res.send(Ok(serde_json::to_value(response).unwrap())).await;
                        return Ok(());
                    }
                    Err(e) => {
                        let _ = res.send(Err(APIError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to update ngrok auth token in DB",
                            &e.to_string(),
                        )))
                        .await;
                        return Ok(());
                    }
                }
            }
            Ok(None) => {
                let _ = res.send(Err(APIError::new(
                    StatusCode::BAD_REQUEST,
                    "There is no ngrok auth token to clear",
                    "",
                )))
                .await;
                return Ok(());
            }
            Err(e) => {
                let _ = res.send(Err(APIError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to read ngrok auth token from DB",
                    &e.to_string(),
                )))
                .await;
                return Ok(());
            }
        }
    }

    pub async fn v2_api_set_ngrok_auth_token(
        db: Arc<SqliteManager>,
        bearer: String,
        auth_token: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        if auth_token.is_empty() {
            let _ = res.send(Err(APIError::new(
                StatusCode::BAD_REQUEST,
                "Auth token is required",
                "",
            )))
            .await;
            return Ok(());
        }

        let mut active_tunnel = ACTIVE_NGROK_TUNNEL.lock().await;

        if let Some(tunnel) = active_tunnel.take() {
            let tunnel_url = tunnel.url().to_string();

            let _ = res.send(Err(APIError::new(
                StatusCode::BAD_REQUEST,
                "Ngrok is already enabled",
                &format!("Ngrok tunnel is already enabled: {}", tunnel_url),
            )))
            .await;
            return Ok(());
        }

        if let Err(e) = db.set_ngrok_auth_token(Some(&auth_token)) {
            let _ = res.send(Err(APIError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update ngrok auth token in DB",
                &e.to_string(),
            )))
            .await;
            return Ok(());
        }

        let response = NgrokStatus {
            enabled: false,
            tunnel: None,
            authtoken: Some(auth_token),
        };

        let _ = res.send(Ok(serde_json::to_value(response).unwrap())).await;

        Ok(())
    }

    pub async fn v2_api_get_ngrok_status(
        db: Arc<SqliteManager>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let status = db.read_ngrok_auth_token().unwrap();

        let response = NgrokStatus {
            enabled: status.is_some(),
            tunnel: None,
            authtoken: status,
        };

        let _ = res.send(Ok(serde_json::to_value(response).unwrap())).await;

        Ok(())
    }

    pub async fn v2_api_set_ngrok_enabled(
        db: Arc<SqliteManager>,
        bearer: String,
        enabled: bool,
        api_listen_address: std::net::SocketAddr,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut active_tunnel_guard = ACTIVE_NGROK_TUNNEL.lock().await;

        if enabled {
            // If there's an existing tunnel, report its status and return.
            if let Some(ref existing_tunnel) = *active_tunnel_guard {
                let tunnel_url = existing_tunnel.url().to_string();
                let authtoken_for_status = match db.read_ngrok_auth_token() {
                    Ok(token_opt) => token_opt,
                    Err(e) => {
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                            shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                            shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Info,
                            &format!("Failed to read ngrok auth token for status (tunnel already active): {}", e),
                        );
                        None // If error reading, report as None for status
                    }
                };
                let response = NgrokStatus {
                    enabled: true,
                    tunnel: Some(tunnel_url),
                    authtoken: authtoken_for_status,
                };
                let _ = res.send(Ok(serde_json::to_value(response).unwrap())).await;
                return Ok(());
            }

            // No existing tunnel, proceed to create one.
            // First, get the auth token from DB. This is crucial for creating a new session if needed.
            let auth_token_from_db = match db.read_ngrok_auth_token() {
                Ok(token_opt) => token_opt,
                Err(e) => {
                    let _ = res.send(Err(APIError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to read ngrok auth token from DB before starting tunnel",
                        &e.to_string(),
                    ))).await;
                    return Ok(());
                }
            };

            // Session Management: Get or create an ngrok session
            let session_for_tunnel_creation: Session;
            { // Scoped to manage the lock on ACTIVE_NGROK_SESSION
                let active_ngrok_session_guard = ACTIVE_NGROK_SESSION.lock().await;
                if let Some(cached_session) = active_ngrok_session_guard.as_ref() {
                    shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Info,
                        &format!("Reusing existing ngrok session: {}", cached_session.id()),
                    );
                    session_for_tunnel_creation = cached_session.clone();
                    // active_ngrok_session_guard lock is released when it goes out of scope here
                } else {
                    // No cached session, so release the lock before .await for connect()
                    drop(active_ngrok_session_guard);

                    shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Info,
                        "No existing ngrok session found, creating a new one.",
                    );

                    // An auth token is required to create a new session.
                    let token_for_new_session = match &auth_token_from_db {
                        Some(token) => token.clone(),
                        None => {
                            let _ = res.send(Err(APIError::new(
                                StatusCode::BAD_REQUEST,
                                "Ngrok auth token is required to create a new ngrok session, but no token is set.",
                                "Please set the ngrok auth token first via the API.",
                            ))).await;
                            return Ok(());
                        }
                    };

                    let new_session = match ngrok::Session::builder()
                        .authtoken(token_for_new_session)
                        .connect()
                        .await
                    {
                        Ok(session) => session,
                        Err(e) => {
                            let _ = res.send(Err(APIError::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Failed to connect to ngrok to establish a new session",
                                &e.to_string(),
                            ))).await;
                            return Ok(());
                        }
                    };
                    
                    // Re-acquire lock to store the new session globally
                    let mut active_ngrok_session_guard_for_storing = ACTIVE_NGROK_SESSION.lock().await;
                    *active_ngrok_session_guard_for_storing = Some(new_session.clone());
                    session_for_tunnel_creation = new_session;
                    // active_ngrok_session_guard_for_storing lock is released when it goes out of scope here
                }
            } // End of scope for locks on ACTIVE_NGROK_SESSION

            let to_url = match Url::parse(&format!("http://{}:{}", api_listen_address.ip(), api_listen_address.port())) { // TODO: Make port configurable
                Ok(url) => url,
                Err(e) => {
                    let _ = res.send(Err(APIError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to parse ngrok target URL",
                        &e.to_string(),
                    ))).await;
                    return Ok(());
                }
            };

            let ngrok_tunnel = match session_for_tunnel_creation
                .http_endpoint()
                .metadata("Shinkai Node")
                .listen_and_forward(to_url)
                .await
            {
                Ok(tunnel) => tunnel,
                Err(e) => {
                    let _ = res.send(Err(APIError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to start ngrok tunnel",
                        &e.to_string(),
                    ))).await;
                    return Ok(());
                }
            };

            let tunnel_url = ngrok_tunnel.url().to_string();
            shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Info,
                &format!(
                    "NGROK Tunnel Started: {} (id: {})",
                    tunnel_url,
                    ngrok_tunnel.id()
                ),
            );

            // Store the tunnel in the static variable.
            *active_tunnel_guard = Some(ngrok_tunnel);

            let response = NgrokStatus {
                enabled, // true
                tunnel: Some(tunnel_url),
                authtoken: auth_token_from_db.clone(), // auth_token_from_db is Option<String>
            };
            let _ = res.send(Ok(serde_json::to_value(response).unwrap())).await;
        } else {
            // Disabling ngrok
            if let Some(mut tunnel_to_close) = active_tunnel_guard.take() {
                shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                    shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                    shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Info,
                    &format!("Closing ngrok tunnel: {}", tunnel_to_close.id()),
                );
                if let Err(e) = tunnel_to_close.close().await {
                    shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Error,
                        &format!("Failed to close ngrok tunnel: {}", e),
                    );
                    // Send error to client
                    let _ = res.send(Err(APIError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to close ngrok tunnel",
                        &e.to_string(),
                    ))).await;
                    // The original code proceeded to send a status update.
                    // Depending on desired behavior, one might return early here.
                    // For now, matching original behavior of still sending status.
                }
            } else {
                shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                    shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                    shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Info,
                    "No active ngrok tunnel found to disable.",
                );
            }

            let authtoken_for_status = match db.read_ngrok_auth_token() {
                Ok(token_opt) => token_opt,
                Err(e) => {
                    shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log(
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption::Node,
                        shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel::Error,
                        &format!("Failed to read ngrok auth token from DB for status when disabling: {}", e),
                    );
                    None 
                }
            };
            let response = NgrokStatus {
                enabled, // false
                tunnel: None,
                authtoken: authtoken_for_status,
            };
            let _ = res.send(Ok(serde_json::to_value(response).unwrap())).await;
        }

        Ok(())
    }
}
