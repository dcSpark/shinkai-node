use futures::StreamExt;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::{
    embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator},
    model_type::EmbeddingModelType,
    vector_resource::{VRKai, VRPack, VRPath},
};
use std::collections::HashMap;
use warp::Buf;

use crate::{
    api::APIError,
    file_stream_parser::FileStreamParser,
    models::dto::{ConvertFromVRPack, VRPackContent},
};

const PART_KEY_EMBEDDING_MODEL: &str = "embedding_model";
const PART_KEY_EMBEDDING_GEN_URL: &str = "embedding_gen_url";
const PART_KEY_EMBEDDING_GEN_KEY: &str = "embedding_gen_key";
const PART_KEY_ENCODED_VRKAI: &str = "encoded_vrkai";
const PART_KEY_ENCODED_VRPACK: &str = "encoded_vrpack";
const PART_KEY_FOLDER_NAME: &str = "folder_name";
const PART_KEY_MAX_NODE_TEXT_SIZE: &str = "max_node_text_size";
const PART_KEY_NUM_OF_RESULTS: &str = "num_of_results";
const PART_KEY_NUM_OF_VRKAIS_TO_SEARCH_INTO: &str = "num_of_vrkais_to_search_into";
const PART_KEY_QUERY_STRING: &str = "query_string";
const PART_KEY_VRPATH: &str = "vrpath";
const PART_KEY_VRPACK_NAME: &str = "vrpack_name";

struct ParsedParts {
    parts: HashMap<String, Vec<String>>,
    files: HashMap<String, Vec<u8>>,
}

async fn process_form_data(form: warp::multipart::FormData, part_keys: Vec<String>) -> ParsedParts {
    let mut parsed_parts = ParsedParts {
        parts: HashMap::new(),
        files: HashMap::new(),
    };

    let part_keys = &part_keys;

    let mut stream = Box::pin(form.filter_map(|part_result| async move {
        if let Ok(part) = part_result {
            shinkai_log(
                ShinkaiLogOption::Executor,
                ShinkaiLogLevel::Debug,
                format!("Received part: {:?}", part).as_str(),
            );

            let part_name = part.name().to_string();
            let file_name = part.filename().map(|s| s.to_string());

            let stream = part
                .stream()
                .map(|res| res.map(|mut buf| buf.copy_to_bytes(buf.remaining()).to_vec()));

            if let Some(file_name) = file_name {
                return Some((file_name, stream, true));
            }

            if part_keys.contains(&part_name) {
                return Some((part_name, stream, false));
            }
        }
        None
    }));

    while let Some((part_name, mut part_stream, is_file)) = stream.next().await {
        shinkai_log(
            ShinkaiLogOption::Executor,
            ShinkaiLogLevel::Debug,
            format!("Processing part: {:?}", part_name).as_str(),
        );

        let mut part_data = Vec::new();
        while let Some(Ok(node)) = part_stream.next().await {
            part_data.extend(node);
        }

        if is_file {
            parsed_parts.files.insert(part_name, part_data);
        } else {
            parsed_parts
                .parts
                .entry(part_name)
                .or_insert_with(Vec::new)
                .push(String::from_utf8(part_data).unwrap_or_default());
        }
    }

    parsed_parts
}

pub async fn pdf_extract_to_text_groups_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(form, vec![PART_KEY_MAX_NODE_TEXT_SIZE.to_string()]).await;

    let (filename, file_buffer) = parsed_parts
        .files
        .into_iter()
        .next()
        .unwrap_or(("".to_string(), Vec::new()));

    let max_node_text_size = parsed_parts
        .parts
        .get(PART_KEY_MAX_NODE_TEXT_SIZE)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .parse::<u64>()
        .unwrap_or(400);

    let file_extension = filename.split('.').last();
    match file_extension {
        Some("pdf") => match FileStreamParser::generate_text_groups(&filename, file_buffer, max_node_text_size).await {
            Ok(text_groups) => Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&text_groups),
                warp::http::StatusCode::OK,
            ))),
            Err(error) => Err(warp::reject::custom(APIError::from(error.to_string()))),
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
    let parsed_parts = process_form_data(
        form,
        vec![
            PART_KEY_EMBEDDING_MODEL.to_string(),
            PART_KEY_EMBEDDING_GEN_URL.to_string(),
            PART_KEY_EMBEDDING_GEN_KEY.to_string(),
        ],
    )
    .await;

    let (filename, file_buffer) = parsed_parts
        .files
        .into_iter()
        .next()
        .unwrap_or(("".to_string(), Vec::new()));

    let mut generator = RemoteEmbeddingGenerator::new_default();
    if let Some(model_type) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_MODEL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.model_type = EmbeddingModelType::from_string(model_type)
            .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;
    }
    if let Some(api_url) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_URL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_url = api_url.to_owned();
    }
    if let Some(api_key) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_KEY)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_key = Some(api_key.to_owned());
    }

    match FileStreamParser::generate_vrkai(&filename, file_buffer, &generator).await {
        Ok(vrkai) => {
            let encoded_vrkai = vrkai
                .encode_as_base64()
                .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

            Ok(Box::new(warp::reply::with_status(
                encoded_vrkai,
                warp::http::StatusCode::OK,
            )))
        }
        Err(error) => Err(warp::reject::custom(APIError::from(error.to_string()))),
    }
}

pub async fn vrkai_vector_search_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(
        form,
        vec![
            PART_KEY_EMBEDDING_MODEL.to_string(),
            PART_KEY_EMBEDDING_GEN_URL.to_string(),
            PART_KEY_EMBEDDING_GEN_KEY.to_string(),
            PART_KEY_ENCODED_VRKAI.to_string(),
            PART_KEY_NUM_OF_RESULTS.to_string(),
            PART_KEY_QUERY_STRING.to_string(),
        ],
    )
    .await;

    let mut generator = RemoteEmbeddingGenerator::new_default();
    if let Some(model_type) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_MODEL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.model_type = EmbeddingModelType::from_string(model_type)
            .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;
    }
    if let Some(api_url) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_URL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_url = api_url.to_owned();
    }
    if let Some(api_key) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_KEY)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_key = Some(api_key.to_owned());
    }

    let encoded_vrkai = parsed_parts
        .parts
        .get(PART_KEY_ENCODED_VRKAI)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();
    let num_of_results = parsed_parts
        .parts
        .get(PART_KEY_NUM_OF_RESULTS)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .parse::<u64>()
        .unwrap_or(3);
    let query_string = parsed_parts
        .parts
        .get(PART_KEY_QUERY_STRING)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    match VRKai::from_base64(&encoded_vrkai) {
        Ok(vrkai) => {
            let query_embedding = generator
                .generate_embedding_default(&query_string)
                .await
                .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

            let results = vrkai.vector_search(query_embedding, num_of_results);

            Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&results),
                warp::http::StatusCode::OK,
            )))
        }
        Err(_) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&"Input is not a valid VRKai."),
            warp::http::StatusCode::BAD_REQUEST,
        ))),
    }
}

pub async fn vrkai_view_contents_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(form, vec![PART_KEY_ENCODED_VRKAI.to_string()]).await;

    let encoded_vrkai = parsed_parts
        .parts
        .get(PART_KEY_ENCODED_VRKAI)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    match VRKai::from_base64(&encoded_vrkai) {
        Ok(vrkai) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&vrkai),
            warp::http::StatusCode::OK,
        ))),
        Err(_) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&"Input is not a valid VRKai."),
            warp::http::StatusCode::BAD_REQUEST,
        ))),
    }
}

pub async fn vrpack_generate_from_files_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(
        form,
        vec![
            PART_KEY_EMBEDDING_MODEL.to_string(),
            PART_KEY_EMBEDDING_GEN_URL.to_string(),
            PART_KEY_EMBEDDING_GEN_KEY.to_string(),
            PART_KEY_VRPACK_NAME.to_string(),
        ],
    )
    .await;

    let mut generator = RemoteEmbeddingGenerator::new_default();
    if let Some(model_type) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_MODEL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.model_type = EmbeddingModelType::from_string(model_type)
            .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;
    }
    if let Some(api_url) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_URL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_url = api_url.to_owned();
    }
    if let Some(api_key) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_KEY)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_key = Some(api_key.to_owned());
    }

    let vrpack_name = parsed_parts
        .parts
        .get(PART_KEY_VRPACK_NAME)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    let vrpack = FileStreamParser::generate_vrpack_from_files(parsed_parts.files, &generator, &vrpack_name)
        .await
        .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

    let encoded_vrpack = vrpack
        .encode_as_base64()
        .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

    Ok(Box::new(warp::reply::with_status(
        encoded_vrpack,
        warp::http::StatusCode::OK,
    )))
}

pub async fn vrpack_generate_from_vrkais_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(form, vec![PART_KEY_VRPACK_NAME.to_string()]).await;

    let vrpack_name = parsed_parts
        .parts
        .get(PART_KEY_VRPACK_NAME)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    let files = parsed_parts.files.into_values().collect::<Vec<Vec<u8>>>();

    let vrpack = FileStreamParser::generate_vrpack_from_vrkais(files, &vrpack_name)
        .await
        .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

    let encoded_vrpack = vrpack
        .encode_as_base64()
        .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

    Ok(Box::new(warp::reply::with_status(
        encoded_vrpack,
        warp::http::StatusCode::OK,
    )))
}

pub async fn vrpack_add_vrkais_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(
        form,
        vec![
            PART_KEY_ENCODED_VRPACK.to_string(),
            PART_KEY_ENCODED_VRKAI.to_string(),
            PART_KEY_VRPATH.to_string(),
        ],
    )
    .await;

    let mut vrpath = VRPath::root();
    if let Some(path) = parsed_parts.parts.get(PART_KEY_VRPATH).unwrap_or(&Vec::new()).first() {
        vrpath = match VRPath::from_string(path) {
            Ok(result) => result,
            Err(_) => {
                return Ok(Box::new(warp::reply::with_status(
                    warp::reply::json(&"Input is not a valid VRPath."),
                    warp::http::StatusCode::BAD_REQUEST,
                )));
            }
        };
    }

    let encoded_vrpack = parsed_parts
        .parts
        .get(PART_KEY_ENCODED_VRPACK)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    let mut vrpack = match VRPack::from_base64(&encoded_vrpack) {
        Ok(result) => result,
        Err(_) => {
            return Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&"Input is not a valid VRPack."),
                warp::http::StatusCode::BAD_REQUEST,
            )));
        }
    };

    for encoded_vrkai in parsed_parts.parts.get(PART_KEY_ENCODED_VRKAI).unwrap_or(&Vec::new()) {
        match VRKai::from_base64(&encoded_vrkai) {
            Ok(vrkai) => {
                vrpack
                    .insert_vrkai(&vrkai, vrpath.clone(), true)
                    .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;
            }
            Err(_) => {
                return Ok(Box::new(warp::reply::with_status(
                    warp::reply::json(&"Input is not a valid VRKai."),
                    warp::http::StatusCode::BAD_REQUEST,
                )));
            }
        }
    }

    let encoded_vrpack = vrpack
        .encode_as_base64()
        .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

    Ok(Box::new(warp::reply::with_status(
        encoded_vrpack,
        warp::http::StatusCode::OK,
    )))
}

pub async fn vrpack_add_folder_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(
        form,
        vec![
            PART_KEY_ENCODED_VRPACK.to_string(),
            PART_KEY_FOLDER_NAME.to_string(),
            PART_KEY_VRPATH.to_string(),
        ],
    )
    .await;

    let mut vrpath = VRPath::root();
    if let Some(path) = parsed_parts.parts.get(PART_KEY_VRPATH).unwrap_or(&Vec::new()).first() {
        vrpath = match VRPath::from_string(path) {
            Ok(result) => result,
            Err(_) => {
                return Ok(Box::new(warp::reply::with_status(
                    warp::reply::json(&"Input is not a valid VRPath."),
                    warp::http::StatusCode::BAD_REQUEST,
                )));
            }
        };
    }

    let encoded_vrpack = parsed_parts
        .parts
        .get(PART_KEY_ENCODED_VRPACK)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    let mut vrpack = match VRPack::from_base64(&encoded_vrpack) {
        Ok(result) => result,
        Err(_) => {
            return Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&"Input is not a valid VRPack."),
                warp::http::StatusCode::BAD_REQUEST,
            )));
        }
    };

    let folder_name = parsed_parts
        .parts
        .get(PART_KEY_FOLDER_NAME)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    vrpack
        .create_folder(&folder_name, vrpath.clone())
        .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

    let encoded_vrpack = vrpack
        .encode_as_base64()
        .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

    Ok(Box::new(warp::reply::with_status(
        encoded_vrpack,
        warp::http::StatusCode::OK,
    )))
}

pub async fn vrpack_vector_search_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(
        form,
        vec![
            PART_KEY_EMBEDDING_MODEL.to_string(),
            PART_KEY_EMBEDDING_GEN_URL.to_string(),
            PART_KEY_EMBEDDING_GEN_KEY.to_string(),
            PART_KEY_ENCODED_VRPACK.to_string(),
            PART_KEY_NUM_OF_RESULTS.to_string(),
            PART_KEY_NUM_OF_VRKAIS_TO_SEARCH_INTO.to_string(),
            PART_KEY_QUERY_STRING.to_string(),
        ],
    )
    .await;

    let mut generator = RemoteEmbeddingGenerator::new_default();
    if let Some(model_type) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_MODEL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.model_type = EmbeddingModelType::from_string(model_type)
            .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;
    }
    if let Some(api_url) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_URL)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_url = api_url.to_owned();
    }
    if let Some(api_key) = parsed_parts
        .parts
        .get(PART_KEY_EMBEDDING_GEN_KEY)
        .unwrap_or(&Vec::new())
        .first()
    {
        generator.api_key = Some(api_key.to_owned());
    }

    let encoded_vrpack = parsed_parts
        .parts
        .get(PART_KEY_ENCODED_VRPACK)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();
    let num_of_results = parsed_parts
        .parts
        .get(PART_KEY_NUM_OF_RESULTS)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .parse::<u64>()
        .unwrap_or(3);
    let num_of_vrkais_to_search_into = parsed_parts
        .parts
        .get(PART_KEY_NUM_OF_VRKAIS_TO_SEARCH_INTO)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .parse::<u64>()
        .unwrap_or(50);
    let query_string = parsed_parts
        .parts
        .get(PART_KEY_QUERY_STRING)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    match VRPack::from_base64(&encoded_vrpack) {
        Ok(vrpack) => {
            let results = vrpack
                .dynamic_deep_vector_search(query_string, num_of_vrkais_to_search_into, num_of_results, generator)
                .await
                .map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

            Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&results),
                warp::http::StatusCode::OK,
            )))
        }
        Err(_) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&"Input is not a valid VRPack."),
            warp::http::StatusCode::BAD_REQUEST,
        ))),
    }
}

pub async fn vrpack_view_contents_handler(
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let parsed_parts = process_form_data(form, vec![PART_KEY_ENCODED_VRPACK.to_string()]).await;

    let encoded_vrpack = parsed_parts
        .parts
        .get(PART_KEY_ENCODED_VRPACK)
        .unwrap_or(&Vec::new())
        .first()
        .unwrap_or(&"".to_string())
        .to_owned();

    match VRPack::from_base64(&encoded_vrpack) {
        Ok(vrpack) => {
            let content =
                VRPackContent::convert_from(vrpack).map_err(|e| warp::reject::custom(APIError::from(e.to_string())))?;

            Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&content),
                warp::http::StatusCode::OK,
            )))
        }
        Err(_) => Ok(Box::new(warp::reply::with_status(
            warp::reply::json(&"Input is not a valid VRPack."),
            warp::http::StatusCode::BAD_REQUEST,
        ))),
    }
}
