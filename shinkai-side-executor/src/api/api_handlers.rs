use futures::StreamExt;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use warp::Buf;

use crate::{api::APIError, file_parser::FileParser};

pub async fn post_extract_json_to_text_groups_handler(
    max_node_text_size: u64,
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let mut stream = Box::pin(form.filter_map(|part_result| async move {
        if let Ok(part) = part_result {
            println!("Received part: {:?}", part);

            if let Some(filename) = part.filename() {
                let filename = filename.to_string();
                let stream = part
                    .stream()
                    .map(|res| res.map(|mut buf| buf.copy_to_bytes(buf.remaining()).to_vec()));
                return Some((filename, stream));
            }
        }
        None
    }));

    if let Some((filename, mut file_stream)) = stream.next().await {
        println!("Processing file: {:?}", filename);

        let mut file_buffer = Vec::new();
        while let Some(Ok(node)) = file_stream.next().await {
            file_buffer.extend(node);
        }

        match FileParser::process_into_text_groups(&filename, file_buffer, max_node_text_size).await {
            Ok(text_groups) => Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&text_groups),
                warp::http::StatusCode::OK,
            ))),
            Err(error) => Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&error.to_string()),
                warp::http::StatusCode::BAD_REQUEST,
            ))),
        }
    } else {
        Err(warp::reject::reject())
    }
}

pub async fn vrkai_process_file_into_resource_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let mut stream = Box::pin(form.filter_map(|part_result| async move {
        if let Ok(part) = part_result {
            println!("Received part: {:?}", part);

            let part_name = part.name().to_string();
            let file_name = part.filename().map(|s| s.to_string());

            let stream = part
                .stream()
                .map(|res| res.map(|mut buf| buf.copy_to_bytes(buf.remaining()).to_vec()));

            if part_name == "generator" {
                return Some((part_name, stream));
            }

            if let Some(file_name) = file_name {
                return Some((file_name, stream));
            }
        }
        None
    }));

    let mut filename = "".to_string();
    let mut file_buffer = Vec::new();
    let mut generator = RemoteEmbeddingGenerator::new_default();

    while let Some((part_name, mut part_stream)) = stream.next().await {
        println!("Processing file: {:?}", part_name);

        let mut part_data = Vec::new();
        while let Some(Ok(node)) = part_stream.next().await {
            part_data.extend(node);
        }

        if part_name == "generator" {
            generator = serde_json::from_slice::<RemoteEmbeddingGenerator>(&part_data)
                .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;
        } else {
            filename = part_name;
            file_buffer = part_data;
        }
    }

    match FileParser::process_into_resource(
        &filename,
        file_buffer,
        generator.model_type().max_input_token_count() as u64,
        &generator,
    )
    .await
    {
        Ok(resource) => Ok(Box::new(warp::reply::json(&resource.to_vrkai()))),
        Err(error) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&error.to_string()),
            warp::http::StatusCode::BAD_REQUEST,
        ))),
    }
}
