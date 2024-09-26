use std::{collections::HashMap, sync::Arc};

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::{
    schemas::{
        file_links::FolderSubscriptionWithPath, identity::Identity, shinkai_name::ShinkaiName,
        shinkai_subscription::ShinkaiSubscription,
    },
    shinkai_message::shinkai_message_schemas::{
        APIAvailableSharedItems, APICreateShareableFolder, APIGetLastNotifications, APIGetMySubscribers,
        APIGetNotificationsBeforeTimestamp, APISubscribeToSharedFolder, APIUnshareFolder, APIUnsubscribeToSharedFolder,
        APIUpdateShareableFolder,
    },
};

use tokio::sync::Mutex;

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

impl Node {
    pub async fn v2_api_available_shared_items(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        bearer: String,
        payload: APIAvailableSharedItems,
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

        if payload.streamer_node_name == node_name.clone().get_node_name_string() {
            let streamer_full_name = ShinkaiName::from_node_and_profile_names(
                payload.streamer_node_name.clone(),
                payload.streamer_profile_name.clone(),
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

            let mut subscription_manager = ext_subscription_manager.lock().await;
            let result = subscription_manager
                .available_shared_folders(
                    streamer_full_name.unwrap().extract_node(),
                    payload.streamer_profile_name.clone(),
                    requester_name.extract_node(),
                    requester_profile.clone(),
                    payload.path,
                )
                .await;

            match result {
                Ok(result) => match serde_json::to_value(&result) {
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
                },
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
                payload.streamer_node_name.clone(),
                payload.streamer_profile_name.clone(),
            ) {
                Ok(ext_node_name) => {
                    let result = my_subscription_manager.get_shared_folder(&ext_node_name).await;
                    match result {
                        Ok(result) => match serde_json::to_value(&result) {
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
                        },
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

    pub async fn v2_api_available_shared_items_open(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        input_payload: APIAvailableSharedItems,
        res: Sender<Result<serde_json::Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

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

    pub async fn v2_api_create_shareable_folder(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        payload: APICreateShareableFolder,
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
                payload.path,
                requester_name,
                payload.subscription_req,
                payload.credentials,
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

    pub async fn v2_api_update_shareable_folder(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        payload: APIUpdateShareableFolder,
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

        let subscription_manager = ext_subscription_manager.lock().await;
        let result = subscription_manager
            .update_shareable_folder_requirements(payload.path, requester_name, payload.subscription)
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

    pub async fn v2_api_unshare_folder(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        payload: APIUnshareFolder,
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

        let mut subscription_manager = ext_subscription_manager.lock().await;
        let result = subscription_manager.unshare_folder(payload.path, requester_name).await;

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

    pub async fn v2_api_subscribe_to_shared_folder(
        db: Arc<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        bearer: String,
        payload: APISubscribeToSharedFolder,
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

        let requester_profile = requester_name.get_profile_name_string().unwrap_or("".to_string());

        let streamer_full_name = match ShinkaiName::from_node_and_profile_names(
            payload.streamer_node_name.clone(),
            payload.streamer_profile_name.clone(),
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
                payload.streamer_profile_name.clone(),
                requester_profile,
                payload.path,
                payload.payment,
                payload.base_folder,
                payload.http_preferred,
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

    pub async fn v2_api_unsubscribe(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        bearer: String,
        payload: APIUnsubscribeToSharedFolder,
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
            payload.streamer_node_name.clone(),
            payload.streamer_profile_name.clone(),
        ) {
            Ok(ext_node_name) => {
                let result = my_subscription_manager
                    .unsubscribe_to_shared_folder(
                        ext_node_name,
                        payload.streamer_profile_name.clone(),
                        sender_profile,
                        payload.path,
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

    pub async fn v2_api_my_subscriptions(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
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

    pub async fn v2_api_get_my_subscribers(
        db: Arc<ShinkaiDB>,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        payload: APIGetMySubscribers,
        res: Sender<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let subscription_manager = ext_subscription_manager.lock().await;
        let subscribers_result = subscription_manager.get_node_subscribers(Some(payload.path)).await;

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

    pub async fn v2_api_get_http_free_subscription_links(
        db: Arc<ShinkaiDB>,
        ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        bearer: String,
        subscription_profile_path: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Validate the format of subscription_profile_path to be "PROFILE:::PATH"
        let parts: Vec<&str> = subscription_profile_path.split(":::").collect();
        if parts.len() != 2 {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid subscription_profile_path format. Expected format 'PROFILE:::PATH'.".to_string(),
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

    pub async fn v2_api_get_last_notifications(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        payload: APIGetLastNotifications,
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

        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        match db.get_last_notifications(requester_name.clone(), payload.count, payload.timestamp) {
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

    pub async fn v2_api_get_notifications_before_timestamp(
        db: Arc<ShinkaiDB>,
        node_name: ShinkaiName,
        identity_manager: Arc<Mutex<IdentityManager>>,
        bearer: String,
        payload: APIGetNotificationsBeforeTimestamp,
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

        if requester_name.get_node_name_string() != node_name.clone().get_node_name_string() {
            let api_error = APIError {
                code: StatusCode::BAD_REQUEST.as_u16(),
                error: "Bad Request".to_string(),
                message: "Invalid node name provided".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
            return Ok(());
        }

        match db.get_notifications_before_timestamp(requester_name.clone(), payload.timestamp, payload.count) {
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
