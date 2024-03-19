use ed25519_dalek::SigningKey;
use reqwest::{Client, Error};
use serde::{Deserialize, Serialize};

use crate::shinkai_manager::ShinkaiManager;
use crate::synchronizer::FilesystemSynchronizer;
use dotenv::dotenv;
use std::collections::HashMap;
use std::env;

use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostDataResponse {
    pub status: String,
    pub data: serde_json::Value,
}

pub async fn request_post(input: String, path: &str) -> Result<PostDataResponse, String> {
    let client = Client::new();
    let shinkai_node_url = env::var("SHINKAI_NODE_URL").expect("SHINKAI_NODE_URL must be set");
    let url = format!("{}{}", shinkai_node_url, path);

    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(input)
        .send()
        .await
    {
        Ok(response) => match response.json::<PostDataResponse>().await {
            Ok(data) => {
                dbg!(data.clone());
                Ok(data)
            }
            Err(e) => {
                eprintln!("Error parsing response: {:?}", e);
                Err(format!("Error parsing response: {:?}", e))
            }
        },
        Err(e) => {
            eprintln!("Error when interacting with {}. Error: {:?}", path, e);
            Err(format!("Error when interacting with {}. Error: {:?}", path, e))
        }
    }
}

async fn generate_encryption_keys() -> (EncryptionStaticKey, EncryptionPublicKey) {
    let seed = rand::rngs::OsRng;
    let secret_key = EncryptionStaticKey::new(seed);
    let public_key = EncryptionPublicKey::from(&secret_key);
    (secret_key, public_key)
}

async fn generate_signature_keys() -> (x25519_dalek::StaticSecret, SigningKey) {
    let mut csprng = rand::rngs::OsRng;
    let secret_key = x25519_dalek::StaticSecret::new(&mut csprng);
    let signing_key = SigningKey::generate(&mut csprng);
    (secret_key, signing_key)
}

pub async fn node_init() -> ShinkaiManager {
    let (my_device_encryption_sk, my_device_encryption_pk) = generate_encryption_keys().await;
    let (my_device_signature_sk, my_device_signing_key) = generate_signature_keys().await;

    let (profile_encryption_sk, profile_encryption_pk) = generate_encryption_keys().await;
    let (profile_signature_sk, profile_signing_key) = generate_signature_keys().await;

    let sender = env::var("PROFILE_NAME").expect("PROFILE_NAME must be set");
    let sender_subidentity = env::var("DEVICE_NAME").expect("DEVICE_NAME must be set");
    let receiver = env::var("PROFILE_NAME").expect("PROFILE_NAME must be set");

    let mut shinkai_manager: Option<ShinkaiManager> = None;

    loop {
        let check_health = ShinkaiManager::check_node_health().await;
        if check_health.is_ok() {
            match ShinkaiManager::initialize_node_connection(
                my_device_encryption_sk.clone(),
                my_device_signing_key.clone(),
                profile_encryption_sk.clone(),
                profile_signing_key.clone(),
                "registration_name".to_string(),
                sender_subidentity,
                sender,
                receiver,
            )
            .await
            {
                Ok(manager) => {
                    shinkai_manager = Some(manager);
                }
                Err(e) => {
                    eprintln!("Failed to initialize node connection: {}", e);
                }
            }
            break;
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
    shinkai_manager.unwrap()
}
