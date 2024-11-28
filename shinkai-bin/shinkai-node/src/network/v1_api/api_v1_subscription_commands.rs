use std::{collections::HashMap, sync::Arc};

use crate::{
    managers::IdentityManager,
    network::{
        network_manager::{
            external_subscriber_manager::ExternalSubscriberManager, my_subscription_manager::MySubscriptionsManager,
        },
        node_error::NodeError,
        Node,
    },
};

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::{
    schemas::{
        file_links::FolderSubscriptionWithPath, shinkai_name::ShinkaiName, shinkai_subscription::ShinkaiSubscription,
    },
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIAvailableSharedItems, APICreateShareableFolder, APIGetLastNotifications, APIGetMySubscribers,
            APIGetNotificationsBeforeTimestamp, APISubscribeToSharedFolder, APIUnshareFolder,
            APIUnsubscribeToSharedFolder, APIUpdateShareableFolder, MessageSchemaType,
        },
    },
};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use tokio::sync::Mutex;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

impl Node {
    pub async fn api_unsubscribe_my_subscriptions(
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIUnsubscribeToSharedFolder>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::UnsubscribeToSharedFolder,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let my_subscription_manager = my_subscription_manager.lock().await;
        let sender_profile = requester_name.get_profile_name_string().unwrap_or("".to_string());

        match ShinkaiName::from_node_and_profile_names(
            input_payload.streamer_node_name.clone(),
            input_payload.streamer_profile_name.clone(),
        ) {
            Ok(ext_node_name) => {
                let result = my_subscription_manager
                    .unsubscribe_to_shared_folder(
                        ext_node_name,
                        input_payload.streamer_profile_name.clone(),
                        sender_profile,
                        input_payload.path,
                    )
                    .await;
                match result {
                    Ok(_) => {
                        let _ = res.send(Ok("Unsubscribed".to_string())).await.map_err(|_| ());
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

        Ok(())
    }

    pub async fn api_subscription_my_subscriptions(
        db: Arc<RwLock<SqliteManager>>,
        _vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        let (_, requester_name) = match Self::validate_and_extract_payload::<String>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::MySubscriptions,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        // Validation: requester_name node should be me
        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let db_result = db.list_all_my_subscriptions();

        match db_result {
            Ok(subscriptions) => {
                match serde_json::to_value(&subscriptions) {
                    Ok(json_value) => {
                        let _ = res.send(Ok(json_value)).await.map_err(|_| ());
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

    #[allow(clippy::too_many_arguments)]
    pub async fn api_subscription_available_shared_items(
        _db: Arc<RwLock<SqliteManager>>,
        _vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIAvailableSharedItems>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
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

        if input_payload.streamer_node_name == node_name.clone().get_node_name_string() {
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
            let mut subscription_manager = ext_subscription_manager.lock().await;
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
                    match serde_json::to_value(&result) {
                        Ok(json_value) => {
                            let _ = res.send(Ok(json_value)).await.map_err(|_| ());
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
            let mut my_subscription_manager = my_subscription_manager.lock().await;

            match ShinkaiName::from_node_and_profile_names(
                input_payload.streamer_node_name.clone(),
                input_payload.streamer_profile_name.clone(),
            ) {
                Ok(ext_node_name) => {
                    let result = my_subscription_manager.get_shared_folder(&ext_node_name).await;
                    match result {
                        Ok(result) => {
                            match serde_json::to_value(&result) {
                                Ok(json_value) => {
                                    let _ = res.send(Ok(json_value)).await.map_err(|_| ());
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

    pub async fn api_subscription_available_shared_items_open(
        node_name: ShinkaiName,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        input_payload: APIAvailableSharedItems,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        if input_payload.streamer_node_name == node_name.clone().get_node_name_string() {
            let mut subscription_manager = ext_subscription_manager.lock().await;
            // TODO: update. only feasible for root for now.
            let path = "/";
            let shared_folder_infos = subscription_manager.get_cached_shared_folder_tree(path).await;

            match serde_json::to_value(&shared_folder_infos) {
                Ok(json_value) => {
                    let _ = res.send(Ok(json_value)).await.map_err(|_| ());
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
        } else {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Streamer name doesn't match".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn api_subscription_subscribe_to_shared_folder(
        _db: Arc<RwLock<SqliteManager>>,
        _vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APISubscribeToSharedFolder>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
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

        let mut subscription_manager = my_subscription_manager.lock().await;
        let result = subscription_manager
            .subscribe_to_shared_folder(
                streamer_full_name.extract_node(),
                input_payload.streamer_profile_name.clone(),
                requester_profile,
                input_payload.path,
                input_payload.payment,
                input_payload.base_folder,
                input_payload.http_preferred,
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

    #[allow(clippy::too_many_arguments)]
    pub async fn api_subscription_create_shareable_folder(
        _db: Arc<RwLock<SqliteManager>>,
        _vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APICreateShareableFolder>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
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

        let mut subscription_manager = ext_subscription_manager.lock().await;
        let result = subscription_manager
            .create_shareable_folder(
                input_payload.path,
                requester_name,
                input_payload.subscription_req,
                input_payload.credentials,
            )
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

    #[allow(clippy::too_many_arguments)]
    pub async fn api_subscription_update_shareable_folder(
        _db: Arc<RwLock<SqliteManager>>,
        _vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIUpdateShareableFolder>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
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

        let subscription_manager = ext_subscription_manager.lock().await;
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

    #[allow(clippy::too_many_arguments)]
    pub async fn api_subscription_unshare_folder(
        _db: Arc<RwLock<SqliteManager>>,
        _vector_fs: Arc<VectorFS>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIUnshareFolder>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
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

        let mut subscription_manager = ext_subscription_manager.lock().await;
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

    pub async fn api_get_my_subscribers(
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, _) = match Self::validate_and_extract_payload::<APIGetMySubscribers>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::GetMySubscribers,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let subscription_manager = ext_subscription_manager.lock().await;
        let subscribers_result = subscription_manager
            .get_node_subscribers(Some(input_payload.path))
            .await;

        match subscribers_result {
            Ok(subscribers) => {
                let _ = res.send(Ok(subscribers)).await.map_err(|_| ());
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to retrieve subscribers: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn api_get_http_free_subscription_links(
        db: Arc<RwLock<SqliteManager>>,
        _node_name: ShinkaiName,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        subscription_id: String,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the format of subscription_id to be "PROFILE:::PATH"
        let parts: Vec<&str> = subscription_id.split(":::").collect();
        if parts.len() != 2 {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid subscription_id format. Expected format 'PROFILE:::PATH'.".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        let _profile = parts[0].to_string();
        let path = parts[1].to_string();

        let folder_subscription = match db.get_folder_requirements(&path) {
            Ok(result) => result,
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::BAD_REQUEST.as_u16(),
                    error: "Bad Request".to_string(),
                    message: format!("Failed to retrieve folder requirements: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        let folder_subs_with_path = FolderSubscriptionWithPath {
            path: path.clone(),
            folder_subscription,
        };

        let subscription_manager = ext_subscription_manager.lock().await;
        let file_links = subscription_manager
            .http_subscription_upload_manager
            .get_cached_subscription_files_links(&folder_subs_with_path);

        match serde_json::to_value(&file_links) {
            Ok(json_value) => {
                let _ = res.send(Ok(json_value)).await.map_err(|_| ());
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to serialize response: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn api_get_last_notifications(
        db: Arc<RwLock<SqliteManager>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) = match Self::validate_and_extract_payload::<APIGetLastNotifications>(
            node_name.clone(),
            identity_manager.clone(),
            encryption_secret_key,
            potentially_encrypted_msg,
            MessageSchemaType::GetLastNotifications,
        )
        .await
        {
            Ok(data) => data,
            Err(api_error) => {
                let _ = res.send(Err(api_error)).await;
                return Ok(());
            }
        };

        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        match db.get_last_notifications(requester_name.clone(), input_payload.count, input_payload.timestamp) {
            Ok(notifications) => {
                let _ = res.send(Ok(serde_json::to_value(notifications).unwrap())).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get last notifications: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }

    pub async fn api_get_notifications_before_timestamp(
        db: Arc<RwLock<SqliteManager>>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        encryption_secret_key: EncryptionStaticKey,
        potentially_encrypted_msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        let (input_payload, requester_name) =
            match Self::validate_and_extract_payload::<APIGetNotificationsBeforeTimestamp>(
                node_name.clone(),
                identity_manager.clone(),
                encryption_secret_key,
                potentially_encrypted_msg,
                MessageSchemaType::GetNotificationsBeforeTimestamp,
            )
            .await
            {
                Ok(data) => data,
                Err(api_error) => {
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        match db.get_notifications_before_timestamp(
            requester_name.clone(),
            input_payload.timestamp,
            input_payload.count,
        ) {
            Ok(notifications) => {
                let _ = res.send(Ok(serde_json::to_value(notifications).unwrap())).await;
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to get notifications before timestamp: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
            }
        }

        Ok(())
    }
}
