use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use warp::{http::StatusCode, Filter};

use crate::api::api_handlers;

const MAX_CONTENT_LENGTH: u64 = 1024 * 1024 * 200; // 200MB

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

    let pdf_extract_to_text_groups = warp::path!("v1" / "pdf" / "extract-to-text-groups")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH)) // 200MB
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::pdf_extract_to_text_groups_handler(form));

    let vrkai_generate_from_file = warp::path!("v1" / "vrkai" / "generate-from-file")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH)) // 200MB
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrkai_generate_from_file_handler(form));

    let vrpack_generate_from_files = warp::path!("v1" / "vrpack" / "generate-from-files")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH)) // 200MB
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_generate_from_files_handler(form));

    let vrpack_generate_from_vrkais = warp::path!("v1" / "vrpack" / "generate-from-vrkais")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH)) // 200MB
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_generate_from_vrkais_handler(form));

    let routes = pdf_extract_to_text_groups
        .or(vrkai_generate_from_file)
        .or(vrpack_generate_from_files)
        .or(vrpack_generate_from_vrkais)
        .recover(handle_rejection);

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
