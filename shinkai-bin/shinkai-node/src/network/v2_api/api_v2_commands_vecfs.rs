use std::{collections::HashMap, sync::Arc};

use async_channel::Sender;
use base64::Engine;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde_json::Value;

use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_fs::shinkai_file_manager::{FileProcessingMode, ShinkaiFileManager};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::{
    schemas::shinkai_fs::ShinkaiFileChunkCollection,
    shinkai_message::shinkai_message_schemas::{
        APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem,
        APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveSourceFile,
        APIVecFsSearchItems,
    },
    shinkai_utils::shinkai_path::ShinkaiPath,
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;

use crate::{
    managers::IdentityManager,
    network::{node_error::NodeError, Node},
};

impl Node {
    pub async fn v2_api_vec_fs_retrieve_path_simplified_json(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsRetrievePathSimplifiedJson,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let vr_path = ShinkaiPath::from_string(input_payload.path);

        // Use list_directory_contents_with_depth to get directory contents with depth 1
        let directory_contents = ShinkaiFileManager::list_directory_contents_with_depth(vr_path, &db, 1);

        if let Err(e) = directory_contents {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to retrieve directory contents: {}", e),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Convert directory contents to JSON
        let json_contents = serde_json::to_value(directory_contents.unwrap()).map_err(|e| NodeError::from(e))?;

        // Send the directory contents as a response
        let _ = res.send(Ok(json_contents)).await.map_err(|_| ());
        Ok(())
    }

    pub async fn v2_create_folder(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsCreateFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Check if the base path exists
        let base_path = ShinkaiPath::from_string(input_payload.path.clone());
        if !base_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Base path does not exist: {}", input_payload.path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Create the full path by appending folder_name to the path
        let full_path_str = if input_payload.path == "/" {
            format!("/{}", input_payload.folder_name)
        } else {
            format!("{}/{}", input_payload.path, input_payload.folder_name)
        };
        let full_path = ShinkaiPath::from_string(full_path_str);

        // Check if the full path already exists
        if full_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!(
                    "Path already exists: {}/{}",
                    input_payload.path, input_payload.folder_name
                ),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Create the folder using ShinkaiFileManager
        match ShinkaiFileManager::create_folder(full_path) {
            Ok(_) => {
                let _ = res.send(Ok("Folder created successfully".to_string())).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create folder: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_move_item(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsMoveItem,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert origin and destination paths
        let origin_path = ShinkaiPath::from_string(input_payload.origin_path.clone());
        if !origin_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Origin path does not exist: {}", input_payload.origin_path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let destination_path = ShinkaiPath::from_string(input_payload.destination_path.clone());
        if destination_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Destination path already exists: {}", input_payload.destination_path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Move the file using ShinkaiFileManager
        match ShinkaiFileManager::move_file(origin_path, destination_path, &db) {
            Ok(_) => {
                let success_message = format!("Item moved successfully to {}", input_payload.destination_path);
                let _ = res.send(Ok(success_message)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to move item: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_copy_item(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsCopyItem,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert origin and destination paths
        let origin_path = ShinkaiPath::from_string(input_payload.origin_path.clone());
        if !origin_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Origin path does not exist: {}", input_payload.origin_path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let destination_path = ShinkaiPath::from_string(input_payload.destination_path.clone());

        // Copy the file using ShinkaiFileManager
        match ShinkaiFileManager::copy_file(origin_path, destination_path) {
            Ok(_) => {
                let success_message = format!("Item copied successfully to {}", input_payload.destination_path);
                let _ = res.send(Ok(success_message)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to copy item: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_move_folder(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsMoveFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert origin and destination paths
        let origin_path = ShinkaiPath::from_string(input_payload.origin_path.clone());
        if !origin_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Origin path does not exist: {}", input_payload.origin_path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let destination_path = ShinkaiPath::from_string(input_payload.destination_path.clone());
        if destination_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Destination path already exists: {}", input_payload.destination_path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Move the folder using ShinkaiFileManager
        match ShinkaiFileManager::move_folder(origin_path, destination_path, &db) {
            Ok(_) => {
                let success_message = format!("Folder moved successfully to {}", input_payload.destination_path);
                let _ = res.send(Ok(success_message)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to move folder: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_copy_folder(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsCopyFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        unimplemented!();

        // let requester_name = match identity_manager.lock().await.get_main_identity() {
        //     Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
        //     _ => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: "Wrong identity type. Expected Standard identity.".to_string(),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let origin_path = match ShinkaiPath::from_string(&input_payload.origin_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: format!("Failed to convert origin path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let destination_path = match ShinkaiPath::from_string(&input_payload.destination_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: format!("Failed to convert destination path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let writer = match vector_fs
        //     .new_writer(requester_name.clone(), origin_path, requester_name.clone())
        //     .await
        // {
        //     Ok(writer) => writer,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create writer: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // match vector_fs.copy_folder(&writer, destination_path).await {
        //     Ok(_) => {
        //         let success_message = format!("Folder copied successfully to {}", input_payload.destination_path);
        //         let _ = res.send(Ok(success_message)).await.map_err(|_| ());
        //         Ok(())
        //     }
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to copy folder: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn v2_delete_folder(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsDeleteFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert the path to ShinkaiPath
        let folder_path = ShinkaiPath::from_string(input_payload.path.clone());
        if !folder_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Folder path does not exist: {}", input_payload.path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Delete the folder using ShinkaiFileManager
        match ShinkaiFileManager::remove_folder(folder_path) {
            Ok(_) => {
                let success_message = format!("Folder successfully deleted: {}", input_payload.path);
                let _ = res.send(Ok(success_message)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to delete folder: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_delete_item(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsDeleteItem,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert the path to ShinkaiPath
        let item_path = ShinkaiPath::from_string(input_payload.path.clone());
        if !item_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("File path does not exist: {}", input_payload.path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Ensure the path is a file
        if !item_path.is_file() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Path is not a file: {}", input_payload.path),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Delete the file using ShinkaiFileManager
        match ShinkaiFileManager::remove_file(item_path, &db) {
            Ok(_) => {
                let success_message = format!("File successfully deleted: {}", input_payload.path);
                let _ = res.send(Ok(success_message)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to delete file: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_search_items(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsSearchItems,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Determine the search path
        let search_path_str = input_payload.path.as_deref().unwrap_or("/").to_string();
        let search_path = ShinkaiPath::from_string(search_path_str.clone());

        // Check if the search path exists
        if !search_path.exists() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Search path does not exist: {}", search_path_str),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let mut parsed_file_ids = Vec::new();
        let mut paths_map = HashMap::new();

        let query_embedding = match embedding_generator
            .generate_embedding_default(&input_payload.search)
            .await
        {
            Ok(embedding) => embedding,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to generate query embedding: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Retrieve files in the specified path
        let search_prefix = search_path.relative_path();
        match db.get_parsed_files_by_prefix(&search_prefix) {
            Ok(parsed_files) => {
                for parsed_file in parsed_files {
                    parsed_file_ids.push(parsed_file.id.unwrap());
                    paths_map.insert(
                        parsed_file.id.unwrap(),
                        ShinkaiPath::from_string(parsed_file.relative_path.clone()),
                    );
                }
            }
            Err(e) => {
                // Handle the error, e.g., log it or send an error response
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get parsed files: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        }

        // Perform a vector search on all parsed files
        let search_results = match db.search_chunks(
            &parsed_file_ids,
            query_embedding,
            input_payload.max_results.unwrap_or(100) as usize,
        ) {
            Ok(results) => results,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to search chunks: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let results = ShinkaiFileChunkCollection {
            chunks: search_results.into_iter().map(|(chunk, _)| chunk).collect(),
            paths: Some(paths_map),
        };

        // Convert results to JSON
        let json_results = match serde_json::to_value(results) {
            Ok(json_results) => json_results,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert results to JSON: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Send the search results as a response
        let _ = res.send(Ok(json_results)).await.map_err(|_| ());
        Ok(())
    }

    pub async fn v2_retrieve_vector_resource(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        path: String,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        unimplemented!();

        // let requester_name = match identity_manager.lock().await.get_main_identity() {
        //     Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
        //     _ => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: "Wrong identity type. Expected Standard identity.".to_string(),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let vr_path = match ShinkaiPath::from_string(&path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: format!("Failed to convert path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let reader = match vector_fs
        //     .new_reader(requester_name.clone(), vr_path, requester_name.clone())
        //     .await
        // {
        //     Ok(reader) => reader,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create reader: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let result = vector_fs.retrieve_vector_resource(&reader).await;

        // match result {
        //     Ok(result_value) => match result_value.to_json_value() {
        //         Ok(json_value) => {
        //             let _ = res.send(Ok(json_value)).await.map_err(|_| ());
        //             Ok(())
        //         }
        //         Err(e) => {
        //             let api_error = APIError {
        //                 code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //                 error: "Internal Server Error".to_string(),
        //                 message: format!("Failed to convert result to JSON: {}", e),
        //             };
        //             let _ = res.send(Err(api_error)).await;
        //             Ok(())
        //         }
        //     },
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to retrieve vector resource: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn v2_upload_file_to_folder(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        bearer: String,
        filename: String,
        file: Vec<u8>,
        path: String,
        _file_datetime: Option<DateTime<Utc>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Construct the full path for the file
        let full_path_str = if path == "/" {
            format!("/{}", filename)
        } else {
            format!("{}/{}", path, filename)
        };
        let full_path = ShinkaiPath::from_string(full_path_str.clone());

        // Save and process the file
        match ShinkaiFileManager::save_and_process_file(
            full_path.clone(),
            file,
            &db,
            FileProcessingMode::Auto,
            &*embedding_generator,
        )
        .await
        {
            Ok(_) => {
                let success_message = format!("File uploaded and processed successfully: {}", full_path_str);
                let _ = res.send(Ok(serde_json::json!({ "message": success_message }))).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to upload and process file: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_retrieve_file(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsRetrieveSourceFile,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Convert the input path to a ShinkaiPath
        let vr_path = ShinkaiPath::from_string(input_payload.path.clone());

        // Read the file content
        let file_content = match std::fs::read(vr_path.as_path()) {
            Ok(content) => content,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to read file content: {:?}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Encode the file content in base64
        let encoded_file_content = base64::engine::general_purpose::STANDARD.encode(&file_content);

        // Send the encoded file content as a response
        let _ = res.send(Ok(encoded_file_content)).await.map_err(|_| ());
        Ok(())
    }

    pub async fn v2_upload_file_to_job(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        bearer: String,
        job_id: String,
        filename: String,
        file: Vec<u8>,
        _file_datetime: Option<DateTime<Utc>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Save and process the file with the job ID
        match ShinkaiFileManager::save_and_process_file_with_jobid(
            &job_id,
            filename.clone(),
            file,
            &db,
            FileProcessingMode::Auto,
            &*embedding_generator,
        )
        .await
        {
            Ok(response) => {
                let success_message = format!(
                    "File uploaded and processed successfully for job {}: {}",
                    job_id, filename
                );
                let _ = res
                    .send(Ok(
                        serde_json::json!({ "message": success_message, "filename": response.filename() }),
                    ))
                    .await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to upload and process file for job {}: {:?}", job_id, e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn v2_api_vec_fs_retrieve_files_for_job(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        job_id: String,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve files for the given job_id using ShinkaiFileManager
        let files_result = ShinkaiFileManager::get_all_files_and_folders_for_job(&job_id, &db);

        if let Err(e) = files_result {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: format!("Failed to retrieve files for job_id {}: {}", job_id, e),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        // Convert the files information to JSON
        let json_files = serde_json::to_value(files_result.unwrap()).map_err(|e| NodeError::from(e))?;

        // Send the files information as a response
        let _ = res.send(Ok(json_files)).await.map_err(|_| ());
        Ok(())
    }

    pub async fn v2_api_vec_fs_get_folder_name_for_job(
        db: Arc<SqliteManager>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        job_id: String,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Retrieve the folder name for the given job_id
        let folder_name_result = db.get_job_folder_name(&job_id);

        match folder_name_result {
            Ok(folder_name) => {
                let folder_name_json = serde_json::json!({
                    "folder_name": folder_name.relative_path().to_string()
                });
                let _ = res.send(Ok(folder_name_json)).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve folder name for job_id {}: {}", job_id, e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
