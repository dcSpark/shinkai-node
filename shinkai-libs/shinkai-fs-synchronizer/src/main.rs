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
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

fn main() {
    let client_keypairs = vec!["keypair1".to_string(), "keypair2".to_string()];
    let syncing_folders = HashMap::new();
    let major_directory = "knowledge";

    let shinkai_manager = ShinkaiManager::new(
        EncryptionStaticKey::new(rand::rngs::OsRng),
        SigningKey::from_bytes(&[0; 32]),
        EncryptionPublicKey::from([0; 32]),
        ProfileName::default(),
        String::default(),
        "".to_string(),
        "".to_string(),
    );
    let mut synchronizer =
        FilesystemSynchronizer::new(shinkai_manager, major_directory, client_keypairs, syncing_folders);

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
