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
use visitor::{traverse_and_initialize_local_state, SyncFolderVisitor};

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

    const MAX_RETRIES: u32 = 3;
    let mut attempts = 0;

    loop {
        // get last synced timestamp - if none provided, then assign None
        let last_synced_time = None;

        // (re)initialize sync_visitor with the new state of syncing_folders at the beginning of each loop iteration
        let syncing_folders = Arc::new(Mutex::new(HashMap::new()));
        let sync_visitor = SyncFolderVisitor::new(syncing_folders.clone(), last_synced_time);
        traverse_and_initialize_local_state::<(), SyncFolderVisitor>(knowledge_dir.to_str().unwrap(), &sync_visitor);

        // fetch last saved persistent state from the disk

        // fetch last saved persistent state from the node

        // Update the synchronizer with the new state of syncing_folders
        let synchronizer = FilesystemSynchronizer::new(
            shinkai_manager.clone(),
            sync_visitor.syncing_folders.clone(),
            major_directory.to_string(),
        );

        attempts += 1;
        match synchronizer.synchronize().await {
            Ok(_) => {
                println!("Synchronization successful. Waiting for the next synchronization cycle...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            Err(e) => {
                eprintln!("Failed to synchronize on attempt {}: {}", attempts, e);
                if attempts >= MAX_RETRIES {
                    eprintln!("Reached maximum retry limit. Aborting.");
                    break;
                }
                // Check node health before retrying
                let mut node_health_check_passed = false;
                while !node_health_check_passed {
                    match shinkai_manager.check_node_health().await {
                        Ok(_) => {
                            println!("Node health check passed. Proceeding to retry synchronization.");
                            node_health_check_passed = true;
                        }
                        Err(health_check_error) => {
                            eprintln!(
                                "Node health check failed: {}. Retrying node health check...",
                                health_check_error
                            );
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        }
                    }
                }
                // Retry synchronization after node health check passes
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}
