pub mod communication;
pub mod shinkai_manager;
pub mod synchronizer;
pub mod visitor;

use crate::shinkai_manager::ShinkaiManager;
use crate::synchronizer::FilesystemSynchronizer;
use dotenv::dotenv;
use std::collections::HashMap;
use std::env;

use ed25519_dalek::SigningKey;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[tokio::main]
async fn main() {
    dotenv().ok();
    let major_directory = "knowledge";

    // TODO: move initialization code to a separate function
    // let my_encryption_secret_key = EncryptionStaticKey::new(rand::rngs::OsRng);
    // let my_signature_secret_key = SigningKey::from_bytes(&[0; 32]);
    // let receiver_public_key = EncryptionPublicKey::from([0; 32]);

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

    let syncing_folders = HashMap::new();
    let _synchronizer = FilesystemSynchronizer::new(shinkai_manager.unwrap(), syncing_folders);

    // synchronizer.traverse_and_synchronize(major_directory);

    // // Add a syncing folder
    // let folder = SyncingFolder {
    //     profile_name: "profile1".to_string(),
    //     vector_fs_path: "vector/path".to_string(),
    //     local_os_folder_path: "local/path".to_string(),
    //     last_synchronized_file_datetime: "2021-07-21T15:00:00.000Z".to_string(),
    // };
    // synchronizer
    //     .add_syncing_folder("local/path".to_string(), folder)
    //     .unwrap();

    // // Get current syncing folders map
    // let current_folders = synchronizer.get_current_syncing_folders_map();
    // println!("{:?}", current_folders);

    // // Stop the synchronizer
    // let final_folders = synchronizer.stop();
    // println!("{:?}", final_folders);
}
