pub mod communication;
pub mod persistent;
pub mod shinkai_manager;
pub mod synchronizer;
pub mod visitor;

use crate::shinkai_manager::ShinkaiManager;
use crate::synchronizer::FilesystemSynchronizer;
use dotenv::dotenv;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};
use visitor::{traverse_and_synchronize, SyncFolderVisitor};

// TODO: move all envs to configuration variables initialized with custom values/yaml or default values

// here we initialize standalone version
#[tokio::main]
async fn main() {
    dotenv().ok();

    let env_file_path = dotenv::dotenv().ok().map(|_| dotenv::from_filename(".env").ok());
    if let Some(path) = env_file_path {
        println!("Environment file loaded from: {:?}", path);
    } else {
        println!("No .env file found or failed to load.");
    }

    let major_directory = "knowledge/";
    let knowledge_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(major_directory);

    let shinkai_manager = match ShinkaiManager::initialize_from_encrypted_file() {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!("Failed to initialize node: {}", e);
            return;
        }
    };

    loop {
        match shinkai_manager.check_node_health().await {
            Ok(_) => {
                println!("Node health check passed.");
                break;
            }
            Err(e) => {
                eprintln!("Node health check failed: {}. Retrying...", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }

    let syncing_folders = Arc::new(Mutex::new(HashMap::new()));
    let sync_visitor = SyncFolderVisitor::new(syncing_folders);
    traverse_and_synchronize::<(), SyncFolderVisitor>(knowledge_dir.to_str().unwrap(), &sync_visitor);

    let synchronizer = FilesystemSynchronizer::new(
        shinkai_manager,
        sync_visitor.syncing_folders,
        major_directory.to_string(),
    );

    const MAX_RETRIES: u32 = 3;
    let mut attempts = 0;

    loop {
        attempts += 1;
        match synchronizer.synchronize().await {
            Ok(_) => {
                println!("Synchronization successful.");
                break;
            }
            Err(e) => {
                eprintln!("Failed to synchronize on attempt {}: {}", attempts, e);
                if attempts >= MAX_RETRIES {
                    eprintln!("Reached maximum retry limit. Aborting.");
                    break;
                }
                // TODO: implement a backoff strategy or a delay before retrying
                // for now keep constant max retries and constant time to keep things simple for now
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}
