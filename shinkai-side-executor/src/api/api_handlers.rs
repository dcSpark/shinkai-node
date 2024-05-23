use futures::StreamExt;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use warp::{Buf, Filter};

use crate::pdf_parser;

pub async fn post_extract_json_to_text_groups_handler(
    max_node_text_size: u64,
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    // TODO: Add custom errors

    let mut stream = Box::pin(form.filter_map(|part_result| async move {
        if let Ok(part) = part_result {
            println!("Received file: {:?}", part);

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

        let mut file_data = Vec::new();
        while let Some(Ok(node)) = file_stream.next().await {
            file_data.extend(node);
        }

        let file_extension = filename.split('.').last();
        match file_extension {
            Some("pdf") => {
                let pdf_parser = pdf_parser::PDFParser::new().map_err(|_| warp::reject::reject())?;
                let result = pdf_parser.process_pdf_file(file_data, max_node_text_size);

                match result {
                    Ok(text_groups) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&text_groups),
                        warp::http::StatusCode::OK,
                    ))),
                    Err(error) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&error.to_string()),
                        warp::http::StatusCode::BAD_REQUEST,
                    ))),
                }
            }
            _ => return Err(warp::reject::reject()),
        }
    } else {
        Err(warp::reject::reject())
    }
}

pub async fn run_api(address: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting server at: {:?}", address);

    let try_bind = TcpListener::bind(&address).await;

    let extract_json_to_text_groups = warp::path!("v1" / "extract_json_to_text_groups" / u64)
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 1024 * 200)) // 200MB
        .and(warp::multipart::form().max_length(1024 * 1024 * 200))
        .and_then(move |max_node_text_size: u64, form: warp::multipart::FormData| {
            post_extract_json_to_text_groups_handler(max_node_text_size, form)
        });

    let routes = extract_json_to_text_groups;

    match try_bind {
        Ok(_) => {
            drop(try_bind);
            warp::serve(routes).run(address).await;
            Ok(())
        }
        Err(e) => Err(Box::new(e)),
    }
}
