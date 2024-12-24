use std::{env, fs, path::Path, sync::Arc};

use crate::{
    managers::IdentityManager,
    network::{node_error::NodeError, Node},
};
use async_channel::Sender;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::Value;

use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_fs::shinkai_file_manager::{FileProcessingMode, ShinkaiFileManager};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::{
    schemas::{identity::Identity, shinkai_name::ShinkaiName},
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIConvertFilesAndSaveToFolder, APIVecFSRetrieveVRObject, APIVecFSRetrieveVectorResource,
            APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem,
            APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
            APIVecFsRetrieveVectorSearchSimplifiedJson, APIVecFsSearchItems, MessageSchemaType,
        },
    },
    shinkai_utils::shinkai_path::ShinkaiPath,
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn validate_and_extract_payload<T: DeserializeOwned>(
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        schema_type: MessageSchemaType,
    ) -> Result<(T, ShinkaiName), APIError> {
        let validation_result = Self::validate_message(
            encryption_secret_key,
            identity_manager,
            &node_name,
            potentially_encrypted_msg,
            Some(schema_type),
        )
        .await;
        let (msg, identity) = match validation_result {
            Ok((msg, identity)) => (msg, identity),
            Err(api_error) => return Err(api_error),
        };

        let content = msg.get_message_content().map_err(|e| APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: format!("Failed to get message content: {}", e),
        })?;

        let input_payload = serde_json::from_str::<T>(&content).map_err(|e| APIError {
            code: StatusCode::BAD_REQUEST.as_u16(),
            error: "Bad Request".to_string(),
            message: format!("Failed to parse payload: {}", e),
        })?;

        let requester_name = match identity {
            Identity::Standard(std_identity) => std_identity.full_identity_name,
            _ => {
                return Err(APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                })
            }
        };

        Ok((input_payload, requester_name))
    }

    #[allow(clippy::too_many_arguments)]
    // Public function for simplified JSON
    pub async fn api_vec_fs_retrieve_path_simplified_json(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        Self::retrieve_path_json_common(
            // Pass parameters and false for is_minimal
            _db,
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            res,
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    // Public function for minimal JSON
    pub async fn api_vec_fs_retrieve_path_minimal_json(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        Self::retrieve_path_json_common(
            // Pass parameters and true for is_minimal
            _db,
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            res,
            true,
        )
        .await
    }

    // Private method to abstract common logic
    #[allow(clippy::too_many_arguments)]
    async fn retrieve_path_json_common(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
        _is_minimal: bool, // TODO: to remove
    ) -> Result<(), NodeError> {
        let (input_payload, _requester_name) =
            match Self::validate_and_extract_payload::<APIVecFsRetrievePathSimplifiedJson>(
                node_name,
                identity_manager,
                encryption_secret_key,
                potentially_encrypted_msg,
                MessageSchemaType::VecFsRetrievePathSimplifiedJson,
            )
            .await
            {
                Ok(data) => data,
                Err(api_error) => {
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

        let vr_path = ShinkaiPath::from_string(input_payload.path);

        // Use list_directory_contents to get directory contents
        let directory_contents = ShinkaiFileManager::list_directory_contents(vr_path, &db);

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

    pub async fn api_vec_fs_search_items(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsSearchItems>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsSearchItems,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let vr_path = match input_payload.path {
        //     Some(path) => match ShinkaiPath::from_string(&path) {
        //         Ok(path) => path,
        //         Err(e) => {
        //             let api_error = APIError {
        //                 code: StatusCode::BAD_REQUEST.as_u16(),
        //                 error: "Bad Request".to_string(),
        //                 message: format!("Failed to convert path to VRPath: {}", e),
        //             };
        //             let _ = res.send(Err(api_error)).await;
        //             return Ok(());
        //         }
        //     },
        //     None => VRPath::root(),
        // };
        // let reader = vector_fs
        //     .new_reader(requester_name.clone(), vr_path, requester_name.clone())
        //     .await;
        // let reader = match reader {
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

        // let max_resources_to_search = input_payload.max_files_to_scan.unwrap_or(100) as u64;
        // let max_results = input_payload.max_results.unwrap_or(100) as u64;

        // let query_embedding = vector_fs
        //     .generate_query_embedding_using_reader(input_payload.search, &reader)
        //     .await
        //     .unwrap();
        // let search_results = vector_fs
        //     .vector_search_fs_item(&reader, query_embedding, max_resources_to_search)
        //     .await
        //     .unwrap();

        // let results: Vec<String> = search_results
        //     .into_iter()
        //     .map(|res| res.path.to_string())
        //     .take(max_results as usize)
        //     .collect();

        // let _ = res.send(Ok(results)).await.map_err(|_| ());
        // Ok(())
    }

    // TODO: implement a vector search endpoint for finding FSItems (we'll need for the search UI in Visor for the FS) and one for the VRKai returned too
    pub async fn api_vec_fs_retrieve_vector_search_simplified_json(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<(String, Vec<String>, f32)>, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) =
            match Self::validate_and_extract_payload::<APIVecFsRetrieveVectorSearchSimplifiedJson>(
                node_name,
                identity_manager,
                encryption_secret_key,
                potentially_encrypted_msg,
                MessageSchemaType::VecFsRetrieveVectorSearchSimplifiedJson,
            )
            .await
            {
                Ok(data) => data,
                Err(api_error) => {
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

        let vr_path = match input_payload.path {
            Some(path) => ShinkaiPath::from_string(path),
            None => ShinkaiPath::from_str(""),
        };

        unimplemented!();

        // let max_resources_to_search = input_payload.max_files_to_scan.unwrap_or(100) as u64;
        // let max_results = input_payload.max_results.unwrap_or(100) as u64;

        // let search_results = match vector_fs
        //     .deep_vector_search(
        //         &reader,
        //         input_payload.search.clone(),
        //         max_resources_to_search,
        //         max_results,
        //         vec![],
        //     )
        //     .await
        // {
        //     Ok(results) => results,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to perform deep vector search: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // // TODO: Change path to be a single output string.
        // // - Also return the source metadata, potentially using the format output method
        // // that is used for showing search results to LLMs
        // let results: Vec<(String, Vec<String>, f32)> = search_results
        //     .into_iter()
        //     .map(|res| {
        //         let content = match res.resource_retrieved_node.node.get_text_content() {
        //             Ok(text) => text.to_string(),
        //             Err(_) => "".to_string(),
        //         };
        //         let path_ids = res.clone().fs_item_path().path_ids;
        //         let score = res.resource_retrieved_node.score;
        //         (content, path_ids, score)
        //     })
        //     .collect();

        // let _ = res.send(Ok(results)).await.map_err(|_| ());
        // Ok(())
    }

    pub async fn api_vec_fs_create_folder(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsCreateFolder>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsCreateFolder,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let vr_path = match ShinkaiPath::from_string(&input_payload.path) {
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

        // let writer = match vector_fs
        //     .new_writer(requester_name.clone(), vr_path, requester_name.clone())
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

        // match vector_fs.create_new_folder(&writer, &input_payload.folder_name).await {
        //     Ok(_) => {
        //         let success_message = format!("Folder '{}' created successfully.", input_payload.folder_name);
        //         let _ = res.send(Ok(success_message)).await.map_err(|_| ());
        //         Ok(())
        //     }
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create new folder: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn api_vec_fs_move_folder(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsMoveFolder>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsMoveFolder,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let folder_path = match ShinkaiPath::from_string(&input_payload.origin_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to convert item path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };
        // let destination_path = match ShinkaiPath::from_string(&input_payload.destination_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to convert destination path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let orig_writer = match vector_fs
        //     .new_writer(requester_name.clone(), folder_path, requester_name.clone())
        //     .await
        // {
        //     Ok(writer) => writer,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create writer for original folder: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // match vector_fs.move_folder(&orig_writer, destination_path).await {
        //     Ok(_) => {
        //         let success_message = format!("Folder moved successfully to {}", input_payload.destination_path);
        //         let _ = res.send(Ok(success_message)).await.map_err(|_| ());
        //         Ok(())
        //     }
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to move folder: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn api_vec_fs_copy_folder(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsCopyFolder>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsCopyFolder,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let folder_path = match ShinkaiPath::from_string(&input_payload.origin_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: format!("Failed to convert folder path to VRPath: {}", e),
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

        // let orig_writer = match vector_fs
        //     .new_writer(requester_name.clone(), folder_path, requester_name.clone())
        //     .await
        // {
        //     Ok(writer) => writer,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create writer for original folder: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // match vector_fs.copy_folder(&orig_writer, destination_path).await {
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

    pub async fn api_vec_fs_delete_item(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsDeleteItem>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsDeleteItem,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let item_path = match ShinkaiPath::from_string(&input_payload.path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: format!("Failed to convert item path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let orig_writer = match vector_fs
        //     .new_writer(requester_name.clone(), item_path, requester_name.clone())
        //     .await
        // {
        //     Ok(writer) => writer,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create writer for item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // match vector_fs.delete_item(&orig_writer).await {
        //     Ok(_) => {
        //         let success_message = format!("Item successfully deleted: {}", input_payload.path);
        //         let _ = res.send(Ok(success_message)).await.map_err(|_| ());
        //         Ok(())
        //     }
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to move item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn api_vec_fs_delete_folder(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsDeleteFolder>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsDeleteFolder,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let item_path = match ShinkaiPath::from_string(&input_payload.path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: format!("Failed to convert folder path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let orig_writer = match vector_fs
        //     .new_writer(requester_name.clone(), item_path, requester_name.clone())
        //     .await
        // {
        //     Ok(writer) => writer,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create writer for item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // match vector_fs.delete_folder(&orig_writer).await {
        //     Ok(_) => {
        //         let success_message = format!("Folder successfully deleted: {}", input_payload.path);
        //         let _ = res.send(Ok(success_message)).await.map_err(|_| ());
        //         Ok(())
        //     }
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to move item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn api_vec_fs_move_item(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsMoveItem>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsMoveItem,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let item_path = match ShinkaiPath::from_string(&input_payload.origin_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::BAD_REQUEST.as_u16(),
        //             error: "Bad Request".to_string(),
        //             message: format!("Failed to convert item path to VRPath: {}", e),
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

        // let orig_writer = match vector_fs
        //     .new_writer(requester_name.clone(), item_path, requester_name.clone())
        //     .await
        // {
        //     Ok(writer) => writer,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create writer for original item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // match vector_fs.move_item(&orig_writer, destination_path).await {
        //     Ok(_) => {
        //         let success_message = format!("Item moved successfully to {}", input_payload.destination_path);
        //         let _ = res.send(Ok(success_message)).await.map_err(|_| ());
        //         Ok(())
        //     }
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to move item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn api_vec_fs_copy_item(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFsCopyItem>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsCopyItem,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();

        // let item_path = match ShinkaiPath::from_string(&input_payload.origin_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to convert item path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };
        // let destination_path = match ShinkaiPath::from_string(&input_payload.destination_path) {
        //     Ok(path) => path,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to convert destination path to VRPath: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let orig_writer = match vector_fs
        //     .new_writer(requester_name.clone(), item_path, requester_name.clone())
        //     .await
        // {
        //     Ok(writer) => writer,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to create writer for original item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // match vector_fs.copy_item(&orig_writer, destination_path).await {
        //     Ok(_) => {
        //         let success_message = format!("Item copied successfully to {}", input_payload.destination_path);
        //         let _ = res.send(Ok(success_message)).await.map_err(|_| ());
        //         Ok(())
        //     }
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to copy item: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         Ok(())
        //     }
        // }
    }

    pub async fn api_vec_fs_retrieve_vector_resource(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) =
            match Self::validate_and_extract_payload::<APIVecFSRetrieveVectorResource>(
                node_name,
                identity_manager,
                encryption_secret_key,
                potentially_encrypted_msg,
                MessageSchemaType::VecFsRetrieveVectorResource,
            )
            .await
            {
                Ok(data) => data,
                Err(api_error) => {
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

        unimplemented!();
        // let vr_path = match ShinkaiPath::from_string(&input_payload.path) {
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
        // let reader = vector_fs
        //     .new_reader(requester_name.clone(), vr_path, requester_name.clone())
        //     .await;
        // let reader = match reader {
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
        // let result = match result {
        //     Ok(result) => result,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to retrieve vector resource: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let json_resp = match result.to_json_value() {
        //     Ok(result) => result,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to convert vector resource to json: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };
        // let _ = res.send(Ok(json_resp)).await.map_err(|_| ());
        // Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn api_convert_files_and_save_to_folder(
        db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<Value>, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) =
            match Self::validate_and_extract_payload::<APIConvertFilesAndSaveToFolder>(
                node_name,
                identity_manager,
                encryption_secret_key,
                potentially_encrypted_msg,
                MessageSchemaType::ConvertFilesAndSaveToFolder,
            )
            .await
            {
                Ok(data) => data,
                Err(api_error) => {
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };
        Self::process_and_save_files(db, input_payload, requester_name, embedding_generator, res).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_and_save_files(
        db: Arc<SqliteManager>,
        input_payload: APIConvertFilesAndSaveToFolder,
        requester_name: ShinkaiName,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        res: Sender<Result<Vec<Value>, APIError>>,
    ) -> Result<(), NodeError> {
        let destination_path = ShinkaiPath::from_string(input_payload.path);

        let files = {
            match db.get_all_files_from_inbox(input_payload.file_inbox.clone()) {
                Ok(files) => files,
                Err(err) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("{}", err),
                        }))
                        .await;
                    return Ok(());
                }
            }
        };

        unimplemented!();

        // type FileData = (String, Vec<u8>);
        // type FileDataVec = Vec<FileData>;

        // // Sort out the vrpacks from the rest
        // let (vr_packs, other_files): (FileDataVec, FileDataVec) =
        //     files.into_iter().partition(|(name, _)| name.ends_with(".vrpack"));

        // for (filename, data) in other_files {
        //     let file_path = destination_path.join(&filename);
        //     let result = ShinkaiFileManager::save_and_process_file(
        //         file_path,
        //         data,
        //         Path::new("/base/dir"), // Replace with actual base directory path
        //         &db,
        //         FileProcessingMode::Auto,
        //         &*embedding_generator,
        //     )
        //     .await;

        //     if let Err(e) = result {
        //         let _ = res
        //             .send(Err(APIError {
        //                 code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //                 error: "Internal Server Error".to_string(),
        //                 message: format!("Error processing file '{}': {}", filename, e),
        //             }))
        //             .await;
        //         return Ok(());
        //     }
        // }

        // // Handle vr_packs if necessary
        // // ...

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn retrieve_vr_kai(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFSRetrieveVRObject>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsRetrieveVRPack,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        unimplemented!();
        // let vr_path = match ShinkaiPath::from_string(&input_payload.path) {
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
        // let reader = vector_fs
        //     .new_reader(requester_name.clone(), vr_path, requester_name.clone())
        //     .await;
        // let reader = match reader {
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

        // let result = vector_fs.retrieve_vrkai(&reader).await;
        // let result = match result {
        //     Ok(result) => result,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to retrieve vector resource: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // if env::var("DEBUG_VRKAI").is_ok() {
        //     let debug_content = result.resource.resource_contents_by_hierarchy_to_string();
        //     let file_name = format!("tmp/{}.txt", input_payload.path.replace("/", "_"));
        //     let path = Path::new(&file_name);
        //     if let Some(parent) = path.parent() {
        //         fs::create_dir_all(parent).unwrap();
        //     }
        //     fs::write(path, debug_content).unwrap();
        // }

        // let json_resp = match result.encode_as_base64() {
        //     Ok(result) => result,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to convert vector resource to json: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };
        // let _ = res.send(Ok(json_resp)).await.map_err(|_| ());
        // Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn retrieve_vr_pack(
        _db: Arc<SqliteManager>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIVecFSRetrieveVRObject>(
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::VecFsRetrieveVRPack,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        unimplemented!();
        // let vr_path = match ShinkaiPath::from_string(&input_payload.path) {
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
        // let reader = vector_fs
        //     .new_reader(requester_name.clone(), vr_path, requester_name.clone())
        //     .await;
        // let reader = match reader {
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

        // let result = vector_fs.retrieve_vrpack(&reader).await;
        // let result = match result {
        //     Ok(result) => result,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to retrieve vector resource: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };

        // let resp = match result.encode_as_base64() {
        //     Ok(result) => result,
        //     Err(e) => {
        //         let api_error = APIError {
        //             code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        //             error: "Internal Server Error".to_string(),
        //             message: format!("Failed to convert vector resource to json: {}", e),
        //         };
        //         let _ = res.send(Err(api_error)).await;
        //         return Ok(());
        //     }
        // };
        // let _ = res.send(Ok(resp)).await.map_err(|_| ());
        // Ok(())
    }
}
