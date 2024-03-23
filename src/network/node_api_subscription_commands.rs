use super::{node_api::APIError, node_error::NodeError, Node};
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::to_string;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIAvailableSharedItems, APICreateShareableFolder, APIUnshareFolder, APIUpdateShareableFolder,
            MessageSchemaType,
        },
    },
};

impl Node {
    pub async fn api_subscription_available_shared_items(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIAvailableSharedItems>(
                potentially_encrypted_msg,
                MessageSchemaType::AvailableSharedItems,
            )
            .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };
        eprintln!("\n\napi_subscription_available_shared_items");
        eprintln!("input_payload.node_name: {:?}", input_payload.node_name);
        eprintln!("self.node_name: {:?}", self.node_name.clone().get_node_name());

        if input_payload.node_name == self.node_name.clone().get_node_name() {
            eprintln!("Requesting shared folders from self");
            // Lock the mutex and handle the Option
            let mut subscription_manager = self.ext_subscription_manager.lock().await;
            let result = subscription_manager
                .available_shared_folders(requester_name, input_payload.path)
                .await;

            match result {
                Ok(result) => {
                    match to_string(&result) {
                        Ok(json_string) => {
                            let _ = res.send(Ok(json_string)).await.map_err(|_| ());
                        }
                        Err(e) => {
                            // Handle serialization error
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to serialize response: {}", e),
                            };
                            let _ = res.send(Err(api_error)).await;
                        }
                    }
                }
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: format!("Failed to convert path to VRPath: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                }
            }
        } else {
            eprintln!("Requesting shared folders from external");
            let mut my_subscription_manager = self.my_subscription_manager.lock().await;

            match ShinkaiName::new(input_payload.node_name.clone()) {
                Ok(ext_node_name) => {
                    let result = my_subscription_manager.get_shared_folder(&ext_node_name).await;
                    match result {
                        Ok(result) => {
                            match to_string(&result) {
                                Ok(json_string) => {
                                    let _ = res.send(Ok(json_string)).await.map_err(|_| ());
                                }
                                Err(e) => {
                                    // Handle serialization error
                                    let api_error = APIError {
                                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                        error: "Internal Server Error".to_string(),
                                        message: format!("Failed to serialize response: {}", e),
                                    };
                                    let _ = res.send(Err(api_error)).await;
                                }
                            }
                        }
                        Err(e) => {
                            let api_error = APIError {
                                code: StatusCode::BAD_REQUEST.as_u16(),
                                error: "Bad Request".to_string(),
                                message: format!("Failed to convert path to VRPath: {}", e),
                            };
                            let _ = res.send(Err(api_error)).await;
                        }
                    }
                }
                Err(_) => {
                    let api_error = APIError {
                        code: StatusCode::BAD_REQUEST.as_u16(),
                        error: "Bad Request".to_string(),
                        message: "Invalid node name provided".to_string(),
                    };
                    let _ = res.send(Err(api_error)).await;
                }
            }
        }

        Ok(())
    }

    pub async fn api_subscription_create_shareable_folder(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APICreateShareableFolder>(
                potentially_encrypted_msg,
                MessageSchemaType::CreateShareableFolder,
            )
            .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut subscription_manager = self.ext_subscription_manager.lock().await;
        let result = subscription_manager
            .available_shared_folders(requester_name, input_payload.path)
            .await;

        match result {
            Ok(_) => {
                let _ = res
                    .send(Ok("Folder successfully made shareable".to_string()))
                    .await
                    .map_err(|_| ());
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to create shareable folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }

    pub async fn api_subscription_update_shareable_folder(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIUpdateShareableFolder>(
                potentially_encrypted_msg,
                MessageSchemaType::UpdateShareableFolder,
            )
            .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut subscription_manager = self.ext_subscription_manager.lock().await;
        let result = subscription_manager
            .available_shared_folders(requester_name, input_payload.path)
            .await;

        match result {
            Ok(_) => {
                let _ = res
                    .send(Ok("Shareable folder requirements updated successfully".to_string()))
                    .await
                    .map_err(|_| ());
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to update shareable folder requirements: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn api_subscription_unshare_folder(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APIUnshareFolder>(
                potentially_encrypted_msg,
                MessageSchemaType::UnshareFolder,
            )
            .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut subscription_manager = self.ext_subscription_manager.lock().await;
        let result = subscription_manager
            .available_shared_folders(requester_name, input_payload.path)
            .await;
        match result {
            Ok(_) => {
                let _ = res
                    .send(Ok("Folder successfully unshared".to_string()))
                    .await
                    .map_err(|_| ());
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to unshare folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }
        Ok(())
    }
}
