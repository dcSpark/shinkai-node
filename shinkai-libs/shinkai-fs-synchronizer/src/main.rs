pub mod shinkai_manager;
pub mod synchronizer;

use crate::shinkai_manager::ShinkaiManager;
use crate::synchronizer::FilesystemSynchronizer;
use crate::synchronizer::SyncingFolder;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use ed25519_dalek::SigningKey;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[tokio::main]
async fn main() {
    let syncing_folders = HashMap::new();
    let major_directory = "knowledge";

    // TODO: remove exemplary initialization and implement auto connecting to the node
    let my_encryption_secret_key = EncryptionStaticKey::new(rand::rngs::OsRng);
    let my_signature_secret_key = SigningKey::from_bytes(&[0; 32]);
    let receiver_public_key = EncryptionPublicKey::from([0; 32]);

    let shinkai_message_builder = ShinkaiMessageBuilder::new(
        my_encryption_secret_key.clone(),
        my_signature_secret_key.clone(),
        receiver_public_key,
    );
    let shinkai_manager = ShinkaiManager::new(
        shinkai_message_builder,
        my_encryption_secret_key,
        my_signature_secret_key,
        receiver_public_key,
        ProfileName::default(),
        String::default(),
        "".to_string(),
        "".to_string(),
        String::default(),
    );

    let mut synchronizer = FilesystemSynchronizer::new(shinkai_manager, syncing_folders);

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
