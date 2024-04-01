use super::{node_api::APIError, node_error::NodeError, Node};
use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::to_string;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIAvailableSharedItems, APICreateShareableFolder, APISubscribeToSharedFolder, APIUnshareFolder,
            APIUpdateShareableFolder, MessageSchemaType,
        },
    },
};

impl Node {
    pub async fn api_subscription_my_subscriptions(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (_, requester_name) = match self
            .validate_and_extract_payload::<String>(potentially_encrypted_msg, MessageSchemaType::MySubscriptions)
            .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != self.node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let db_lock = self.db.lock().await;
        let db_result = db_lock.list_all_my_subscriptions();

        match db_result {
            Ok(subscriptions) => {
                match to_string(&subscriptions) {
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
                    message: format!("Failed to retrieve subscriptions: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

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

        if input_payload.streamer_node_name == self.node_name.clone().get_node_name_string() {
            if !requester_name.has_profile() {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Requester name does not have a profile".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            let streamer_full_name = ShinkaiName::from_node_and_profile_names(
                input_payload.streamer_node_name.clone(),
                input_payload.streamer_profile_name.clone(),
            );
            if streamer_full_name.is_err() {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Invalid origin node name or profile name provided".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }

            let requester_profile = requester_name.get_profile_name_string().unwrap();

            // Lock the mutex and handle the Option
            let mut subscription_manager = self.ext_subscription_manager.lock().await;
            let result = subscription_manager
                .available_shared_folders(
                    streamer_full_name.unwrap().extract_node(),
                    input_payload.streamer_profile_name.clone(),
                    requester_name.extract_node(),
                    requester_profile.clone(),
                    input_payload.path,
                )
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
            let mut my_subscription_manager = self.my_subscription_manager.lock().await;

            match ShinkaiName::from_node_and_profile_names(input_payload.streamer_node_name.clone(), input_payload.streamer_profile_name.clone()) {
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

    pub async fn api_subscription_subscribe_to_shared_folder(
        &self,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match self
            .validate_and_extract_payload::<APISubscribeToSharedFolder>(
                potentially_encrypted_msg,
                MessageSchemaType::SubscribeToSharedFolder,
            )
            .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let requester_profile = requester_name.get_profile_name_string().unwrap_or("".to_string());

        let streamer_full_name = match ShinkaiName::from_node_and_profile_names(
            input_payload.streamer_node_name.clone(),
            input_payload.streamer_profile_name.clone(),
        ) {
            Ok(shinkai_name) => shinkai_name,
            Err(_) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: "Invalid node name provided".to_string(),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let mut subscription_manager = self.my_subscription_manager.lock().await;
        let result = subscription_manager
            .subscribe_to_shared_folder(
                streamer_full_name.extract_node(),
                input_payload.streamer_profile_name.clone(),
                requester_profile,
                input_payload.path,
                input_payload.payment,
            )
            .await;

        match result {
            Ok(_) => {
                let _ = res.send(Ok("Subscription Requested".to_string())).await.map_err(|_| ());
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to subscribe to shared folder: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
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

        if !requester_name.has_profile() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Requester name does not have a profile".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let mut subscription_manager = self.ext_subscription_manager.lock().await;
        let result = subscription_manager
            .create_shareable_folder(input_payload.path, requester_name, input_payload.subscription_req)
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
            .update_shareable_folder_requirements(input_payload.path, requester_name, input_payload.subscription)
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
            .unshare_folder(input_payload.path, requester_name)
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
