use std::sync::Arc;

use async_channel::Sender;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIConvertFilesAndSaveToFolder, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder,
    APIVecFsDeleteItem, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsSearchItems,
};
use shinkai_vector_resources::{
    embedding_generator::EmbeddingGenerator, file_parser::unstructured_api::UnstructuredAPI, vector_resource::VRPath,
};
use tokio::sync::Mutex;

use crate::{
    db::ShinkaiDB,
    managers::IdentityManager,
    network::{
        node_api_router::APIError,
        node_error::NodeError,
        subscription_manager::external_subscriber_manager::{ExternalSubscriberManager, SharedFolderInfo},
        Node,
    },
    schemas::identity::Identity,
    vector_fs::vector_fs::VectorFS,
};

impl Node {
    pub async fn v2_api_vec_fs_retrieve_path_simplified_json(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsRetrievePathSimplifiedJson,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
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
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let result = vector_fs.retrieve_fs_path_simplified_json_value(&reader).await;

        fn add_shared_folder_info(obj: &mut serde_json::Value, shared_folders: &[SharedFolderInfo]) {
            if let Some(path) = obj.get("path") {
                if let Some(path_str) = path.as_str() {
                    if let Some(shared_folder) = shared_folders.iter().find(|sf| sf.path == path_str) {
                        let mut shared_folder_info = serde_json::to_value(shared_folder).unwrap();
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

    pub async fn v2_convert_files_and_save_to_folder(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIConvertFilesAndSaveToFolder,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        unstructured_api: Arc<UnstructuredAPI>,
        external_subscriber_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        res: Sender<Result<Vec<Value>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        Self::process_and_save_files(
            db,
            vector_fs,
            input_payload,
            requester_name,
            embedding_generator,
            unstructured_api,
            external_subscriber_manager,
            res,
        )
        .await
    }

    pub async fn v2_create_folder(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsCreateFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
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

    pub async fn v2_move_item(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsMoveItem,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let origin_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert origin path to VRPath: {}", e),
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

        let writer = match vector_fs
            .new_writer(requester_name.clone(), origin_path, requester_name.clone())
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

        match vector_fs.move_item(&writer, destination_path).await {
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

    pub async fn v2_copy_item(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsCopyItem,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let origin_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert origin path to VRPath: {}", e),
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

        let writer = match vector_fs
            .new_writer(requester_name.clone(), origin_path, requester_name.clone())
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

        match vector_fs.copy_item(&writer, destination_path).await {
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

    pub async fn v2_move_folder(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsMoveFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let origin_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert origin path to VRPath: {}", e),
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

        let writer = match vector_fs
            .new_writer(requester_name.clone(), origin_path, requester_name.clone())
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

        match vector_fs.move_folder(&writer, destination_path).await {
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

    pub async fn v2_copy_folder(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsCopyFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let origin_path = match VRPath::from_string(&input_payload.origin_path) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert origin path to VRPath: {}", e),
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

        let writer = match vector_fs
            .new_writer(requester_name.clone(), origin_path, requester_name.clone())
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

        match vector_fs.copy_folder(&writer, destination_path).await {
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

    pub async fn v2_delete_folder(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsDeleteFolder,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
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

        let writer = match vector_fs
            .new_writer(requester_name.clone(), item_path, requester_name.clone())
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

        match vector_fs.delete_folder(&writer).await {
            Ok(_) => {
                let success_message = format!("Folder successfully deleted: {}", input_payload.path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to delete folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_delete_item(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsDeleteItem,
        bearer: String,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
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

        let writer = match vector_fs
            .new_writer(requester_name.clone(), item_path, requester_name.clone())
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

        match vector_fs.delete_item(&writer).await {
            Ok(_) => {
                let success_message = format!("Item successfully deleted: {}", input_payload.path);
                let _ = res.send(Ok(success_message)).await.map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to delete item: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_search_items(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        input_payload: APIVecFsSearchItems,
        bearer: String,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let search_path_str = input_payload.path.as_deref().unwrap_or("/");
        let search_path = match VRPath::from_string(search_path_str) {
            Ok(path) => path,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to convert search path to VRPath: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let reader = match vector_fs
            .new_reader(requester_name.clone(), search_path, requester_name.clone())
            .await
        {
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

    pub async fn v2_retrieve_vector_resource(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        path: String,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let requester_name = match identity_manager.lock().await.get_main_identity() {
            Some(Identity::Standard(std_identity)) => std_identity.clone().full_identity_name,
            _ => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Wrong identity type. Expected Standard identity.".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let vr_path = match VRPath::from_string(&path) {
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
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let result = vector_fs.retrieve_vector_resource(&reader).await;

        match result {
            Ok(result_value) => match result_value.to_json_value() {
                Ok(json_value) => {
                    let _ = res.send(Ok(json_value)).await.map_err(|_| ());
                    Ok(())
                }
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to convert result to JSON: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    Ok(())
                }
            },
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve vector resource: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_upload_file_to_folder(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        embedding_generator: Arc<dyn EmbeddingGenerator>,
        unstructured_api: Arc<UnstructuredAPI>,
        external_subscriber_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        filename: String,
        file: Vec<u8>,
        path: String,
        file_datetime: Option<DateTime<Utc>>,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Step 1: Create a file inbox
        let hash_hex = uuid::Uuid::new_v4().to_string();
        if let Err(err) = db.create_files_message_inbox(hash_hex.clone()) {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: format!("Failed to create files message inbox: {}", err),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }
        let file_inbox_name = hash_hex;

        // Step 2: Add the file to the inbox
        let (add_file_res_sender, add_file_res_receiver) = async_channel::bounded(1);

        match Self::v2_add_file_to_inbox(
            db.clone(),
            vector_fs.clone(),
            file_inbox_name.clone(),
            filename.clone(),
            file.clone(),
            bearer.clone(),
            add_file_res_sender,
        )
        .await
        {
            Ok(_) => match add_file_res_receiver.recv().await {
                Ok(Ok(_)) => {}
                Ok(Err(api_error)) => {
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
                Err(_) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: "Failed to receive add file result".to_string(),
                        }))
                        .await;
                    return Ok(());
                }
            },
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Step 3: Convert the file and save it to the folder
        let input_payload = APIConvertFilesAndSaveToFolder {
            path,
            file_inbox: file_inbox_name,
            file_datetime,
        };

        let (convert_res_sender, convert_res_receiver) = async_channel::bounded(1);

        match Self::v2_convert_files_and_save_to_folder(
            db,
            vector_fs,
            identity_manager,
            input_payload,
            embedding_generator,
            unstructured_api,
            external_subscriber_manager,
            bearer,
            convert_res_sender,
        )
        .await
        {
            Ok(_) => match convert_res_receiver.recv().await {
                Ok(Ok(result)) => {
                    let first_element = match result.into_iter().next() {
                        Some(element) => element,
                        None => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: "Result array is empty".to_string(),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
                    };
                    let _ = res.send(Ok(first_element)).await;
                }
                Ok(Err(api_error)) => {
                    let _ = res.send(Err(api_error)).await;
                }
                Err(_) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: "Failed to receive conversion result".to_string(),
                        }))
                        .await;
                }
            },
            Err(node_error) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to convert files and save to folder: {}", node_error),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
