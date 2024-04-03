use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

pub enum SyncInterval {
    Immediate,
    Timed(Duration),
    None,
}

pub struct FilesystemSynchronizer {
    pub abort_handler: Option<AbortHandle>,
    pub shinkai_manager_for_sync: ShinkaiManagerForSync,
    pub folder_to_watch: PathBuf,
    pub destination_path: PathBuf,
    pub profile_name: String,
    pub syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
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
        sync_interval: SyncInterval,
    ) -> Result<Self, ShinkaiMirrorDBError> {
        let db = ShinkaiMirrorDB::new(&db_path)?;
        let syncing_folders_db = Arc::new(Mutex::new(db));
        let profile_name = shinkai_manager_for_sync.sender_subidentity.clone();

        let task_handle = match sync_interval {
            SyncInterval::Immediate | SyncInterval::Timed(_) => {
                let shinkai_manager_clone = shinkai_manager_for_sync.clone();
                let folder_to_watch_clone = folder_to_watch.clone();
                let syncing_folders_db_clone = syncing_folders_db.clone();
                let destination_clone = destination_path.clone();
                let profile_name_clone = profile_name.clone();

                Some(tokio::spawn(async move {
                    if let SyncInterval::Immediate = sync_interval {
                        // Immediate sync logic
                        let result = FilesystemSynchronizer::process_updates(
                            &shinkai_manager_clone,
                            &folder_to_watch_clone,
                            &profile_name_clone,
                            &destination_clone,
                            syncing_folders_db_clone.clone(),
                        )
                        .await;
                        eprintln!("Immediate sync finished. Result: {:?}", result);
                    } else if let SyncInterval::Timed(duration) = sync_interval {
                        // Timed sync logic
                        loop {
                            eprintln!("Syncing folders");
                            let result = FilesystemSynchronizer::process_updates(
                                &shinkai_manager_clone,
                                &folder_to_watch_clone,
                                &profile_name_clone,
                                &destination_clone,
                                syncing_folders_db_clone.clone(),
                            )
                            .await;
                            eprintln!("Syncing folders finished. Result: {:?}", result);
                            tokio::time::sleep(duration).await;
                        }
                    }
                }))
            }
            SyncInterval::None => None,
        };

        let abort_handle = task_handle.map(|handle| handle.abort_handle());

        Ok(FilesystemSynchronizer {
            profile_name,
            shinkai_manager_for_sync,
            folder_to_watch,
            destination_path,
            syncing_folders_db,
            abort_handler: abort_handle,
        })
    }

    pub async fn scan_folders_and_calculate_difference(
        folder_to_watch: &PathBuf,
        profile_name: &str,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    ) -> Vec<(PathBuf, SystemTime)> {
        let current_folder_files = Self::scan_folders(folder_to_watch);
        let mut files_to_update = Vec::new();

        for (full_path, modified_time) in current_folder_files {
            let relative_path = match full_path.strip_prefix(folder_to_watch) {
                Ok(path) => path,
                Err(_) => continue, // If the path cannot be stripped, skip this iteration
            };

            // Convert the relative_path back to PathBuf to work with the rest of the code
            let path = PathBuf::from(relative_path);
            let syncing_folders = syncing_folders_db.lock().await;
            // Use get_file_mirror_state to check if the file exists in the database
            match syncing_folders.get_file_mirror_state(profile_name.to_string(), path.clone()) {
                Ok(Some(syncing_folder)) => {
                    // If the file exists and the modified time is greater than the last synchronized time, add it to the update list
                    if modified_time != syncing_folder.local_last_synchronized_file_datetime {
                        eprintln!(
                            "File modified: {:?} value: {:?} prev: {:?}",
                            path, modified_time, syncing_folder.local_last_synchronized_file_datetime
                        );
                        // Read and print out the content of the file
                        match std::fs::read_to_string(&full_path) {
                            Ok(content) => {
                                eprintln!("Content of the file: {}", content);
                            }
                            Err(e) => {
                                eprintln!("Failed to read file: {:?}, error: {}", full_path, e);
                            }
                        }
                        files_to_update.push((path, modified_time));
                    }
                }
                Ok(None) => {
                    eprintln!("File not found in database: {:?}", path);
                    // If the file does not exist in the database, add it to the update list
                    files_to_update.push((path, modified_time));
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
        fn scan_path(path: PathBuf, folder_files: &mut HashMap<PathBuf, SystemTime>) {
            if path.is_dir() {
                if let Ok(paths) = std::fs::read_dir(&path) {
                    for path in paths.filter_map(Result::ok) {
                        let path = path.path();
                        scan_path(path, folder_files); // Recursively scan the path
                    }
                }
            } else if let Ok(metadata) = path.metadata() {
                if let Ok(modified) = metadata.modified() {
                    folder_files.insert(path, modified);
                }
            }
        }

        scan_path(folder_to_watch.clone(), &mut folder_files);
        eprintln!("scan_folders> {:?}", folder_files);

        folder_files
    }

    pub async fn create_folders(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
        files: &Vec<PathBuf>,
        folder_to_watch: &PathBuf,
        destination_path: &PathBuf,
    ) -> Result<(), PostRequestError> {
        let mut folders_to_create = std::collections::HashSet::new();

        for file_path in files {
            let relative_path = file_path.strip_prefix(folder_to_watch).unwrap_or(file_path);
            let destination_buf = destination_path.join(relative_path);
            if let Some(destination_dir) = destination_buf.parent() {
                // Convert PathBuf to a string slice, ensuring it's a valid UTF-8 path
                let mut destination_str = destination_dir.to_string_lossy().into_owned();
                // Check if the destination_str starts with "./" and remove it
                if destination_str.starts_with("./") {
                    destination_str = destination_str[2..].to_string();
                }
                folders_to_create.insert(destination_str);
            }
        }

        for folder_path in folders_to_create {
            if folder_path == "." {
                continue;
            }

            let path_components: Vec<&str> = folder_path.split('/').filter(|c| !c.is_empty()).collect();
            let mut current_path = String::new();

            for (index, component) in path_components.iter().enumerate() {
                if index > 0 {
                    current_path.push('/');
                }
                current_path.push_str(component);

                let folder_check_path = format!("/{}", current_path);

                match shinkai_manager_for_sync.get_node_folder(&folder_check_path).await {
                    Ok(_) => eprintln!("Folder exists: {}", folder_check_path),
                    Err(_) => {
                        // Correctly construct the create_folder_path without the erroneous "./"
                        let create_folder_path = if current_path.contains('/') {
                            format!("/{}", &current_path[..current_path.rfind('/').unwrap_or(0)])
                        } else {
                            "/".to_string()
                        };
                        eprintln!("Creating folder: {} in {}", component, create_folder_path);
                        shinkai_manager_for_sync
                            .create_folder(component, &create_folder_path)
                            .await
                            .map_err(|e| {
                                eprintln!("Failed to create folder: {:?}, error: {}", folder_check_path, e);
                                PostRequestError::Unknown(format!("Failed to create folder: {}", folder_check_path))
                            })?;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn upload_files(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
        files: Vec<(PathBuf, SystemTime)>,
        profile_name: &str,
        destination_path: &PathBuf,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
        folder_to_watch: &PathBuf,
    ) -> Result<(), PostRequestError> {
        eprintln!("upload_files> {:?}", files);
        for (file_path, modified_time) in files {
            eprintln!("upload_files> file_path: {:?}", file_path);
            let file_data = std::fs::read(&file_path)
                .map_err(|_| PostRequestError::FSFolderNotFound("Failed to read file data".into()))?;
            eprintln!("upload_files> file_data: {:?}", file_data.len());
            let filename = file_path
                .file_name()
                .ok_or(PostRequestError::Unknown("Failed to extract filename".into()))?
                .to_str()
                .ok_or(PostRequestError::Unknown("Failed to convert filename to string".into()))?;

            let relative_path = file_path.strip_prefix(folder_to_watch).unwrap_or(&file_path);
            // Ensure destination_buf is the directory path only, not including the file name
            let destination_dir = destination_path.join(relative_path.parent().unwrap_or_else(|| Path::new("")));
            let destination_str = destination_dir.to_string_lossy();

            let upload_result = shinkai_manager_for_sync
                .upload_file(&file_data, filename, &destination_str)
                .await;

            if let Ok(_) = upload_result {
                let file_path_for_db = relative_path.to_path_buf();
                let mut db = syncing_folders_db.lock().await;
                let syncing_folder = SyncingFolder {
                    local_last_synchronized_file_datetime: modified_time,
                };
                eprintln!("file path for db: {:?}", file_path_for_db);
                db.add_or_update_file_mirror_state(profile_name.to_string(), file_path_for_db, syncing_folder)
                    .map_err(|_| PostRequestError::Unknown("Failed to update file mirror state".into()))?;
            } else if let Err(e) = upload_result {
                eprintln!("Failed to upload file: {:?}", e);
                return Err(e);
            }
        }

        eprintln!("upload_files> Done");
        Ok(())
    }

    fn stop(self) {
        if let Some(handle) = self.abort_handler {
            handle.abort();
        }
    }

    pub async fn process_updates(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
        folder_to_watch: &PathBuf,
        profile_name: &str,
        destination_path: &PathBuf,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    ) -> Result<(), PostRequestError> {
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
                eprintln!("\n\nprocess_updates> files to update: {:?}\n\n", files_to_update);

                let paths_to_create: Vec<PathBuf> = files_to_update.iter().map(|(path, _)| path.clone()).collect();

                // First, create necessary folders based on the files' relative paths
                Self::create_folders(
                    shinkai_manager_for_sync,
                    &paths_to_create,
                    folder_to_watch,
                    destination_path,
                )
                .await?;

                // Then, upload the files
                Self::upload_files(
                    shinkai_manager_for_sync,
                    files_to_update,
                    profile_name,
                    destination_path,
                    syncing_folders_db,
                    folder_to_watch,
                )
                .await
            }
            Err(health_check_error) => {
                // Handle the case where the health check fails
                eprintln!("Node health check failed: {}", health_check_error);
                Err(PostRequestError::Unknown(format!(
                    "Node health check failed: {}",
                    health_check_error
                )))
            }
        }
    }

    pub async fn get_scan_folders_and_calculate_difference(&self) -> Vec<(PathBuf, SystemTime)> {
        FilesystemSynchronizer::scan_folders_and_calculate_difference(
            &self.folder_to_watch,
            &self.profile_name,
            self.syncing_folders_db.clone(),
        )
        .await
    }

    pub async fn force_process_updates(&self) -> Result<(), PostRequestError> {
        FilesystemSynchronizer::process_updates(
            &self.shinkai_manager_for_sync,
            &self.folder_to_watch,
            &self.profile_name,
            &self.destination_path,
            self.syncing_folders_db.clone(),
        )
        .await
    }
}