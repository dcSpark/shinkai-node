use futures::StreamExt;
use warp::Buf;

use crate::{api::APIError, pdf_parser};

pub async fn post_extract_json_to_text_groups_handler(
    max_node_text_size: u64,
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
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
                let pdf_parser =
                    pdf_parser::PDFParser::new().map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;
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
            _ => {
                return Err(warp::reject::custom(APIError::new(
                    warp::http::StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "File extension is not supported.",
                )))
            }
        }
    } else {
        Err(warp::reject::reject())
    }
}
