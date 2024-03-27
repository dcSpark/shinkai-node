pub mod communication;
pub mod persistent;
pub mod shinkai_manager;
pub mod synchronizer;

use crate::shinkai_manager::ShinkaiManager;
use crate::synchronizer::FilesystemSynchronizer;
use dotenv::dotenv;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

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
        // (re)initialize sync_visitor with the new state of syncing_folders at the beginning of each loop iteration
        // Update the synchronizer with the new state of syncing_folders
        let mut synchronizer = FilesystemSynchronizer::new(shinkai_manager.clone());

        // TODO: move out to separate function - triggers recursive visit_dirs which traverses folders and builds syncingFolders hashmap
        let major_directory_path = Path::new(major_directory);
        if major_directory_path.is_dir() {
            match synchronizer.visit_dirs(major_directory_path) {
                Ok(_) => println!("SyncingFolders initialization completed."),
                Err(e) => println!("Error during traversal: {}", e),
            }
        } else {
            println!("The provided path is not a directory.");
        }

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
