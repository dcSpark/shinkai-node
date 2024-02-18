use super::{node_api::APIError, node_error::NodeError, Node};
use crate::{schemas::identity::Identity, vector_fs::vector_fs_types::DistributionOrigin};
use aes_gcm::aead::{generic_array::GenericArray, Aead};
use async_channel::Sender;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{
            APIAddAgentRequest, APIConvertFilesAndSaveToFolder, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest,
            APIVecFSRetrieveVectorResource, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder,
            APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
            APIVecFsRetrieveVectorSearchSimplifiedJson, IdentityPermissions, MessageSchemaType,
            RegistrationCodeRequest, RegistrationCodeType,
        },
    },
};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, VRPath};

impl Node {
    async fn validate_and_extract_payload<T: DeserializeOwned>(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        schema_type: MessageSchemaType,
    ) -> Result<(T, ShinkaiName), APIError> {
        let validation_result = self
            .validate_message(potentially_encrypted_msg, Some(schema_type))
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

    pub async fn api_vec_fs_retrieve_path_simplified_json(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFsRetrievePathSimplifiedJson>(
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
        let mut vector_fs = self.vector_fs.lock().await;
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
        let reader = vector_fs.new_reader(requester_name.clone(), vr_path, requester_name.clone());
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

        let result = vector_fs.retrieve_fs_path_simplified_json(&reader);
        let result = match result {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to retrieve fs path simplified json: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let _ = res.send(Ok(result)).await.map_err(|_| ());
        Ok(())
    }

    pub async fn api_vec_fs_retrieve_vector_search_simplified_json(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<(String, Vec<String>, f32)>, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFsRetrieveVectorSearchSimplifiedJson>(
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

        let mut vector_fs = self.vector_fs.lock().await;
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
        let reader = vector_fs.new_reader(requester_name.clone(), vr_path, requester_name.clone());
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
            .deep_vector_search(&reader, input_payload.search.clone(), max_resources_to_search, max_results)
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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFsCreateFolder>(
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

        let mut vector_fs = self.vector_fs.lock().await;
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

        let writer = match vector_fs.new_writer(requester_name.clone(), vr_path, requester_name.clone()) {
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

        match vector_fs.create_new_folder(&writer, &input_payload.folder_name) {
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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFsMoveFolder>(
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

        let mut vector_fs = self.vector_fs.lock().await;
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

        let orig_writer = match vector_fs.new_writer(requester_name.clone(), folder_path, requester_name.clone()) {
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

        match vector_fs.move_folder(&orig_writer, destination_path) {
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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFsCopyFolder>(
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

        let mut vector_fs = self.vector_fs.lock().await;
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

        let orig_writer = match vector_fs.new_writer(requester_name.clone(), folder_path, requester_name.clone()) {
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

        match vector_fs.copy_folder(&orig_writer, destination_path) {
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

    pub async fn api_vec_fs_move_item(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFsMoveItem>(
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

        let mut vector_fs = self.vector_fs.lock().await;
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

        let orig_writer = match vector_fs.new_writer(requester_name.clone(), item_path, requester_name.clone()) {
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

        match vector_fs.move_item(&orig_writer, destination_path) {
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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFsCopyItem>(
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

        let mut vector_fs = self.vector_fs.lock().await;
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

        let orig_writer = match vector_fs.new_writer(requester_name.clone(), item_path, requester_name.clone()) {
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

        match vector_fs.copy_item(&orig_writer, destination_path) {
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
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIVecFSRetrieveVectorResource>(
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
        let mut vector_fs = self.vector_fs.lock().await;
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
        let reader = vector_fs.new_reader(requester_name.clone(), vr_path, requester_name.clone());
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

        let result = vector_fs.retrieve_vector_resource(&reader);
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

        let json_resp = match result.to_json() {
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

    pub async fn api_convert_files_and_save_to_folder(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIConvertFilesAndSaveToFolder>(
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
        let mut vector_fs = self.vector_fs.lock().await;
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

        // For now just check for .vrkai files and store them
        let files = {
            let db_lock = self.db.lock().await;
            match db_lock.get_all_files_from_inbox(input_payload.file_inbox.clone()) {
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
        // eprintln!("Files: {:?}", files);

        // TODO(Rob): we also need to handle the other ones which need to be converted to vrkai
        // read files from file_inbox
        // write .vrkai directly
        // convert other files to .vrkai
        let vrkai_files: Vec<(String, Vec<u8>)> =
            files.into_iter().filter(|(name, _)| name.ends_with(".vrkai")).collect();

        let mut success_messages = Vec::new();

        for vrkai_file in vrkai_files {
            let first_folder_path = destination_path.clone();
            eprintln!("first_folder_path: {:?}", first_folder_path);
            let writer = vector_fs
                .new_writer(requester_name.clone(), first_folder_path, requester_name.clone())
                .unwrap();

            // Convert Vec<u8> to a String to use with from_json
            let json_str = match String::from_utf8(vrkai_file.1.clone()) {
                Ok(str) => str,
                Err(err) => {
                    let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to convert Vec<u8> to String: {}", err),
                        }))
                        .await;
                    return Ok(());
                }
            };


            let base_vr = match BaseVectorResource::from_json(&json_str) {
                Ok(vr) => vr,
                Err(err) => {
                    // Attempt to unescape the JSON string and retry parsing
                    let unescaped_json_str = json_str.replace("\\\"", "\"");
                    match BaseVectorResource::from_json(&unescaped_json_str) {
                        Ok(vr) => vr,
                        Err(_) => {
                            let _ = res
                                .send(Err(APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to parse JSON to BaseVectorResource: {}", err),
                                }))
                                .await;
                            return Ok(());
                        }
                    }
                }
            };

            if let Err(e) = vector_fs.save_vector_resource_in_folder(
                &writer,
                base_vr,
                None,       // TODO: we could extract it if it's part of the vrkai
                DistributionOrigin::None,   // TODO: extend the schema or read it from the vrkai
            ) {
                let _ = res
                        .send(Err(APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Error saving vector resource in folder: {}", e),
                        }))
                        .await;
                    return Ok(());
            }

            // TODO: Define permissions here
            //     let perm_writer = vector_fs
            //     .new_writer(default_test_profile(), item.path.clone(), default_test_profile())
            //     .unwrap();
            // vector_fs
            //     .set_path_permission(&perm_writer, ReadPermission::Private, WritePermission::Private)
            //     .unwrap();

            let success_message = format!("Vector resource '{}' saved in folder successfully.", vrkai_file.0);
            success_messages.push(success_message);
        }

        let _ = res.send(Ok(success_messages)).await.map_err(|_| ());
        Ok(())
    }
}
