use futures::StreamExt;
use shinkai_vector_resources::{
    embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator},
    model_type::EmbeddingModelType,
};
use warp::Buf;

use crate::{api::APIError, file_stream_parser::FileStreamParser};

const PART_KEY_EMBEDDING_MODEL: &str = "embedding_model";
const PART_KEY_EMBEDDING_GEN_URL: &str = "embedding_gen_url";
const PART_KEY_EMBEDDING_GEN_KEY: &str = "embedding_gen_key";
const PART_KEY_MAX_NODE_TEXT_SIZE: &str = "max_node_text_size";

pub async fn pdf_extract_to_text_groups_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let mut filename = "".to_string();
    let mut file_buffer = Vec::new();
    let mut max_node_text_size = 400;

    let mut stream = Box::pin(form.filter_map(|part_result| async move {
        if let Ok(part) = part_result {
            println!("Received part: {:?}", part);

            let part_name = part.name().to_string();
            let file_name = part.filename().map(|s| s.to_string());

            let stream = part
                .stream()
                .map(|res| res.map(|mut buf| buf.copy_to_bytes(buf.remaining()).to_vec()));

            if part_name == PART_KEY_MAX_NODE_TEXT_SIZE {
                return Some((part_name, stream));
            }

            if let Some(file_name) = file_name {
                return Some((file_name, stream));
            }
        }
        None
    }));

    while let Some((part_name, mut part_stream)) = stream.next().await {
        println!("Processing file: {:?}", part_name);

        let mut part_data = Vec::new();
        while let Some(Ok(node)) = part_stream.next().await {
            part_data.extend(node);
        }

        match part_name.as_str() {
            PART_KEY_MAX_NODE_TEXT_SIZE => {
                if let Ok(size) = String::from_utf8(part_data).unwrap_or_default().parse::<u64>() {
                    max_node_text_size = size;
                }
            }
            _ => {
                filename = part_name;
                file_buffer = part_data;
            }
        }
    }

    let file_extension = filename.split('.').last();
    match file_extension {
        Some("pdf") => match FileStreamParser::generate_text_groups(&filename, file_buffer, max_node_text_size).await {
            Ok(text_groups) => Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&text_groups),
                warp::http::StatusCode::OK,
            ))),
            Err(error) => Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&error.to_string()),
                warp::http::StatusCode::BAD_REQUEST,
            ))),
        },
        _ => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&"File does not have PDF extension."),
            warp::http::StatusCode::BAD_REQUEST,
        ))),
    }
}

pub async fn vrkai_generate_from_file_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let mut filename = "".to_string();
    let mut file_buffer = Vec::new();

    let mut embedding_model = "".to_string();
    let mut embedding_gen_url = "".to_string();
    let mut embedding_gen_key: Option<String> = None;

    let mut stream = Box::pin(form.filter_map(|part_result| async move {
        if let Ok(part) = part_result {
            println!("Received part: {:?}", part);

            let part_name = part.name().to_string();
            let file_name = part.filename().map(|s| s.to_string());

            let stream = part
                .stream()
                .map(|res| res.map(|mut buf| buf.copy_to_bytes(buf.remaining()).to_vec()));

            if [
                PART_KEY_EMBEDDING_MODEL.to_string(),
                PART_KEY_EMBEDDING_GEN_URL.to_string(),
                PART_KEY_EMBEDDING_GEN_KEY.to_string(),
            ]
            .contains(&part_name)
            {
                return Some((part_name, stream));
            }

            if let Some(file_name) = file_name {
                return Some((file_name, stream));
            }
        }
        None
    }));

    while let Some((part_name, mut part_stream)) = stream.next().await {
        println!("Processing file: {:?}", part_name);

        let mut part_data = Vec::new();
        while let Some(Ok(node)) = part_stream.next().await {
            part_data.extend(node);
        }

        match part_name.as_str() {
            PART_KEY_EMBEDDING_MODEL => embedding_model = String::from_utf8(part_data).unwrap_or_default(),
            PART_KEY_EMBEDDING_GEN_URL => embedding_gen_url = String::from_utf8(part_data).unwrap_or_default(),
            PART_KEY_EMBEDDING_GEN_KEY => embedding_gen_key = Some(String::from_utf8(part_data).unwrap_or_default()),
            _ => {
                filename = part_name;
                file_buffer = part_data;
            }
        }
    }

    let generator = RemoteEmbeddingGenerator::new(
        EmbeddingModelType::from_string(&embedding_model)
            .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?,
        &embedding_gen_url,
        embedding_gen_key,
    );

    match FileStreamParser::generate_vrkai(
        &filename,
        file_buffer,
        generator.model_type().max_input_token_count() as u64,
        &generator,
    )
    .await
    {
        Ok(vrkai) => {
            let encoded_vrkai = vrkai
                .encode_as_base64()
                .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

            Ok(Box::new(warp::reply::with_status(
                encoded_vrkai,
                warp::http::StatusCode::OK,
            )))
        }
        Err(error) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&error.to_string()),
            warp::http::StatusCode::BAD_REQUEST,
        ))),
    }
}
