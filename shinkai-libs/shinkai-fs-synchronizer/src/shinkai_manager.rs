use crate::{communication, persistent::Storage};
use ed25519_dalek::SigningKey;
use serde::Deserialize;
use shinkai_message_primitives::{
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::{
        encryption::{
            encryption_public_key_to_string, encryption_secret_key_to_string, string_to_encryption_public_key,
        },
        shinkai_message_builder::{ProfileName, ShinkaiMessageBuilder},
        signatures::{ephemeral_signature_keypair, signature_secret_key_to_string, string_to_signature_secret_key},
    },
};
use std::{convert::TryInto, fs};
use std::{env, fmt};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use hex::decode;
use libsodium_sys::*;
use std::str;

use shinkai_vector_resources::vector_resource::SimplifiedFSEntry;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NodeHealthStatus {
    pub is_pristine: bool,
    pub node_name: String,
    pub status: String,
    pub version: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct DeviceKeys {
    pub my_device_encryption_pk: String,
    pub my_device_encryption_sk: String,
    pub my_device_identity_pk: String,
    pub my_device_identity_sk: String,
    pub profile_encryption_pk: String,
    pub profile_encryption_sk: String,
    pub profile_identity_pk: String,
    pub profile_identity_sk: String,
    pub profile: String,
    pub identity_type: String,
    pub permission_type: String,
    pub shinkai_identity: String,
    pub registration_code: String,
    pub node_encryption_pk: String,
    pub node_address: String,
    pub registration_name: String,
    pub node_signature_pk: String,
}

impl DeviceKeys {
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

#[derive(Clone)]
pub struct ShinkaiManager {
    pub message_builder: ShinkaiMessageBuilder,
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub my_signature_secret_key: SigningKey,
    pub receiver_public_key: EncryptionPublicKey,
    pub sender: ProfileName,
    pub sender_subidentity: String,
    pub node_receiver: ProfileName,
    pub node_receiver_subidentity: ProfileName,
    pub profile_name: ProfileName,
    pub node_address: String,
}

impl fmt::Debug for ShinkaiManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShinkaiManager")
            .field("message_builder", &self.message_builder)
            // .field("my_encryption_secret_key", &self.my_encryption_secret_key)
            // .field("my_signature_secret_key", &self.my_signature_secret_key)
            // .field("receiver_public_key", &self.receiver_public_key)
            .field("sender", &self.sender)
            .field("sender_subidentity", &self.sender_subidentity)
            .field("node_receiver", &self.node_receiver)
            .field("node_receiver_subidentity", &self.node_receiver_subidentity)
            .field("profile_name", &self.profile_name)
            .finish()
    }
}

pub fn string_to_static_key(key_str: String) -> Result<EncryptionStaticKey, &'static str> {
    let key_bytes = hex::decode(key_str).map_err(|_| "Failed to decode hex string")?;
    let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| "Invalid key length")?;
    Ok(x25519_dalek::StaticSecret::from(key_array))
}

pub enum SynchronizerMode {
    Standalone,
    Library,
}

impl ShinkaiManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
        profile_name: ProfileName,
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
            profile_name,
            node_address,
        }
    }

    pub fn initialize_from_encrypted_file() -> Result<Self, &'static str> {
        let encryption_passphrase = env::var("ENCRYPTION_PASSPHRASE").expect("ENCRYPTION_PASSPHRASE must be set");
        let encrypted_keys = env::var("ENCRYPTED_KEYS").expect("ENCRYPTED_KEYS must be set");
        let decrypted_keys = Self::decrypt_exported_keys(&encrypted_keys, &encryption_passphrase)?;

        Ok(Self::initialize(decrypted_keys))
    }

    // for library implementation, just call this function and things get initialized
    pub fn initialize(keys: DeviceKeys) -> Self {
        let recipient = keys.shinkai_identity;
        let sender = recipient.clone();
        let sender_subidentity = keys.profile.to_string();

        let profile_name = keys.profile;

        let shinkai_manager = ShinkaiManager::new(
            string_to_static_key(keys.profile_encryption_sk).unwrap(),
            string_to_signature_secret_key(&keys.profile_identity_sk).unwrap(),
            string_to_encryption_public_key(&keys.node_encryption_pk).unwrap(),
            sender.clone(),
            sender_subidentity.clone(),
            sender,             // node_receiver
            sender_subidentity, // node_receiver_subidentity
            profile_name,
            keys.node_address,
        );

        shinkai_manager
    }

    pub async fn check_node_health(&self) -> Result<NodeHealthStatus, &'static str> {
        let shinkai_health_url = format!("{}/v1/shinkai_health", self.node_address);

        match reqwest::get(&shinkai_health_url).await {
            Ok(response) => {
                if response.status().is_success() {
                    let health_data: serde_json::Value =
                        response.json().await.expect("Failed to parse health check response");

                    let health_status: NodeHealthStatus = serde_json::from_value(health_data.clone())
                        .expect("Failed to parse health data into NodeHealthStatusPayload");

                    if health_status.status == "ok" {
                        println!("Shinkai node is healthy.");
                        Ok(health_status)
                    } else {
                        eprintln!("Shinkai node health check failed.");
                        Err("Shinkai node health check failed")
                    }
                } else {
                    eprintln!("Failed to reach Shinkai node for health check.");
                    Err("Failed to reach Shinkai node for health check")
                }
            }
            Err(e) => {
                eprintln!("Error verifying node health. Please check Node configuration and if all is fine, then Shinkai Node itself. \n{}", e);
                Err("Error verifying node health")
            }
        }
    }

    pub async fn get_node_folder(&mut self, path: &str) -> Result<SimplifiedFSEntry, &'static str> {
        let shinkai_message = ShinkaiMessageBuilder::vecfs_retrieve_path_simplified(
            path,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            "".to_string(),
        )?;

        let payload = serde_json::to_string(&shinkai_message).expect("Failed to serialize shinkai_message");
        let response = crate::communication::request_post(
            self.node_address.clone(),
            payload,
            "/v1/vec_fs/retrieve_path_simplified_json",
        )
        .await;

        let simplified_path_json_response = match response {
            Ok(data) => Ok(data.data),
            Err(e) => {
                eprintln!("Failed to retrieve node folder: {}", e);
                Err("Failed to retrieve node folder")
            }
        };

        match simplified_path_json_response {
            Ok(response) => {
                dbg!(&response);
                let fs_entry = SimplifiedFSEntry::from_json(&response.as_str().unwrap_or("")).unwrap();

                dbg!(&fs_entry);
                Ok(fs_entry)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn create_folder(&mut self, folder_name: &str, path: &str) -> Result<(), &'static str> {
        let shinkai_message = ShinkaiMessageBuilder::vecfs_create_folder(
            folder_name,
            path,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )?;

        let folder_creation_message = serde_json::json!(shinkai_message);
        let resp = crate::communication::request_post(
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
                eprintln!("Failed to create folder: {}", e);
                return Err("Failed to create folder");
            }
        }

        Ok(())
    }

    // TODO: how to delete folder with files on the node
    // fn delete_folder(&self, folder_name: &str, path: &str) -> Result<(), &'static str> {
    //     self.message_builder.vecfs_delete_folder(
    //         folder_name,
    //         path,
    //         self.my_encryption_secret_key.clone(),
    //         self.my_signature_secret_key.clone(),
    //         self.receiver_public_key.clone(),
    //         self.sender.clone(),
    //         self.sender_subidentity.clone(),
    //         self.node_receiver.clone(),
    //         self.node_receiver_subidentity.clone(),
    //     )?;

    //     Ok(())
    // }

    pub async fn upload_file(&self, file_bytes: &[u8], destination_path: &str) -> Result<(), &'static str> {
        // TODO: add missing pieces here

        // Prepare the file data
        // let file_data = encrypted_file_data; // In Rust, Vec<u8> can be used directly

        // let form_data = multipart::Form::new()
        //     .file("file", file_data, destination_path)
        //     .map_err(|_| "Failed to create form data")?;

        // let url = format!(
        //     "{}/v1/add_file_to_inbox_with_symmetric_key/{}/{}",
        //     self.base_url, hash, nonce_str
        // );

        // TODO: add http service that communicates with the node api
        // self.http_service
        //     .fetch(&url, form_data)
        //     .await
        //     .map_err(|_| "HTTP request failed")?;

        Ok(())
    }

    fn add_items_to_db(&mut self, destination_path: &str, file_inbox: &str) -> Result<(), &'static str> {
        ShinkaiMessageBuilder::vecfs_create_items(
            destination_path,
            file_inbox,
            self.my_encryption_secret_key.clone(),
            self.my_signature_secret_key.clone(),
            self.receiver_public_key,
            self.sender.clone(),
            self.sender_subidentity.clone(),
            self.node_receiver.clone(),
            self.node_receiver_subidentity.clone(),
        )?;

        Ok(())
    }

    async fn decode_message(&self, message: ShinkaiMessage) -> String {
        let decrypted_message = message
            .decrypt_outer_layer(&self.my_encryption_secret_key, &self.receiver_public_key)
            .expect("Failed to decrypt body content");

        let content = decrypted_message.get_message_content().unwrap();

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();
        content.to_string()
    }

    pub fn decrypt_exported_keys(encrypted_body: &str, passphrase: &str) -> Result<DeviceKeys, &'static str> {
        unsafe {
            if libsodium_sys::sodium_init() == -1 {
                return Err("Failed to initialize libsodium");
            }

            if !encrypted_body.starts_with("encrypted:") {
                return Err("Unexpected variant");
            }

            let content = &encrypted_body["encrypted:".len()..];
            let salt_hex = &content[..32];
            let nonce_hex = &content[32..56];
            let ciphertext_hex = &content[56..];

            let salt = decode(salt_hex).map_err(|_| "Failed to decode salt")?;
            let nonce = decode(nonce_hex).map_err(|_| "Failed to decode nonce")?;
            let ciphertext = decode(ciphertext_hex).map_err(|_| "Failed to decode ciphertext")?;

            let mut key = vec![0u8; 32];

            let pwhash_result = crypto_pwhash(
                key.as_mut_ptr(),
                key.len() as u64,
                passphrase.as_ptr() as *const i8,
                passphrase.len() as u64,
                salt.as_ptr(),
                crypto_pwhash_OPSLIMIT_INTERACTIVE as u64,
                crypto_pwhash_MEMLIMIT_INTERACTIVE as usize,
                crypto_pwhash_ALG_DEFAULT as i32,
            );

            if pwhash_result != 0 {
                return Err("Key derivation failed");
            }

            let mut decrypted_data = vec![0u8; ciphertext.len() - crypto_aead_chacha20poly1305_IETF_ABYTES as usize];
            let mut decrypted_len = 0u64;

            let decryption_result = crypto_aead_chacha20poly1305_ietf_decrypt(
                decrypted_data.as_mut_ptr(),
                &mut decrypted_len,
                std::ptr::null_mut(),
                ciphertext.as_ptr(),
                ciphertext.len() as u64,
                std::ptr::null(),
                0,
                nonce.as_ptr() as *const u8,
                key.as_ptr(),
            );
            if decryption_result != 0 {
                return Err("Decryption failed");
            }

            decrypted_data.truncate(decrypted_len as usize);
            let decrypted_str = String::from_utf8(decrypted_data).map_err(|_| "Failed to decode decrypted data")?;
            serde_json::from_str(&decrypted_str).map_err(|_| "Failed to parse decrypted data into DeviceKeys")
        }
    }
}
