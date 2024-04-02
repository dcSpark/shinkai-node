// Start:
// Scan current folder and files in specified folder
// Request Node for current state of its folder
// Save to memory both states
// Calculate difference
// Send to Node the difference
// Concurrent:
// Have an event watcher seeing for changes in the folder
// Update Sync state if anything changes
// if a sync process is not outgoing, start it
// if a sync progress is outgoing, do nothing

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::AbortHandle;

use crate::http_requests::PostRequestError;
use crate::persistence::{ShinkaiMirrorDB, ShinkaiMirrorDBError};
use crate::shinkai::shinkai_manager_for_sync::ShinkaiManagerForSync;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncingFolder {
    local_last_synchronized_file_datetime: SystemTime,
}

pub struct FilesystemSynchronizer {
    pub abort_handler: AbortHandle,
    pub shinkai_manager_for_sync: ShinkaiManagerForSync,
    pub folder_to_watch: PathBuf,
    pub destination_path: PathBuf,
    pub profile_name: String,
    pub syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    pub sync_thread_handle: Option<thread::JoinHandle<()>>,
}

impl std::fmt::Debug for FilesystemSynchronizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilesystemSynchronizer")
            .field("folder_to_watch", &self.folder_to_watch)
            .field("destination_path", &self.destination_path)
            .field("profile_name", &self.profile_name)
            .finish()
    }
}

impl FilesystemSynchronizer {
    pub async fn new(
        shinkai_manager_for_sync: ShinkaiManagerForSync,
        folder_to_watch: PathBuf,
        destination_path: PathBuf,
        db_path: String,
        sync_interval_min: Option<Duration>,
    ) -> Result<Self, ShinkaiMirrorDBError> {
        let db = ShinkaiMirrorDB::new(&db_path)?;
        let syncing_folders_db = Arc::new(Mutex::new(db));
        let profile_name = shinkai_manager_for_sync.sender_subidentity.clone();

        let profile_name_clone = profile_name.clone();
        let shinkai_manager_clone = shinkai_manager_for_sync.clone();
        let folder_to_watch_clone = folder_to_watch.clone();
        let syncing_folders_db_clone = syncing_folders_db.clone();
        let destination_clone = destination_path.clone();

        let handle = tokio::spawn(async move {
            let sync_interval = sync_interval_min.unwrap_or_else(|| Duration::from_secs(60 * 5));

            loop {
                eprintln!("Syncing folders");
                let result = Self::process_updates(
                    &shinkai_manager_clone,
                    &folder_to_watch_clone,
                    &profile_name_clone,
                    &destination_clone,
                    syncing_folders_db_clone.clone(),
                )
                .await;
                eprintln!("Syncing folders finished. Result: {:?}", result);

                // Sleep until the next iteration
                std::thread::sleep(sync_interval);
            }
        });

        Ok(FilesystemSynchronizer {
            profile_name,
            shinkai_manager_for_sync,
            folder_to_watch,
            destination_path,
            syncing_folders_db,
            sync_thread_handle: None,
            abort_handler: handle.abort_handle(),
        })
    }

    pub async fn scan_folders_and_calculate_difference(
        folder_to_watch: &PathBuf,
        profile_name: &str,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    ) -> Vec<PathBuf> {
        let current_folder_files = Self::scan_folders(folder_to_watch);
        let mut files_to_update = Vec::new();

        for (path, modified_time) in current_folder_files {
            let syncing_folders = syncing_folders_db.lock().await;
            // Use get_file_mirror_state to check if the file exists in the database
            match syncing_folders.get_file_mirror_state(profile_name.to_string(), path.clone()) {
                Ok(Some(syncing_folder)) => {
                    // If the file exists and the modified time is greater than the last synchronized time, add it to the update list
                    if modified_time > syncing_folder.local_last_synchronized_file_datetime {
                        files_to_update.push(path);
                    }
                }
                Ok(None) => {
                    // If the file does not exist in the database, add it to the update list
                    files_to_update.push(path);
                }
                Err(e) => {
                    // Handle potential errors, for example, log them or push them to an error list
                    eprintln!("Error accessing database for file {:?}: {}", path, e);
                }
            }
        }
        eprintln!("scan_folders_and_calculate_difference> {:?}", files_to_update);

        files_to_update
    }

    pub fn scan_folders(folder_to_watch: &PathBuf) -> HashMap<PathBuf, SystemTime> {
        let mut folder_files = HashMap::new();
        let paths = std::fs::read_dir(folder_to_watch).expect("Could not read directory");

        for path in paths {
            let path = path.expect("Could not read path").path();
            if path.is_dir() {
                let inner_paths = std::fs::read_dir(&path).expect("Could not read inner directory");
                for inner_path in inner_paths {
                    let inner_path = inner_path.expect("Could not read inner path").path();
                    if let Ok(metadata) = inner_path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            folder_files.insert(inner_path, modified);
                        }
                    }
                }
            } else if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    folder_files.insert(path, modified);
                }
            }
        }
        eprintln!("scan_folders> {:?}", folder_files);

        folder_files
    }

    pub async fn upload_files(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
        files: Vec<PathBuf>,
        profile_name: &str,
        destination_path: &PathBuf,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    ) -> Result<(), PostRequestError> {
        // Adjusted return type
        eprintln!("upload_files> {:?}", files);
        for file_path in files {
            let file_data = std::fs::read(&file_path)
                .map_err(|_| PostRequestError::FSFolderNotFound("Failed to read file data".into()))?;
            let filename = file_path
                .file_name()
                .ok_or(PostRequestError::Unknown("Failed to extract filename".into()))?
                .to_str()
                .ok_or(PostRequestError::Unknown("Failed to convert filename to string".into()))?;

            // Construct the destination PathBuf
            let destination_buf = destination_path.join(file_path.strip_prefix(destination_path).unwrap_or(&file_path));
            // Extract the directory part of the destination PathBuf, removing the filename
            let destination_dir_buf = destination_buf.parent().unwrap_or(&destination_buf);
            // Convert PathBuf to a string slice
            let mut destination_str = destination_dir_buf.to_string_lossy().into_owned();

            // Remove leading '.' if it exists
            if destination_str.starts_with('.') {
                destination_str.remove(0);
            }

            let path_components: Vec<&str> = destination_str.split('/').filter(|c| !c.is_empty()).collect();
            let mut current_path = String::new();

            for (index, component) in path_components.iter().enumerate() {
                if index > 0 {
                    current_path.push('/');
                }
                current_path.push_str(component);

                let folder_check_path = if index == 0 {
                    format!("/{}", current_path)
                } else {
                    current_path.clone()
                };

                match shinkai_manager_for_sync.get_node_folder(&folder_check_path).await {
                    Ok(_) => eprintln!("Folder exists: {}", folder_check_path),
                    Err(_) => {
                        let create_folder_path = if index == 0 {
                            "/".to_string()
                        } else {
                            current_path[..current_path.rfind('/').unwrap_or(0)].to_string()
                        };

                        eprintln!(
                            "Folder does not exist, creating: {} in {}",
                            component, create_folder_path
                        );
                        if let Err(e) = shinkai_manager_for_sync
                            .create_folder(component, &create_folder_path)
                            .await
                        {
                            eprintln!("Failed to create folder: {:?}, error: {}", folder_check_path, e);
                            return Err(PostRequestError::Unknown(format!(
                                "Failed to create folder: {}",
                                folder_check_path
                            )));
                        }
                    }
                }
            }

            // Attempt to upload the file, only proceed if successful
            let upload_result = shinkai_manager_for_sync
                .upload_file(&file_data, filename, &destination_str)
                .await;
            if let Ok(_) = upload_result {
                // Update the last synchronized file datetime in syncing_folders_db
                let parent_path = file_path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                let mut db = syncing_folders_db.lock().await;
                let syncing_folder = SyncingFolder {
                    local_last_synchronized_file_datetime: SystemTime::now(),
                };
                // Use add_or_update_file_mirror_state to update the database
                if let Err(_) =
                    db.add_or_update_file_mirror_state(profile_name.to_string(), parent_path, syncing_folder)
                {
                    eprintln!("Failed to update file mirror state");
                    return Err(PostRequestError::Unknown("Failed to update file mirror state".into()));
                }
            } else if let Err(e) = upload_result {
                // If an error occurs during file upload, return the error
                eprintln!("Failed to upload file: {:?}", e);
                return Err(e);
            }
        }

        eprintln!("upload_files> Done");
        Ok(())
    }

    fn stop(self) {
        self.abort_handler.abort();
    }

    pub async fn process_updates(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
        folder_to_watch: &PathBuf,
        profile_name: &str,
        destination_path: &PathBuf,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    ) -> Result<(), PostRequestError> {
        // Updated return type
        // Check the health of the external service before proceeding
        match shinkai_manager_for_sync.check_node_health().await {
            Ok(health_status) => {
                println!("Node health check passed: {:?}", health_status);
                // Proceed with the updates if the health check is successful
                let files_to_update = Self::scan_folders_and_calculate_difference(
                    folder_to_watch,
                    profile_name,
                    syncing_folders_db.clone(),
                )
                .await;
                Self::upload_files(
                    shinkai_manager_for_sync,
                    files_to_update,
                    profile_name,
                    destination_path,
                    syncing_folders_db,
                )
                .await
            }
            Err(health_check_error) => {
                // Handle the case where the health check fails
                eprintln!("Node health check failed: {}", health_check_error);
                Err(PostRequestError::Unknown(format!(
                    "Node health check failed: {}",
                    health_check_error
                ))) // Adjusted to use PostRequestError
            }
        }
    }

    // For later:
    // {
    //     "name": "Zeko_Mina_Rollup",
    //     "path": "/test_folder/Zeko_Mina_Rollup",
    //     "vr_header": {
    //       "resource_name": "Zeko_Mina_Rollup",
    //       "resource_id": "dbd162851a56481c1b376d6be505f8d4365c03b860e1d317796915c1c2ccaa0f",
    //       "resource_base_type": "Document",
    //       "resource_source": {
    //         "Reference": {
    //           "FileRef": {
    //             "file_name": "files/Zeko_Mina_Rollup",
    //             "file_type": {
    //               "Document": "Pdf"
    //             },
    //             "original_creation_datetime": null,
    //             "text_chunking_strategy": "V1"
    //           }
    //         }
    //       },
    //       "resource_embedding": null,
    //       "resource_created_datetime": "2024-04-02T02:20:31.292269Z",
    //       "resource_last_written_datetime": "2024-04-02T02:20:46.551353Z",
    //       "resource_embedding_model_used": {
    //         "TextEmbeddingsInference": "AllMiniLML6v2"
    //       },
    //       "resource_merkle_root": "7597614731185beae509021556fff2f7ff86d12518e40d7435ed262fc5c5acd1",
    //       "resource_keywords": {
    //         "keyword_list": [],
    //         "keywords_embedding": null
    //       },
    //       "resource_distribution_info": {
    //         "origin": null,
    //         "release_datetime": null
    //       },
    //       "data_tag_names": [],
    //       "metadata_index_keys": [
    //         "page_numbers"
    //       ]
    //     },
    //     "created_datetime": "2024-04-02T02:20:31.292269Z",
    //     "last_written_datetime": "2024-04-02T02:20:46.551353Z",
    //     "last_read_datetime": "2024-04-02T02:20:46.695135Z",
    //     "vr_last_saved_datetime": "2024-04-02T02:20:46.553970Z",
    //     "source_file_map_last_saved_datetime": "2024-04-02T02:20:46.553970Z",
    //     "distribution_info": {
    //       "origin": null,
    //       "release_datetime": null
    //     },
    //     "vr_size": 2482698,
    //     "source_file_map_size": 908844,
    //     "merkle_hash": "7597614731185beae509021556fff2f7ff86d12518e40d7435ed262fc5c5acd1"
    //   }
}
