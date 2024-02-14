use super::{node_api::APIError, node_error::NodeError, Node};
use crate::schemas::identity::Identity;
use aes_gcm::aead::{generic_array::GenericArray, Aead};
use async_channel::Sender;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{
            APIAddAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveVectorSearchSimplifiedJson, IdentityPermissions, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType
        },
    },
};
use shinkai_vector_resources::vector_resource::VRPath;

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
        res: Sender<Result<Vec<String>, APIError>>,
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

        let search_results = match vector_fs
            .deep_vector_search(&reader, input_payload.search.clone(), 100, 100)
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

        // TODO: Not sure if this is the right thing to do
        let results: Vec<String> = search_results
            .into_iter()
            .map(|res| res.fs_item_path().format_to_string())
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


}
