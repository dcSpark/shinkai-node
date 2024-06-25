use std::sync::Arc;

use super::{
    node_api::APIError, node_error::NodeError,
    subscription_manager::external_subscriber_manager::ExternalSubscriberManager, Node,
};
use crate::{
    db::ShinkaiDB, llm_provider::parsing_helper::ParsingHelper, managers::IdentityManager,
    network::subscription_manager::external_subscriber_manager::SharedFolderInfo, schemas::identity::Identity,
    vector_fs::vector_fs::VectorFS,
};
use async_channel::Sender;
use reqwest::StatusCode;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIConvertFilesAndSaveToFolder, APIVecFSRetrieveVRObject, APIVecFSRetrieveVectorResource,
            APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem,
            APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
            APIVecFsRetrieveVectorSearchSimplifiedJson, APIVecFsSearchItems, MessageSchemaType,
        },
    },
};
use shinkai_vector_resources::{
    embedding_generator::EmbeddingGenerator,
    file_parser::{file_parser::FileParser, unstructured_api::UnstructuredAPI},
    source::DistributionInfo,
    vector_resource::{VRPack, VRPath},
};
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
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        Self::retrieve_path_json_common(
            // Pass parameters and false for is_minimal
            _db,
            vector_fs,
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            ext_subscription_manager,
            res,
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    // Public function for minimal JSON
    pub async fn api_vec_fs_retrieve_path_minimal_json(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        Self::retrieve_path_json_common(
            // Pass parameters and true for is_minimal
            _db,
            vector_fs,
            node_name,
            identity_manager,
            encryption_secret_key,
            potentially_encrypted_msg,
            ext_subscription_manager,
            res,
            true,
        )
        .await
    }

    // Private method to abstract common logic
    #[allow(clippy::too_many_arguments)]
    async fn retrieve_path_json_common(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        res: Sender<Result<Value, APIError>>,
        is_minimal: bool, // Determines which JSON representation to retrieve
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) =
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

        let vr_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert path to VRPath: {}", e),
                };
                // Immediately send the error and return from the function
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let reader = match vector_fs
            .new_reader(requester_name.clone(), vr_path, requester_name.clone())
            .await
        {
            Ok(reader) => reader,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create reader: {}", e),
                };
                // Immediately send the error and return from the function
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let result = if is_minimal {
            vector_fs.retrieve_fs_path_minimal_json_value(&reader).await
        } else {
            vector_fs.retrieve_fs_path_simplified_json_value(&reader).await
        };

        fn add_shared_folder_info(obj: &mut serde_json::Value, shared_folders: &[SharedFolderInfo]) {
            if let Some(path) = obj.get("path") {
                if let Some(path_str) = path.as_str() {
                    if let Some(shared_folder) = shared_folders.iter().find(|sf| sf.path == path_str) {
                        let mut shared_folder_info = serde_json::to_value(shared_folder).unwrap();
                        // Remove the "tree" field from the shared_folder_info before adding it to the obj
                        if let Some(obj) = shared_folder_info.as_object_mut() {
                            obj.remove("tree");
                        }
                        obj.as_object_mut().unwrap().insert(
                            "shared_folder_info".to_string(),
                            serde_json::to_value(shared_folder).unwrap(),
                        );
                    }
                }
            }

            if let Some(child_folders) = obj.get_mut("child_folders") {
                if let Some(child_folders_array) = child_folders.as_array_mut() {
                    for child_folder in child_folders_array {
                        add_shared_folder_info(child_folder, shared_folders);
                    }
                }
            }
        }

        match result {
            Ok(mut result_value) => {
                let mut subscription_manager = ext_subscription_manager.lock().await;
                let shared_folders_result = subscription_manager
                    .available_shared_folders(
                        requester_name.extract_node(),
                        requester_name.get_profile_name_string().unwrap_or_default(),
                        requester_name.extract_node(),
                        requester_name.get_profile_name_string().unwrap_or_default(),
                        input_payload.path,
                    )
                    .await;
                drop(subscription_manager);

                if let Ok(shared_folders) = shared_folders_result {
                    add_shared_folder_info(&mut result_value, &shared_folders);
                }

                let _ = res.send(Ok(result_value)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve fs path json: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_search_items(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let vr_path = match input_payload.path {
            Some(path) => match VRPath::from_string(&path) {
                Ok(path) => path,
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to convert path to VRPath: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            },
            None => VRPath::root(),
        };
        let reader = vector_fs
            .new_reader(requester_name.clone(), vr_path, requester_name.clone())
            .await;
        let reader = match reader {
            Ok(reader) => reader,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create reader: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let max_resources_to_search = input_payload.max_files_to_scan.unwrap_or(100) as u64;
        let max_results = input_payload.max_results.unwrap_or(100) as u64;

        let query_embedding = vector_fs
            .generate_query_embedding_using_reader(input_payload.search, &reader)
            .await
            .unwrap();
        let search_results = vector_fs
            .vector_search_fs_item(&reader, query_embedding, max_resources_to_search)
            .await
            .unwrap();

        let results: Vec<String> = search_results
            .into_iter()
            .map(|res| res.path.to_string())
            .take(max_results as usize)
            .collect();

        let _ = res.send(Ok(results)).await.map_err(|_| ());
        Ok(())
    }

    // TODO: implement a vector search endpoint for finding FSItems (we'll need for the search UI in Visor for the FS) and one for the VRKai returned too
    pub async fn api_vec_fs_retrieve_vector_search_simplified_json(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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
            Some(path) => match VRPath::from_string(&path) {
                Ok(path) => path,
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to convert path to VRPath: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            },
            None => VRPath::root(),
        };
        let reader = vector_fs
            .new_reader(requester_name.clone(), vr_path, requester_name.clone())
            .await;
        let reader = match reader {
            Ok(reader) => reader,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create reader: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let max_resources_to_search = input_payload.max_files_to_scan.unwrap_or(100) as u64;
        let max_results = input_payload.max_results.unwrap_or(100) as u64;
        let search_results = match vector_fs
            .deep_vector_search(
                &reader,
                input_payload.search.clone(),
                max_resources_to_search,
                max_results,
            )
            .await
        {
            Ok(results) => results,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to perform deep vector search: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // TODO: Change path to be a single output string.
        // - Also return the source metadata, potentially using the format output method
        // that is used for showing search results to LLMs
        let results: Vec<(String, Vec<String>, f32)> = search_results
            .into_iter()
            .map(|res| {
                let content = match res.resource_retrieved_node.node.get_text_content() {
                    Ok(text) => text.to_string(),
                    Err(_) => "".to_string(),
                };
                let path_ids = res.clone().fs_item_path().path_ids;
                let score = res.resource_retrieved_node.score;
                (content, path_ids, score)
            })
            .collect();

        let _ = res.send(Ok(results)).await.map_err(|_| ());
        Ok(())
    }

    pub async fn api_vec_fs_create_folder(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let vr_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let writer = match vector_fs
            .new_writer(requester_name.clone(), vr_path, requester_name.clone())
            .await
        {
            Ok(writer) => writer,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create writer: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match vector_fs.create_new_folder(&writer, &input_payload.folder_name).await {
            Ok(_) => {
                let success_message = format!("Folder '{}' created successfully.", input_payload.folder_name);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create new folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_move_folder(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let folder_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert item path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let destination_path = match VRPath::from_string(&input_payload.destination_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert destination path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let orig_writer = match vector_fs
            .new_writer(requester_name.clone(), folder_path, requester_name.clone())
            .await
        {
            Ok(writer) => writer,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create writer for original folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match vector_fs.move_folder(&orig_writer, destination_path).await {
            Ok(_) => {
                let success_message = format!("Folder moved successfully to {}", input_payload.destination_path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to move folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_copy_folder(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let folder_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert folder path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let destination_path = match VRPath::from_string(&input_payload.destination_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert destination path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let orig_writer = match vector_fs
            .new_writer(requester_name.clone(), folder_path, requester_name.clone())
            .await
        {
            Ok(writer) => writer,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create writer for original folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match vector_fs.copy_folder(&orig_writer, destination_path).await {
            Ok(_) => {
                let success_message = format!("Folder copied successfully to {}", input_payload.destination_path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to copy folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_delete_item(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let item_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert item path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let orig_writer = match vector_fs
            .new_writer(requester_name.clone(), item_path, requester_name.clone())
            .await
        {
            Ok(writer) => writer,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create writer for item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match vector_fs.delete_item(&orig_writer).await {
            Ok(_) => {
                let success_message = format!("Item successfully deleted: {}", input_payload.path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to move item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_delete_folder(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let item_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert folder path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let orig_writer = match vector_fs
            .new_writer(requester_name.clone(), item_path, requester_name.clone())
            .await
        {
            Ok(writer) => writer,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create writer for item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match vector_fs.delete_folder(&orig_writer).await {
            Ok(_) => {
                let success_message = format!("Folder successfully deleted: {}", input_payload.path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to move item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_move_item(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let item_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert item path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let destination_path = match VRPath::from_string(&input_payload.destination_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert destination path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let orig_writer = match vector_fs
            .new_writer(requester_name.clone(), item_path, requester_name.clone())
            .await
        {
            Ok(writer) => writer,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create writer for original item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match vector_fs.move_item(&orig_writer, destination_path).await {
            Ok(_) => {
                let success_message = format!("Item moved successfully to {}", input_payload.destination_path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to move item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_copy_item(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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

        let item_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert item path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let destination_path = match VRPath::from_string(&input_payload.destination_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert destination path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let orig_writer = match vector_fs
            .new_writer(requester_name.clone(), item_path, requester_name.clone())
            .await
        {
            Ok(writer) => writer,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create writer for original item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        match vector_fs.copy_item(&orig_writer, destination_path).await {
            Ok(_) => {
                let success_message = format!("Item copied successfully to {}", input_payload.destination_path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to copy item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn api_vec_fs_retrieve_vector_resource(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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
        let vr_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let reader = vector_fs
            .new_reader(requester_name.clone(), vr_path, requester_name.clone())
            .await;
        let reader = match reader {
            Ok(reader) => reader,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create reader: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let result = vector_fs.retrieve_vector_resource(&reader).await;
        let result = match result {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve vector resource: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let json_resp = match result.to_json_value() {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert vector resource to json: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let _ = res.send(Ok(json_resp)).await.map_err(|_| ());
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn api_convert_files_and_save_to_folder(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        unstructured_api: Arc<UnstructuredAPI>,
        external_subscriber_manager: Arc<Mutex<ExternalSubscriberManager>>,
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
        let destination_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let files = {
            match vector_fs.db.get_all_files_from_inbox(input_payload.file_inbox.clone()) {
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

        type FileData = (String, Vec<u8>);
        type FileDataVec = Vec<FileData>;

        // Sort out the vrpacks from the rest
        let (vr_packs, other_files): (FileDataVec, FileDataVec) =
            files.into_iter().partition(|(name, _)| name.ends_with(".vrpack"));

        let mut dist_files = vec![];
        for file in other_files {
            let distribution_info = DistributionInfo::new_auto(&file.0, input_payload.file_datetime);
            dist_files.push((file.0, file.1, distribution_info));
        }

        let file_parser = match db.get_local_processing_preference()? {
            true => FileParser::Local,
            false => FileParser::Unstructured((*unstructured_api).clone()),
        };

        // TODO: provide a default agent so that an LLM can be used to generate description of the VR for document files
        let processed_vrkais =
            ParsingHelper::process_files_into_vrkai(dist_files, &*embedding_generator, None, file_parser).await?;

        // Save the vrkais into VectorFS
        let mut success_messages = Vec::new();
        for (filename, vrkai) in processed_vrkais {
            let folder_path = destination_path.clone();
            let writer = vector_fs
                .new_writer(requester_name.clone(), folder_path.clone(), requester_name.clone())
                .await?;

            let save_result = vector_fs.save_vrkai_in_folder(&writer, vrkai).await;
            let fs_item = match save_result {
                Ok(fs_item) => fs_item,
                Err(e) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Error saving '{}' in folder: {}", filename, e),
                        }))
                        .await;
                    return Ok(());
                }
            };

            #[derive(Serialize, Debug)]
            struct VectorResourceInfo {
                name: String,
                path: String,
                merkle_hash: String,
            }

            let resource_info = VectorResourceInfo {
                name: filename.to_string(),
                path: fs_item.path.to_string(),
                merkle_hash: fs_item.merkle_hash,
            };

            let success_message = match serde_json::to_value(&resource_info) {
                Ok(json) => json,
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to convert vector resource info to JSON: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };
            success_messages.push(success_message);
        }

        // Extract the vrpacks into the VectorFS
        for (filename, vrpack_bytes) in vr_packs {
            let vrpack = VRPack::from_bytes(&vrpack_bytes)?;

            let folder_path = destination_path.clone();
            let writer = vector_fs
                .new_writer(requester_name.clone(), folder_path.clone(), requester_name.clone())
                .await?;

            if let Err(e) = vector_fs.extract_vrpack_in_folder(&writer, vrpack).await {
                let _ = res
                    .send(Err(APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Error extracting/saving '{}' into folder: {}", filename, e),
                    }))
                    .await;
                return Ok(());
            }
        }
        {
            // remove inbox
            match vector_fs.db.remove_inbox(&input_payload.file_inbox) {
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
        }

        // We need to force ext_manager to update their cache
        {
            let mut ext_manager = external_subscriber_manager.lock().await;
            let _ = ext_manager.update_shared_folders().await;
        }
        let _ = res.send(Ok(success_messages)).await.map_err(|_| ());
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn retrieve_vr_kai(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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
        let vr_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let reader = vector_fs
            .new_reader(requester_name.clone(), vr_path, requester_name.clone())
            .await;
        let reader = match reader {
            Ok(reader) => reader,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create reader: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let result = vector_fs.retrieve_vrkai(&reader).await;
        let result = match result {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve vector resource: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let json_resp = match result.encode_as_base64() {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert vector resource to json: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let _ = res.send(Ok(json_resp)).await.map_err(|_| ());
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn retrieve_vr_pack(
        _db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
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
        let vr_path = match VRPath::from_string(&input_payload.path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let reader = vector_fs
            .new_reader(requester_name.clone(), vr_path, requester_name.clone())
            .await;
        let reader = match reader {
            Ok(reader) => reader,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create reader: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let result = vector_fs.retrieve_vrpack(&reader).await;
        let result = match result {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve vector resource: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let resp = match result.encode_as_base64() {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert vector resource to json: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        let _ = res.send(Ok(resp)).await.map_err(|_| ());
        Ok(())
    }
}
