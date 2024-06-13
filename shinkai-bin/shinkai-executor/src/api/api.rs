use serde::Serialize;
use std::{io::Write, net::SocketAddr};
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

    // PDF
    let pdf_extract_to_text_groups = warp::path!("v1" / "pdf" / "extract-to-text-groups")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::pdf_extract_to_text_groups_handler(form));

    // VRKai
    let vrkai_generate_from_file = warp::path!("v1" / "vrkai" / "generate-from-file")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrkai_generate_from_file_handler(form));

    let vrkai_vector_search = warp::path!("v1" / "vrkai" / "vector-search")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrkai_vector_search_handler(form));

    let vrkai_view_contents = warp::path!("v1" / "vrkai" / "view-contents")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrkai_view_contents_handler(form));

    // VRPack
    let vrpack_generate_from_files = warp::path!("v1" / "vrpack" / "generate-from-files")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_generate_from_files_handler(form));

    let vrpack_generate_from_vrkais = warp::path!("v1" / "vrpack" / "generate-from-vrkais")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_generate_from_vrkais_handler(form));

    let vrpack_add_vrkais = warp::path!("v1" / "vrpack" / "add-vrkais")
        .and(warp::put())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_add_vrkais_handler(form));

    let vrpack_add_folder = warp::path!("v1" / "vrpack" / "add-folder")
        .and(warp::put())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_add_folder_handler(form));

    let vrpack_vector_search = warp::path!("v1" / "vrpack" / "vector-search")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_vector_search_handler(form));

    let vrpack_view_contents = warp::path!("v1" / "vrpack" / "view-contents")
        .and(warp::post())
        .and(warp::body::content_length_limit(MAX_CONTENT_LENGTH))
        .and(warp::multipart::form().max_length(MAX_CONTENT_LENGTH))
        .and_then(move |form: warp::multipart::FormData| api_handlers::vrpack_view_contents_handler(form));

    let routes = pdf_extract_to_text_groups
        .or(vrkai_generate_from_file)
        .or(vrkai_vector_search)
        .or(vrkai_view_contents)
        .or(vrpack_generate_from_files)
        .or(vrpack_generate_from_vrkais)
        .or(vrpack_add_vrkais)
        .or(vrpack_add_folder)
        .or(vrpack_vector_search)
        .or(vrpack_view_contents)
        .recover(handle_rejection);

    match try_bind {
        Ok(_) => {
            match check_and_download_dependencies().await {
                Ok(_) => {}
                Err(e) => eprintln!("Error downloading ocrs models: {:?}", e),
            }

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

async fn check_and_download_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::fs::create_dir("ocrs");

    let ocrs_models_url = "https://ocrs-models.s3-accelerate.amazonaws.com/";
    let detection_model = "text-detection.rten";
    let recognition_model = "text-recognition.rten";

    if !std::path::Path::new(detection_model).exists() {
        println!("Downloading OCRS model {}", detection_model);

        let client = reqwest::Client::new();
        let file_data = client
            .get(format!("{}{}", ocrs_models_url, detection_model))
            .send()
            .await?
            .bytes()
            .await?;

        let mut file = std::fs::File::create(format!("ocrs/{}", detection_model))?;
        file.write_all(&file_data)?;
    }

    if !std::path::Path::new(recognition_model).exists() {
        println!("Downloading OCRS model {}", recognition_model);

        let client = reqwest::Client::new();
        let file_data = client
            .get(format!("{}{}", ocrs_models_url, recognition_model))
            .send()
            .await?
            .bytes()
            .await?;

        let mut file = std::fs::File::create(format!("ocrs/{}", recognition_model))?;
        file.write_all(&file_data)?;
    }

    Ok(())
}
