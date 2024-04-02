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
use std::thread;
use std::time::{Duration, SystemTime};

use dashmap::DashMap;
use tokio::task::AbortHandle;

use crate::shinkai::shinkai_manager_for_sync::ShinkaiManagerForSync;

#[derive(Clone, Debug)]
pub struct SyncingFolder {
    local_last_synchronized_file_datetime: SystemTime,
}

pub struct FilesystemSynchronizer {
    pub abort_handler: AbortHandle,
    pub shinkai_manager_for_sync: ShinkaiManagerForSync,
    pub folder_to_watch: PathBuf,
    pub destination_path: PathBuf,
    pub syncing_folders: DashMap<PathBuf, SyncingFolder>,
    pub sync_thread_handle: Option<thread::JoinHandle<()>>,
}

impl FilesystemSynchronizer {
    pub async fn new(
        shinkai_manager_for_sync: ShinkaiManagerForSync,
        folder_to_watch: PathBuf,
        destination_path: PathBuf,
        syncing_folders_restore: HashMap<PathBuf, SyncingFolder>,
        sync_interval_min: Option<Duration>,
    ) -> Self {
        let syncing_folders_clone = DashMap::new();
        for (key, value) in syncing_folders_restore {
            syncing_folders_clone.insert(key, value);
        }

        let shinkai_manager_clone = shinkai_manager_for_sync.clone();
        let folder_to_watch_clone = folder_to_watch.clone();
        let syncing_folders_clone_thread = syncing_folders_clone.clone();

        let handle = tokio::spawn(async move {
            let sync_interval = sync_interval_min.unwrap_or_else(|| Duration::from_secs(60 * 5));

            loop {
                let _ = Self::process_updates(
                    &shinkai_manager_clone,
                    &folder_to_watch_clone,
                    &syncing_folders_clone_thread,
                )
                .await;

                // Sleep until the next iteration
                std::thread::sleep(sync_interval);
            }
        });

        FilesystemSynchronizer {
            shinkai_manager_for_sync,
            folder_to_watch,
            destination_path,
            syncing_folders: syncing_folders_clone,
            sync_thread_handle: None,
            abort_handler: handle.abort_handle(),
        }
    }

    pub fn scan_folders_and_calculate_difference(
        folder_to_watch: &PathBuf,
        syncing_folders: &DashMap<PathBuf, SyncingFolder>,
    ) -> Vec<PathBuf> {
        let current_folder_files = Self::scan_folders(folder_to_watch);
        let mut files_to_update = Vec::new();

        for (path, modified_time) in current_folder_files {
            if let Some(syncing_folder) = syncing_folders.get(&path) {
                if modified_time > syncing_folder.local_last_synchronized_file_datetime {
                    files_to_update.push(path);
                }
            }
        }

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

        folder_files
    }

    pub async fn upload_files(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
        files: Vec<PathBuf>,
        syncing_folders: &DashMap<PathBuf, SyncingFolder>,
    ) -> Result<(), &'static str> {
        for file_path in files {
            let file_data = std::fs::read(&file_path).map_err(|_| "Failed to read file data")?;
            let filename = file_path
                .file_name()
                .ok_or("Failed to extract filename")?
                .to_str()
                .ok_or("Failed to convert filename to string")?;

            // Attempt to upload the file, only proceed if successful
            if shinkai_manager_for_sync.upload_file(&file_data, filename).await.is_ok() {
                // Update the last synchronized file datetime in syncing_folders
                let parent_path = file_path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                if let Some(mut syncing_folder) = syncing_folders.get_mut(&parent_path) {
                    syncing_folder.local_last_synchronized_file_datetime = SystemTime::now();
                }
            } else {
                // If an error occurs during file upload, return the error
                return Err("Failed to upload file");
            }
        }

        Ok(())
    }

    fn stop(self) -> HashMap<PathBuf, SyncingFolder> {
        // Wait for synchronizer thread to finish
        self.abort_handler.abort();

        let mut hashmap = HashMap::new();
        for entry in self.syncing_folders.iter() {
            hashmap.insert(entry.key().clone(), entry.value().clone());
        }

        hashmap
    }

    pub async fn process_updates(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
        folder_to_watch: &PathBuf,
        syncing_folders: &DashMap<PathBuf, SyncingFolder>,
    ) -> Result<(), &'static str> {
        let files_to_update = Self::scan_folders_and_calculate_difference(folder_to_watch, syncing_folders);
        Self::upload_files(shinkai_manager_for_sync, files_to_update, syncing_folders).await
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
