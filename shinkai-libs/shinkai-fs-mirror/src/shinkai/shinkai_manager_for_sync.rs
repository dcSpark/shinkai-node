use std::fs;
use std::path::Path;

use crate::http_requests::{request_post, request_post_multipart, PostDataResponse, PostRequestError, PostStringResponse};
use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
};
use shinkai_message_primitives::shinkai_utils::{
    encryption::{string_to_encryption_public_key, string_to_encryption_static_key},
    file_encryption::{aes_encryption_key_to_string, random_aes_encryption_key},
    shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString},
    signatures::string_to_signature_secret_key,
};
use shinkai_vector_resources::vector_resource::SimplifiedFSEntry;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::shinkai_utils::decrypt_exported_keys;
use super::{shinkai_device_keys::ShinkaiDeviceKeys, shinkai_response_types::NodeHealthStatus};

#[derive(Clone)]
pub struct ShinkaiManagerForSync {
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

impl ShinkaiManagerForSync {
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

        ShinkaiManagerForSync::new(
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
            Err(e) => Err("Error verifying node health"),
        }
    }

    pub async fn upload_file(
        &self,
        file_data: &[u8],
        filename: &str,
        destination: &str,
    ) -> Result<(), PostRequestError> {
        let destination = if destination.starts_with("./") {
            &destination[1..] // Skip the first character and use the rest of the string
        } else {
            destination
        };
        
        eprintln!(
            "Uploading file: {} to node address: {} with destination: {}",
            filename,
            self.node_address.clone(),
            destination
        );
        let symmetrical_sk = random_aes_encryption_key();

        // Create inbox where to upload file
        let shinkai_message = ShinkaiMessageBuilder::create_files_inbox_with_sym_key(
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            "".to_string(),
            aes_encryption_key_to_string(symmetrical_sk),
            self.sender_subidentity.clone(),
            self.sender.clone(),
            self.node_receiver.clone(),
        )
        .unwrap();

        let inbox_message_creation = serde_json::json!(shinkai_message);
        request_post(
            self.node_address.clone(),
            inbox_message_creation.to_string(),
            "/v1/create_files_inbox_with_symmetric_key",
        )
        .await
        .map_err(|e| PostRequestError::RequestFailed(format!("HTTP request failed with err: {:?}", e)))?;

        // Encrypt file and upload it
        let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);
        let nonce_slice = nonce.as_slice();
        let nonce_str = aes_nonce_to_hex_string(nonce_slice);
        let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(ciphertext).file_name(filename.to_string()),
        );

        let url = format!(
            "/v1/add_file_to_inbox_with_symmetric_key/{}/{}",
            hash_of_aes_encryption_key_hex(symmetrical_sk),
            nonce_str
        );

        request_post_multipart(self.node_address.clone(), &url, form)
            .await
            .map_err(|e| PostRequestError::RequestFailed(format!("Multipart HTTP request failed with err: {:?}", e)))?;

        // Process message
        let shinkai_message = ShinkaiMessageBuilder::vecfs_create_items(
            destination,
            &hash_of_aes_encryption_key_hex(symmetrical_sk),
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )
        .unwrap();

        let message_creation = serde_json::json!(shinkai_message);
        request_post(
            self.node_address.clone(),
            message_creation.to_string(),
            "/v1/vec_fs/convert_files_and_save_to_folder",
        )
        .await
        .map_err(|e| PostRequestError::RequestFailed(format!("Convert File HTTP request failed with err: {:?}", e)))?;

        Ok(())
    }

    // Need review
    pub async fn get_node_folder(&self, path: &str) -> Result<SimplifiedFSEntry, PostRequestError> {
        let formatted_path = if path.starts_with("/") {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        println!("Checking {} in vector FS using vecfs_retrieve_path_simplified", &path);
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
        .unwrap();

        // TODO: handle unwrap()

        let payload = serde_json::to_string(&shinkai_message).expect("Failed to serialize shinkai_message");
        let response = request_post(
            self.node_address.clone(),
            payload,
            "/v1/vec_fs/retrieve_path_simplified_json",
        )
        .await;

        let simplified_path_json_response = match response {
            Ok(data) => Ok(data.data),
            Err(e) => Err(e),
        };

        match simplified_path_json_response {
            Ok(response) => {
                let fs_entry = SimplifiedFSEntry::from_json(response.as_str().unwrap_or("")).unwrap();
                Ok(fs_entry)
            }
            Err(e) => Err(e),
        }
    }

    // Need review
    pub async fn create_folder(&self, folder_name: &str, path: &str) -> Result<(), &'static str> {
        let formatted_path = if path == "/" {
            path.to_string()
        } else {
            let mut name = if !path.starts_with("/") {
                format!("/{}", path) // Add "/" at the start if not present
            } else {
                path.to_string()
            };
            if name.ends_with("/") && name != "/" {
                name.pop(); // Remove trailing '/' if present and not the root path
            }
            name
        };

        println!(
            "Creating folder: {} in path: {}",
            &folder_name.to_string(),
            &formatted_path
        );
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
            Ok(response) => {
                println!("Folder creation successful: {:?}", response);
            }
            Err(e) => {
                return Err("Failed to create folder");
            }
        }

        Ok(())
    }

    pub async fn retrieve_vector_resource(&self, path: &str) -> Result<PostDataResponse, PostRequestError> {
        let formatted_path = if path.starts_with("/") {
            path.to_string()
        } else {
            format!("/{}", path)
        };
    
        println!("Retrieving vector resource for path: {}", &formatted_path);
        let shinkai_message = ShinkaiMessageBuilder::vecfs_retrieve_resource(
            &formatted_path,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        ).unwrap(); // Consider handling this unwrap more gracefully
    
        let retrieve_resource_message = serde_json::json!(shinkai_message);
        let response = request_post(
            self.node_address.clone(),
            retrieve_resource_message.to_string(),
            "/v1/vec_fs/retrieve_vector_resource",
        )
        .await;
    
        match response {
            Ok(resp) => {
                // println!("Vector resource retrieval successful: {:?}", resp);
                Ok(resp)
            },
            Err(e) => {
                eprintln!("Failed to retrieve vector resource: {:?}", e);
                Err(PostRequestError::RequestFailed(format!("Failed to retrieve vector resource: {:?}", e)))
            }
        }
    }
}
