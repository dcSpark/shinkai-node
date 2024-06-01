use std::fs;
use std::path::Path;

use crate::http_requests::{request_post, PostRequestError};
use ed25519_dalek::SigningKey;
use serde_json::Value;
use shinkai_message_primitives::schemas::shinkai_subscription_req::{FolderSubscription, SubscriptionPayment};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APICreateShareableFolder, FileDestinationCredentials,
};
use shinkai_message_primitives::shinkai_utils::{
    encryption::{string_to_encryption_public_key, string_to_encryption_static_key},
    shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString},
    signatures::string_to_signature_secret_key,
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::shinkai_utils::decrypt_exported_keys;
use super::{shinkai_device_keys::ShinkaiDeviceKeys, shinkai_response_types::NodeHealthStatus};

#[derive(Clone)]
pub struct ShinkaiManagerForSubs {
    pub message_builder: ShinkaiMessageBuilder,
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub my_signature_secret_key: SigningKey,
    pub receiver_public_key: EncryptionPublicKey,
    pub sender: ShinkaiNameString,
    pub sender_subidentity: String,
    pub node_receiver: ShinkaiNameString,
    pub node_receiver_subidentity: ShinkaiNameString,
    pub node_address: String, // Should it be a SockAddr instead?
}

impl ShinkaiManagerForSubs {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: String,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString, // this is the destination profile
        node_address: String,
    ) -> Self {
        let shinkai_message_builder = ShinkaiMessageBuilder::new(
            my_encryption_secret_key.clone(),
            my_signature_secret_key.clone(),
            receiver_public_key,
        );

        Self {
            message_builder: shinkai_message_builder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            node_address,
        }
    }

    pub fn initialize(keys: ShinkaiDeviceKeys) -> Self {
        let recipient = keys.shinkai_identity.clone();
        let sender = recipient.clone();
        let sender_subidentity = keys.profile.to_string();

        ShinkaiManagerForSubs::new(
            string_to_encryption_static_key(&keys.profile_encryption_sk.clone()).unwrap(),
            string_to_signature_secret_key(&keys.profile_identity_sk).unwrap(),
            string_to_encryption_public_key(&keys.node_encryption_pk).unwrap(),
            sender.clone(),
            sender_subidentity.clone(),
            sender,             // node_receiver
            sender_subidentity, // node_receiver_subidentity
            keys.node_address.clone(),
        )
    }

    pub fn initialize_from_encrypted_file_path(file_path: &Path, passphrase: &str) -> Result<Self, &'static str> {
        let encrypted_keys = fs::read_to_string(file_path).map_err(|_| "Failed to read encrypted keys from file")?;
        let decrypted_keys = decrypt_exported_keys(&encrypted_keys, passphrase)?;

        Ok(Self::initialize(decrypted_keys))
    }

    // Need review
    pub async fn check_node_health(&self) -> Result<NodeHealthStatus, &'static str> {
        let shinkai_health_url = format!("{}/v1/shinkai_health", self.node_address);
        // eprintln!("Checking node health at: {}", shinkai_health_url.clone());

        match reqwest::get(&shinkai_health_url).await {
            Ok(response) => {
                if response.status().is_success() {
                    let health_data: serde_json::Value =
                        response.json().await.expect("Failed to parse health check response");

                    let health_status: NodeHealthStatus = serde_json::from_value(health_data.clone())
                        .expect("Failed to parse health data into NodeHealthStatusPayload");

                    if health_status.status == "ok" {
                        Ok(health_status)
                    } else {
                        Err("Shinkai node health check failed")
                    }
                } else {
                    Err("Failed to reach Shinkai node for health check")
                }
            }
            Err(_e) => Err("Error verifying node health"),
        }
    }

    // Need review
    pub async fn get_node_folder(&self, path: &str) -> Result<serde_json::Value, PostRequestError> {
        let formatted_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        // println!("Checking {} in vector FS using vecfs_retrieve_path_simplified", &path);
        let shinkai_message = ShinkaiMessageBuilder::vecfs_retrieve_path_simplified(
            &formatted_path,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            "".to_string(),
        )
        .unwrap(); // Consider handling this unwrap more gracefully

        let payload = serde_json::to_string(&shinkai_message).expect("Failed to serialize shinkai_message");
        let response = request_post(
            self.node_address.clone(),
            payload,
            "/v1/vec_fs/retrieve_path_simplified_json",
        )
        .await;

        match response {
            Ok(data) => Ok(data.data),
            Err(e) => Err(e),
        }
    }

    pub async fn create_folder(&self, folder_name: &str, path: &str) -> Result<(), &'static str> {
        let formatted_path = if path == "/" {
            path.to_string()
        } else {
            let mut name = if !path.starts_with('/') {
                format!("/{}", path) // Add "/" at the start if not present
            } else {
                path.to_string()
            };
            if name.ends_with('/') && name != "/" {
                name.pop(); // Remove trailing '/' if present and not the root path
            }
            name
        };

        // println!(
        //     "Creating folder: {} in path: {}",
        //     &folder_name.to_string(),
        //     &formatted_path
        // );
        let shinkai_message = ShinkaiMessageBuilder::vecfs_create_folder(
            folder_name,
            &formatted_path,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )?;

        let folder_creation_message = serde_json::json!(shinkai_message);
        let resp = request_post(
            self.node_address.clone(),
            folder_creation_message.to_string(),
            "/v1/vec_fs/create_folder",
        )
        .await;

        match resp {
            Ok(_) => {
                // println!("Folder creation successful: {:?}", response);
            }
            Err(_e) => {
                return Err("Failed to create folder");
            }
        }

        Ok(())
    }

    pub async fn create_share_folder(
        &self,
        path: &str,
        subscription_req: FolderSubscription,
        credentials: Option<FileDestinationCredentials>,
    ) -> Result<(), &'static str> {
        let formatted_path = if path == "/" {
            path.to_string()
        } else {
            let mut name = if !path.starts_with('/') {
                format!("/{}", path) // Add "/" at the start if not present
            } else {
                path.to_string()
            };
            if name.ends_with('/') && name != "/" {
                name.pop(); // Remove trailing '/' if present and not the root path
            }
            name
        };

        let payload = APICreateShareableFolder {
            path: formatted_path.to_string(),
            subscription_req,
            credentials,
        };

        let shinkai_message = ShinkaiMessageBuilder::subscriptions_create_share_folder(
            payload,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )?;

        let folder_creation_message = serde_json::json!(shinkai_message);
        let resp = request_post(
            self.node_address.clone(),
            folder_creation_message.to_string(),
            "/v1/create_shareable_folder",
        )
        .await;

        match resp {
            Ok(_) => {
                // println!("Folder creation successful: {:?}", response);
            }
            Err(_e) => {
                return Err("Failed to create folder");
            }
        }

        Ok(())
    }

    pub async fn subscribe_to_folder(
        &self,
        shared_folder_path: &str,
        streamer_node: String,
        streamer_profile: String,
        subscription_req: SubscriptionPayment,
        http_preferred: Option<bool>,
        base_folder: Option<String>,
    ) -> Result<(), &'static str> {
        let formatted_path = if shared_folder_path == "/" {
            shared_folder_path.to_string()
        } else {
            let mut name = if !shared_folder_path.starts_with('/') {
                format!("/{}", shared_folder_path) // Add "/" at the start if not present
            } else {
                shared_folder_path.to_string()
            };
            if name.ends_with('/') && name != "/" {
                name.pop(); // Remove trailing '/' if present and not the root path
            }
            name
        };

        let shinkai_message = ShinkaiMessageBuilder::vecfs_subscribe_to_shared_folder(
            formatted_path.to_string(),
            subscription_req,
            http_preferred,
            base_folder,
            streamer_node.to_string(),
            streamer_profile.to_string(),
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
            None
        )
        .unwrap();

        let folder_creation_message = serde_json::json!(shinkai_message);
        let resp = request_post(
            self.node_address.clone(),
            folder_creation_message.to_string(),
            "/v1/subscribe_to_shared_folder",
        )
        .await;

        match resp {
            Ok(_) => {
                // println!("Folder creation successful: {:?}", response);
            }
            Err(_e) => {
                return Err("Failed to create folder");
            }
        }

        Ok(())
    }

    pub async fn available_shared_items(
        &self,
        path: &str,
        streamer_node_name: String,
        streamer_profile_name: String,
    ) -> Result<Value, &'static str> {
        let formatted_path = if path == "/" {
            path.to_string()
        } else {
            let mut name = if !path.starts_with('/') {
                format!("/{}", path) // Add "/" at the start if not present
            } else {
                path.to_string()
            };
            if name.ends_with('/') && name != "/" {
                name.pop(); // Remove trailing '/' if present and not the root path
            }
            name
        };

        let shinkai_message = ShinkaiMessageBuilder::vecfs_available_shared_items(
            Some(formatted_path.to_string()),
            streamer_node_name,
            streamer_profile_name,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
            None,
        )?;

        let folder_creation_message = serde_json::json!(shinkai_message);
        let resp = request_post(
            self.node_address.clone(),
            folder_creation_message.to_string(),
            "/v1/available_shared_items",
        )
        .await;

        match resp {
            Ok(resp) => {
                // println!("Folder creation successful: {:?}", response);
                Ok(resp.data)
            }
            Err(e) => {
                eprintln!("Failed to get available shared items {:?}", e);
                Err("Failed to get available shared items")
            }
        }
    }

    pub async fn my_subscriptions(&self) -> Result<Value, &'static str> {
        let shinkai_message = ShinkaiMessageBuilder::my_subscriptions(
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )?;

        let folder_creation_message = serde_json::json!(shinkai_message);
        let resp = request_post(
            self.node_address.clone(),
            folder_creation_message.to_string(),
            "/v1/my_subscriptions",
        )
        .await;

        match resp {
            Ok(resp) => {
                // println!("Folder creation successful: {:?}", response);
                Ok(resp.data)
            }
            Err(_e) => Err("Failed to create folder"),
        }
    }
}
