use crate::api_v1;
use crate::api_v2;
use crate::api_sse;

use super::node_commands::NodeCommand;
use async_channel::Sender;
use hyper::server::conn::Http;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::json;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::shinkai_log;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::rustls::{self, ServerConfig};
use tokio_rustls::TlsAcceptor;
use utoipa::ToSchema;
use warp::Filter;

#[derive(serde::Serialize, ToSchema, Debug, Clone)]
pub struct SendResponseBodyData {
    pub message_id: String,
    pub parent_message_id: Option<String>,
    pub inbox: String,
    pub scheduled_time: String,
}

#[derive(serde::Serialize, ToSchema, Debug, Clone)]
pub struct SendResponseBody {
    pub status: String,
    pub message: String,
    pub data: Option<SendResponseBodyData>,
}

#[derive(serde::Serialize, ToSchema, Debug, Clone)]
pub struct GetPublicKeysResponse {
    pub signature_public_key: String,
    pub encryption_public_key: String,
}

#[derive(Serialize, ToSchema, Debug, Clone)]
pub struct APIError {
    pub code: u16,
    pub error: String,
    pub message: String,
}

impl APIError {
    pub fn new(code: StatusCode, error: &str, message: &str) -> Self {
        Self {
            code: code.as_u16(),
            error: error.to_string(),
            message: message.to_string(),
        }
    }
}

impl From<&str> for APIError {
    fn from(error: &str) -> Self {
        APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: error.to_string(),
        }
    }
}

impl From<async_channel::SendError<NodeCommand>> for APIError {
    fn from(error: async_channel::SendError<NodeCommand>) -> Self {
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Failed with error: {}", error),
        }
    }
}

impl From<String> for APIError {
    fn from(error: String) -> Self {
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: error,
        }
    }
}

impl warp::reject::Reject for APIError {}

pub async fn run_api(
    node_commands_sender: Sender<NodeCommand>,
    address: SocketAddr,
    https_address: SocketAddr,
    node_name: String,
    private_https_certificate: Option<String>,
    public_https_certificate: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    shinkai_log(
        ShinkaiLogOption::Api,
        ShinkaiLogLevel::Info,
        &format!("Starting Node API server at: {}", &address),
    );

    let log = warp::log::custom(|info| {
        shinkai_log(
            ShinkaiLogOption::Api,
            ShinkaiLogLevel::Debug,
            &format!(
                "ip: {:?}, method: {:?}, path: {:?}, status: {:?}, elapsed: {:?}",
                info.remote_addr(),
                info.method(),
                info.path(),
                info.status(),
                info.elapsed(),
            ),
        );
    });

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "OPTIONS", "DELETE"])
        .allow_headers(vec![
            "Content-Type",
            "Authorization",
            "x-shinkai-tool-id",
            "x-shinkai-app-id",
            "x-shinkai-llm-provider",
            "x-shinkai-original-tool-router-key"
        ]);

    let v1_routes = warp::path("v1").and(
        api_v1::api_v1_router::v1_routes(node_commands_sender.clone(), node_name.clone())
            .recover(handle_rejection)
            .with(log)
            .with(cors.clone()),
    );

    let v2_routes = warp::path("v2").and(
        api_v2::api_v2_router::v2_routes(node_commands_sender.clone(), node_name.clone())
            .recover(handle_rejection)
            .with(log)
            .with(cors.clone()),
    );

    let mcp_routes = warp::path("mcp").and(
        api_sse::api_sse_routes::mcp_sse_routes(node_commands_sender.clone(), node_name.clone())
            .recover(handle_rejection)
            .with(log)
            .with(cors.clone()),
    );

    // Combine all routes
    let routes = v1_routes
        .or(v2_routes)
        .or(mcp_routes)
        .with(log).with(cors);

    // Wrap the HTTP server in an async block that returns a Result
    let http_server = async {
        warp::serve(routes.clone()).run(address).await;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    };

    // Check if certificates are provided for HTTPS
    if let (Some(cert_string), Some(key_string)) = (public_https_certificate, private_https_certificate) {
        if !cert_string.is_empty() && !key_string.is_empty() {
            eprintln!("cert_string: {}", cert_string);
            eprintln!("key_string: {}", key_string);

            // Parse TLS keys from strings
            let certs = rustls_pemfile::certs(&mut cert_string.as_bytes())
                .map_err(|_| "Failed to parse certificates")?
                .into_iter()
                .map(rustls::Certificate)
                .collect();

            let key = rustls_pemfile::pkcs8_private_keys(&mut key_string.as_bytes())
                .map_err(|_| "Failed to parse private key")?
                .into_iter()
                .next()
                .ok_or("No private key found")
                .map(rustls::PrivateKey)?; // Use the PrivateKey constructor

            let config = ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_single_cert(certs, key)?;

            let tls_acceptor = TlsAcceptor::from(Arc::new(config));

            let listener = TcpListener::bind(&https_address).await?;
            let https_server = async {
                loop {
                    let (stream, _) = listener.accept().await?;
                    let acceptor = tls_acceptor.clone();
                    let routes = routes.clone();
                    tokio::spawn(async move {
                        match acceptor.accept(stream).await {
                            Ok(stream) => {
                                let service = warp::service(routes);
                                if let Err(e) = Http::new().serve_connection(stream, service).await {
                                    // Log the error instead of panicking
                                    shinkai_log(
                                        ShinkaiLogOption::Api,
                                        ShinkaiLogLevel::Error,
                                        &format!("Error serving connection: {:?}", e),
                                    );
                                }
                            }
                            Err(e) => {
                                // Log the TLS handshake error
                                shinkai_log(
                                    ShinkaiLogOption::Api,
                                    ShinkaiLogLevel::Error,
                                    &format!("TLS handshake failed: {:?}", e),
                                );
                            }
                        }
                    });
                }
                // This is unreachable because the server is running indefinitely
                // But the compiler needs it to return something
                #[allow(unreachable_code)]
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            };

            // Run both servers concurrently
            tokio::try_join!(http_server, https_server)?;
        }
    } else {
        // Only run HTTP server
        http_server.await?;
    }

    Ok(())
}

pub async fn handle_node_command<T, U, V>(
    node_commands_sender: Sender<NodeCommand>,
    message: V,
    command: T,
) -> Result<impl warp::Reply, warp::reject::Rejection>
where
    T: FnOnce(Sender<NodeCommand>, V, Sender<Result<U, APIError>>) -> NodeCommand,
    U: Serialize,
    V: Serialize,
{
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .clone()
        .send(command(node_commands_sender, message, res_sender))
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(message) => Ok(warp::reply::with_status(
            warp::reply::json(&json!({"status": "success", "data": message})),
            StatusCode::OK,
        )),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&json!({"status": "error", "error": error.message})),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(api_error) = err.find::<APIError>() {
        let json = warp::reply::json(api_error);
        Ok(warp::reply::with_status(
            json,
            StatusCode::from_u16(api_error.code).unwrap(),
        ))
    } else if err.is_not_found() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Please check your URL.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::NOT_FOUND))
    } else if let Some(body_err) = err.find::<warp::filters::body::BodyDeserializeError>() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::BAD_REQUEST,
            "Invalid Body",
            &format!("Deserialization error: {}", body_err),
        ));
        Ok(warp::reply::with_status(json, StatusCode::BAD_REQUEST))
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed",
            "Please check your request method.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::METHOD_NOT_ALLOWED))
    } else if err.find::<warp::reject::PayloadTooLarge>().is_some() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Payload Too Large",
            "The request payload is too large.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::PAYLOAD_TOO_LARGE))
    } else if err.find::<warp::reject::InvalidQuery>().is_some() {
        let json = warp::reply::json(&APIError::new(
            StatusCode::BAD_REQUEST,
            "Invalid Query",
            "The request query string is invalid.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::BAD_REQUEST))
    } else {
        // Unexpected error, we don't want to expose anything to the user.
        let json = warp::reply::json(&APIError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "An unexpected error occurred. Please try again.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
    }
}
