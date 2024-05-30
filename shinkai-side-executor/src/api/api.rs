use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use warp::{http::StatusCode, Filter};

use crate::api::api_handlers;

#[derive(Serialize, Debug, Clone)]
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

pub async fn run_api(address: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting server at: {:?}", address);

    let try_bind = TcpListener::bind(&address).await;

    let extract_json_to_text_groups = warp::path!("v1" / "extract_json_to_text_groups" / u64)
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 1024 * 200)) // 200MB
        .and(warp::multipart::form().max_length(1024 * 1024 * 200))
        .and_then(move |max_node_text_size: u64, form: warp::multipart::FormData| {
            api_handlers::post_extract_json_to_text_groups_handler(max_node_text_size, form)
        });

    let routes = extract_json_to_text_groups.recover(handle_rejection);

    match try_bind {
        Ok(_) => {
            drop(try_bind);
            warp::serve(routes).run(address).await;
            Ok(())
        }
        Err(e) => Err(Box::new(e)),
    }
}

async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, warp::Rejection> {
    eprintln!("API Error: {:?}", err);
    if let Some(api_error) = err.find::<APIError>() {
        let json = warp::reply::json(api_error);
        Ok(warp::reply::with_status(
            json,
            StatusCode::from_u16(api_error.code).unwrap(),
        ))
    } else {
        let json = warp::reply::json(&APIError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "An unexpected error occurred. Please try again.",
        ));
        Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
    }
}
