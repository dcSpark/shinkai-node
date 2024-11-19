use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::Datelike;
use chrono::TimeZone;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;

use crate::http_requests::PostRequestError;
use crate::persistence::{ShinkaiMirrorDB, ShinkaiMirrorDBError};
use crate::shinkai::api_schemas::{DistributionInfo, FileInfo, FileUploadResponse};
use crate::shinkai::shinkai_manager_for_sync::ShinkaiManagerForSync;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncingFolder {
    local_last_synchronized_file_datetime: SystemTime,
    merkle_hash: Option<String>,
    is_folder: bool,
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
    pub should_mirror_delete: bool,
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
        should_mirror_delete: bool,
        upload_timeout: Option<Duration>,
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
                            should_mirror_delete,
                            upload_timeout,
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
                                should_mirror_delete,
                                upload_timeout,
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
            should_mirror_delete,
        })
    }

    pub async fn scan_folders_and_remove_old_files(
        destination_path: PathBuf,
        folder_to_watch: &PathBuf,
        profile_name: &str,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    ) {
        // Scan the local folder for existing files
        let folder_files = Self::scan_folders(folder_to_watch);

        // Lock the database to access the synchronized files' information
        let db = syncing_folders_db.lock().await;

        // Iterate over each file found in the local folder
        for (path, local_modified_time) in folder_files {
            // Calculate the relative path of the file with respect to the folder being watched
            if let Ok(relative_path) = path.strip_prefix(folder_to_watch) {
                // Remove the file extension from the relative path
                let relative_path_without_extension = relative_path.with_extension("");
                // Construct the full path as it would be represented in the destination, without the file extension
                let full_destination_path = destination_path.join(relative_path_without_extension);

                // Attempt to retrieve the file's synchronization state from the database
                match db.get_file_mirror_state(profile_name.to_string(), full_destination_path.clone()) {
                    Ok(Some(syncing_folder)) => {
                        // If the file exists in the database, compare the last modified times
                        if local_modified_time > syncing_folder.local_last_synchronized_file_datetime {
                            // If the local file is older, delete it
                            if let Err(e) =
                                db.delete_file_mirror_state(profile_name.to_string(), full_destination_path.clone())
                            {
                                eprintln!("Failed to delete outdated file {:?}: {}", full_destination_path, e);
                            } else {
                                eprintln!("Deleted outdated file {:?}", full_destination_path);
                            }
                        }
                    }
                    Ok(None) => {
                        // Do nothing
                        // eprintln!("File {:?} is not in the database.", full_destination_path);
                    }
                    Err(e) => {
                        // Log any errors encountered while accessing the database
                        eprintln!("Error accessing database for file {:?}: {}", full_destination_path, e);
                    }
                }
            }
        }
    }

    pub async fn scan_folders_and_calculate_difference(
        destination_path: PathBuf,
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
            let mut path = destination_path.join(PathBuf::from(relative_path));
            // Remove the file extension
            path = path.with_extension("");
            let syncing_folders = syncing_folders_db.lock().await;
            // Use get_file_mirror_state to check if the file exists in the database
            // eprintln!("Checking file: {:?}", path);
            match syncing_folders.get_file_mirror_state(profile_name.to_string(), path.clone()) {
                Ok(Some(syncing_folder)) => {
                    // If the file exists and the modified time is greater than the last synchronized time, add it to the update list
                    if modified_time > syncing_folder.local_last_synchronized_file_datetime {
                        eprintln!(
                            "File {:?} has been modified. Last synchronized time: {:?}, Current time: {:?}",
                            path, syncing_folder.local_last_synchronized_file_datetime, modified_time
                        );
                        files_to_update.push((full_path, modified_time));
                    }
                }
                Ok(None) => {
                    // If the file does not exist in the database, add it to the update list
                    eprintln!("File {:?} is not in the database.", path);
                    files_to_update.push((full_path, modified_time));
                }
                Err(e) => {
                    // Handle potential errors, for example, log them or push them to an error list
                    eprintln!("Error accessing database for file {:?}: {}", path, e);
                }
            }
        }
        files_to_update
    }

    /// the function scans the server for files that are not present in the local folder
    /// and returns a list of files to remove (and folders which they should be ignored)
    pub async fn scan_server_to_files_to_remove(
        destination_path: PathBuf,
        folder_to_watch: &PathBuf,
        _profile_name: &str,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
    ) -> Vec<PathBuf> {
        // Scan the local folder for existing files and store them in a HashMap for quick lookup
        let current_folder_files = Self::scan_folders(folder_to_watch)
            .into_keys()
            .map(|path| {
                let mut full_path = destination_path.join(path);
                if !full_path.to_string_lossy().starts_with('/') {
                    full_path = PathBuf::from("/").join(full_path);
                }
                // Remove the file extension
                full_path.set_extension("");
                (full_path, ())
            }) // Create a tuple with a dummy value
            .collect::<HashMap<_, _>>();

        let mut files_to_remove = Vec::new();
        let syncing_folders = syncing_folders_db.lock().await;

        // Retrieve all file mirror states from the database
        if let Ok(all_file_mirror_states) = syncing_folders.all_file_mirror_states() {
            for (db_path, file_info) in all_file_mirror_states {
                let normalized_db_path = Path::new(&db_path)
                    .strip_prefix(folder_to_watch)
                    .unwrap_or(Path::new(&db_path));

                // Check if the file from the database is not present in the current folder files
                if !current_folder_files.contains_key(normalized_db_path) && !file_info.is_folder {
                    // If the file is not present locally, add it to the removal list
                    let full_path_to_remove = destination_path.join(normalized_db_path);
                    files_to_remove.push(full_path_to_remove);
                }
            }
        } else {
            eprintln!("Failed to retrieve file mirror states from the database.");
        }

        files_to_remove
    }

    pub async fn remove_empty_folders(
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
    ) -> Result<(), PostRequestError> {
        // Attempt to retrieve the folder from the node to ensure it exists
        match shinkai_manager_for_sync.get_node_folder("/").await {
            Ok(result) => {
                // Extract paths and check for empty folders
                let paths_and_file_info = Self::extract_paths_and_hashes(&result);

                for (path, file_info) in paths_and_file_info {
                    if file_info.is_folder && file_info.child_item_count == 0 {
                        // If the folder is empty, send a request to delete it
                        let delete_result = shinkai_manager_for_sync.delete_folder(&path).await;
                        if let Err(e) = delete_result {
                            eprintln!("Failed to delete empty folder {}: {:?}", path, e);
                        } else {
                            eprintln!("Deleted empty folder {}", path);
                        }
                    }
                }

                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to get node folder: {:?}", e);
                Err(e)
            }
        }
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
            } else {
                // Use creation_datetime_extraction to get the modified date
                if let Ok(Some(datetime_str)) = FilesystemSynchronizer::creation_datetime_extraction(&path) {
                    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(&datetime_str) {
                        let system_time: SystemTime = datetime.into();
                        folder_files.insert(path, system_time);
                    }
                }
            }
        }

        scan_path(folder_to_watch.clone(), &mut folder_files);

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
                    Ok(_) => (), // eprintln!("Folder exists: {}", folder_check_path)
                    Err(_) => {
                        // Correctly construct the create_folder_path without the erroneous "./"
                        let create_folder_path = if current_path.contains('/') {
                            format!("/{}", &current_path[..current_path.rfind('/').unwrap_or(0)])
                        } else {
                            "/".to_string()
                        };
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
        upload_timeout: Option<Duration>,
    ) -> Result<(), PostRequestError> {
        let total_files = files.len();
        let mut uploaded_files_count = 0;

        for (file_path, modified_time) in files {
            // Skip uploading if the file is named .DS_Store
            if file_path.file_name().map_or(false, |name| name == ".DS_Store") {
                continue;
            }

            let file_data = std::fs::read(&file_path)
                .map_err(|_| PostRequestError::FSFolderNotFound("Failed to read file data".into()))?;
            let filename = file_path
                .file_name()
                .ok_or(PostRequestError::Unknown("Failed to extract filename".into()))?
                .to_str()
                .ok_or(PostRequestError::Unknown("Failed to convert filename to string".into()))?;

            let relative_path = file_path.strip_prefix(folder_to_watch).unwrap_or(&file_path);
            // Ensure destination_buf is the directory path only, not including the file name
            let destination_dir = destination_path.join(relative_path.parent().unwrap_or_else(|| Path::new("")));
            let destination_str = destination_dir.to_string_lossy();

            let creation_datetime_str = Self::creation_datetime_extraction(&file_path).map_err(|e| {
                eprintln!("Failed to extract creation datetime: {:?}", e);
                PostRequestError::Unknown("Failed to extract creation datetime".into())
            })?;

            let retry_delays = [0, 60, 300]; // Retry delays in seconds: immediately, after 1 minute, and after 5 minutes
            let mut attempt = 0;

            loop {
                let upload_result: Result<Vec<FileUploadResponse>, PostRequestError> = shinkai_manager_for_sync
                    .upload_file(
                        &file_data,
                        filename,
                        &destination_str,
                        creation_datetime_str.clone(),
                        upload_timeout,
                    )
                    .await;

                match upload_result {
                    Ok(response) if !response.is_empty() => {
                        let response = &response[0].clone(); // Use the first result
                        uploaded_files_count += 1;
                        eprintln!("Uploaded {}/{} files.", uploaded_files_count, total_files);

                        // Construct the full path for the database, ensuring there's a "/" between destination_path and file_path_for_db if necessary
                        let file_path_for_db = destination_dir.join(filename).with_extension("");

                        let db = syncing_folders_db.lock().await;
                        let syncing_folder = SyncingFolder {
                            local_last_synchronized_file_datetime: modified_time,
                            merkle_hash: Some(response.merkle_hash.clone()),
                            is_folder: false,
                        };
                        db.add_or_update_file_mirror_state(profile_name.to_string(), file_path_for_db, syncing_folder)
                            .map_err(|_| PostRequestError::Unknown("Failed to update file mirror state".into()))?;
                        break;
                    }
                    Ok(_) => eprintln!("No files were uploaded."),
                    Err(e) => match e {
                        PostRequestError::RequestFailed(ref msg)
                            if msg.contains("timed out") && attempt < retry_delays.len() =>
                        {
                            eprintln!("Timeout error occurred, retrying... Attempt: {}", attempt + 1);
                            tokio::time::sleep(tokio::time::Duration::from_secs(retry_delays[attempt])).await;
                            attempt += 1;
                        }
                        _ => {
                            eprintln!("Failed to upload file: {:?}", e);
                            break; // Move on to the next file after the specified number of retries
                        }
                    },
                }
            }
        }

        Ok(())
    }

    /// Deletes the local entries (not files) which are not found in the Node
    /// This could happen if files were deleted in the Node, the local entries need to be synced with the Node
    /// It also updates the local entries that are found in the Node but dont exist local registry
    /// This could happen if the local registry was deleted or the file was added to the Node
    pub async fn sync_local_with_node_folder(
        destination_path: PathBuf,
        profile_name: &str,
        syncing_folders_db: Arc<Mutex<ShinkaiMirrorDB>>,
        shinkai_manager_for_sync: &ShinkaiManagerForSync,
    ) -> Result<(), ShinkaiMirrorDBError> {
        let destination_path_str = destination_path.to_string_lossy();
        let folder_check_path = if destination_path_str == "/" {
            String::from(destination_path_str.clone())
        } else {
            format!("/{}", destination_path_str.trim_start_matches("./"))
        };

        // Attempt to retrieve the folder from the node to ensure it exists
        match shinkai_manager_for_sync.get_node_folder(&folder_check_path).await {
            Ok(result) => {
                // eprintln!(">> Node folder exists: {:?}", result);
                let paths_and_file_info = Self::extract_paths_and_hashes(&result);

                // If the folder exists on the node, proceed with synchronization
                let db = syncing_folders_db.lock().await;

                // Get all file mirror states before synchronization
                let all_file_mirror_states = db.all_file_mirror_states().unwrap();
                let filtered_states: Vec<_> = all_file_mirror_states
                    .into_iter()
                    .filter(|(path, _)| path.starts_with(&*destination_path_str))
                    .collect();

                // Iterate through each file found on the node
                for (node_path, file_info) in paths_and_file_info.iter() {
                    let local_path_without_extension = Path::new(node_path).with_extension("");
                    // Check if the file exists locally and in the registry
                    match db.get_file_mirror_state(profile_name.to_string(), local_path_without_extension.clone()) {
                        Ok(Some(syncing_folder)) => {
                            // Determine the correct file_modified_time
                            let file_modified_time = file_info.distribution_info.as_ref().map_or_else(
                                || {
                                    Utc.datetime_from_str(&file_info.last_written_datetime, "%+")
                                        .unwrap_or_else(|_| Utc::now())
                                },
                                |di| Utc.datetime_from_str(&di.datetime, "%+").unwrap_or_else(|_| Utc::now()),
                            );

                            // Convert DateTime<Utc> to SystemTime to compare with syncing_folder.local_last_synchronized_file_datetime
                            let file_modified_time_system = file_modified_time.into();

                            if file_modified_time_system != syncing_folder.local_last_synchronized_file_datetime {
                                let syncing_folder = SyncingFolder {
                                    local_last_synchronized_file_datetime: file_modified_time_system,
                                    merkle_hash: Some(file_info.merkle_hash.clone()),
                                    is_folder: file_info.is_folder,
                                };
                                db.add_or_update_file_mirror_state(
                                    profile_name.to_string(),
                                    local_path_without_extension,
                                    syncing_folder,
                                )?;
                            }
                        }
                        Ok(None) => {
                            // If the file exists locally but not in the registry, add it
                            let file_modified_time = file_info.distribution_info.as_ref().map_or_else(
                                || {
                                    Utc.datetime_from_str(&file_info.created_datetime, "%+")
                                        .unwrap_or_else(|_| Utc::now())
                                },
                                |di| Utc.datetime_from_str(&di.datetime, "%+").unwrap_or_else(|_| Utc::now()),
                            );

                            // Convert DateTime<Utc> to SystemTime for consistency with the rest of the system
                            let file_modified_time_system = file_modified_time.into();

                            let syncing_folder = SyncingFolder {
                                local_last_synchronized_file_datetime: file_modified_time_system,
                                merkle_hash: Some(file_info.merkle_hash.clone()),
                                is_folder: file_info.is_folder,
                            };
                            db.add_or_update_file_mirror_state(
                                profile_name.to_string(),
                                local_path_without_extension,
                                syncing_folder,
                            )?;
                        }
                        Err(e) => {
                            eprintln!(
                                "Error accessing database for file {:?}: {}",
                                local_path_without_extension, e
                            );
                        }
                    }
                }

                // Delete local entries that do not exist on the server
                for (path, _) in filtered_states.iter() {
                    // Convert the PathBuf to a String for comparison
                    let db_path_str = path.to_string_lossy();

                    // Strip the leading "./" from the path string
                    let comparable_path_str = db_path_str.strip_prefix("./").unwrap_or(&db_path_str);

                    // Convert the string slice back to a Path, remove the extension, and convert back to a string slice
                    let path_without_extension_buf = Path::new(comparable_path_str).with_extension("");
                    let path_without_extension = path_without_extension_buf.to_str().unwrap_or(comparable_path_str);

                    if !paths_and_file_info.contains_key(path_without_extension) {
                        eprintln!("Deleting local entry not found on server: {:?}", comparable_path_str);
                        if let Err(e) = db.delete_file_mirror_state(profile_name.to_string(), PathBuf::from(path)) {
                            eprintln!("Failed to delete local entry not found on server {:?}: {}", path, e);
                        }
                    }
                }

                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to get node folder: {:?}", e);
                if let PostRequestError::FSFolderNotFound(_) = e {
                    let db = syncing_folders_db.lock().await;
                    let _ = db.delete_keys_with_profile_and_prefix(profile_name, &destination_path);
                    eprintln!(
                        "Deleted all keys for profile: '{}' with prefix: '{}' due to FS folder not found.",
                        profile_name, destination_path_str
                    );
                }
                // Handle the case where the folder does not exist on the node or another error occurs
                Err(ShinkaiMirrorDBError::from(e))
            }
        }
    }

    #[allow(dead_code)]
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
        should_mirror_delete: bool,
        upload_timeout: Option<Duration>,
    ) -> Result<(), PostRequestError> {
        // Check the health of the external service before proceeding
        match shinkai_manager_for_sync.check_node_health().await {
            Ok(_health_status) => {
                eprintln!("Scanning folders for changes and removing outdated files from registry...");
                let _ = Self::scan_folders_and_remove_old_files(
                    destination_path.clone(),
                    folder_to_watch,
                    profile_name,
                    syncing_folders_db.clone(),
                )
                .await;

                // Sync current persistence with the one from the server
                eprintln!("Syncing local with node folder (deleting local registry for files that are not in the node anymore)...");
                let _ = Self::sync_local_with_node_folder(
                    destination_path.clone(),
                    profile_name,
                    syncing_folders_db.clone(),
                    shinkai_manager_for_sync,
                )
                .await;

                // Proceed with the updates if the health check is successful
                eprintln!("Scanning folders for changes and calculating differences...");
                let files_to_update = Self::scan_folders_and_calculate_difference(
                    destination_path.clone(),
                    folder_to_watch,
                    profile_name,
                    syncing_folders_db.clone(),
                )
                .await;
                eprintln!("Files to update: {:?}", files_to_update.len());

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
                    syncing_folders_db.clone(),
                    folder_to_watch,
                    upload_timeout,
                )
                .await?;

                // if delete files is enabled, delete files that are not in the local registry
                if should_mirror_delete {
                    eprintln!("Searching for files to delete (not in the local registry anymore)...");
                    let files_to_delete = Self::scan_server_to_files_to_remove(
                        destination_path.clone(),
                        folder_to_watch,
                        profile_name,
                        syncing_folders_db.clone(),
                    )
                    .await;
                    // eprintln!("Files to delete: {:?}", files_to_delete);

                    for file_path in files_to_delete {
                        // Convert PathBuf to a string, remove the leading "./", and remove the file extension
                        let mut file_path_str = file_path.to_string_lossy().into_owned();
                        if file_path_str.starts_with("./") {
                            file_path_str = file_path_str[2..].to_string();
                        }
                        // Remove the file extension
                        let file_path_without_extension = Path::new(&file_path_str).with_extension("");
                        let mut file_path_final = file_path_without_extension.to_string_lossy().to_string();

                        // Ensure the path starts with a "/"
                        if !file_path_final.starts_with('/') {
                            file_path_final.insert(0, '/');
                        }

                        match shinkai_manager_for_sync.delete_item(&file_path_final).await {
                            Ok(_) => eprintln!("Successfully deleted: {}", file_path_final),
                            Err(e) => eprintln!("Failed to delete {}: {:?}", file_path_final, e),
                        }
                    }

                    let _ = Self::remove_empty_folders(shinkai_manager_for_sync).await;
                }

                Ok(())
            }
            Err(health_check_error) => {
                // Handle the case where the health check fails
                Err(PostRequestError::Unknown(format!(
                    "Node health check failed: {}",
                    health_check_error
                )))
            }
        }
    }

    pub async fn get_scan_folders_and_calculate_difference(&self) -> Vec<(PathBuf, SystemTime)> {
        let differences = FilesystemSynchronizer::scan_folders_and_calculate_difference(
            self.destination_path.clone(),
            &self.folder_to_watch,
            &self.profile_name,
            self.syncing_folders_db.clone(),
        )
        .await;

        differences
            .into_iter()
            .map(|(full_path, modified_time)| {
                let relative_path = full_path.strip_prefix(&self.folder_to_watch).unwrap_or(&full_path);
                (relative_path.to_path_buf(), modified_time)
            })
            .collect()
    }

    pub async fn force_process_updates(&self) -> Result<(), PostRequestError> {
        FilesystemSynchronizer::process_updates(
            &self.shinkai_manager_for_sync,
            &self.folder_to_watch,
            &self.profile_name,
            &self.destination_path,
            self.syncing_folders_db.clone(),
            self.should_mirror_delete,
            None,
        )
        .await
    }

    /// Cleans the database entries for the current profile and a given key prefix.
    pub async fn clean_for_prefix(&self, key_prefix: &Path) -> Result<(), ShinkaiMirrorDBError> {
        let db = self.syncing_folders_db.lock().await;
        db.delete_keys_with_profile_and_prefix(&self.profile_name, key_prefix)
    }

    pub fn extract_paths_and_hashes(result: &Value) -> HashMap<String, FileInfo> {
        let mut paths_and_file_info = HashMap::new();
        if let Value::String(result_str) = result {
            if let Ok(parsed_result) = serde_json::from_str::<Value>(result_str) {
                Self::extract_paths_and_hashes_recursive(&parsed_result, &mut paths_and_file_info);
            } else {
                eprintln!("Failed to parse result string as JSON.");
            }
        } else {
            Self::extract_paths_and_hashes_recursive(result, &mut paths_and_file_info);
        }

        paths_and_file_info
    }

    fn extract_paths_and_hashes_recursive(value: &Value, paths_and_file_info: &mut HashMap<String, FileInfo>) {
        match value {
            Value::Object(obj) => {
                if let Some(Value::String(path)) = obj.get("path") {
                    if let Some(Value::String(merkle_hash)) = obj.get("merkle_hash") {
                        let name = obj.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                        let source_file_map_last_saved_datetime = obj
                            .get("source_file_map_last_saved_datetime")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let created_datetime = obj
                            .get("created_datetime")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let last_written_datetime = obj
                            .get("last_written_datetime")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let distribution_info = obj
                            .get("distribution_info")
                            .and_then(|di| serde_json::from_value::<DistributionInfo>(di.clone()).ok());

                        let is_folder = obj.contains_key("child_folders") || obj.contains_key("child_items");
                        let child_item_count = obj
                            .get("child_items")
                            .and_then(|ci| ci.as_array())
                            .map_or(0, |ci| ci.len());
                        let child_folder_count = obj
                            .get("child_folders")
                            .and_then(|cf| cf.as_array())
                            .map_or(0, |cf| cf.len());

                        let total_child_count = child_item_count + child_folder_count;

                        let file_info = FileInfo {
                            merkle_hash: merkle_hash.clone(),
                            name,
                            source_file_map_last_saved_datetime,
                            distribution_info,
                            created_datetime,
                            last_written_datetime,
                            is_folder,
                            child_item_count: total_child_count,
                        };

                        paths_and_file_info.insert(path.clone(), file_info);
                    }
                }

                if let Some(Value::Array(child_folders)) = obj.get("child_folders") {
                    for child_folder in child_folders {
                        Self::extract_paths_and_hashes_recursive(child_folder, paths_and_file_info);
                    }
                }

                if let Some(Value::Array(child_items)) = obj.get("child_items") {
                    for child_item in child_items {
                        Self::extract_paths_and_hashes_recursive(child_item, paths_and_file_info);
                    }
                }
            }
            _ => eprintln!("Expected an object but found something else."),
        }
    }

    /// ### Documentation: File Creation Datetime Extraction
    /// #### Overview
    /// The `creation_datetime_extraction` function is designed to determine the creation datetime of a file by examining both
    /// the file's metadata and its name (or its parent folder's name). This is particularly useful in scenarios where file metadata
    /// might not be reliable or when files are named according to their creation dates. The function aims to use the oldest
    /// available datetime between the file's name, parent folder's name, and metadata.
    ///
    /// #### How It Works
    /// 1. **Extract Datetime from Filename or Parent Folder**: The function first attempts to extract a datetime from the
    /// file's name or its parent folder's name using the `extract_datetime_from_path` helper function. This extraction relies
    /// on a predefined format (currently `YYYYMMDD`).
    /// 2. **File Metadata Datetime**: Independently, the function retrieves the file's creation datetime from its metadata.
    /// 3. **Choosing the Oldest Datetime**: If both datetimes (from the filename/parent and metadata) are available, the function
    /// compares them and selects the oldest. If only one source of datetime is available, that datetime is used. If neither source
    /// provides a valid datetime, the function returns an error indicating the failure to extract or calculate the file creation datetime.
    ///
    pub fn creation_datetime_extraction(file_path: &PathBuf) -> Result<Option<String>, PostRequestError> {
        // Attempt to extract datetime from the filename or its parent folder
        let datetime_from_name_or_parent = Self::extract_datetime_from_path(file_path)
            .or_else(|| file_path.parent().and_then(Self::extract_datetime_from_path));

        let file_metadata_datetime = file_path.metadata().and_then(|metadata| metadata.modified()).ok();

        match (datetime_from_name_or_parent, file_metadata_datetime) {
            (Some(datetime_str), Some(metadata_datetime)) => {
                // If both datetime are available, choose the oldest (most likely the path)
                let datetime: chrono::DateTime<chrono::Utc> = metadata_datetime.into();
                let metadata_datetime_str = datetime.to_rfc3339();
                Ok(Some(Self::choose_oldest_datetime(
                    &datetime_str,
                    &metadata_datetime_str,
                )))
            }
            (Some(datetime_str), None) => Ok(Some(datetime_str)),
            (None, Some(metadata_datetime)) => {
                let datetime: chrono::DateTime<chrono::Utc> = metadata_datetime.into();
                Ok(Some(datetime.to_rfc3339()))
            }
            (None, None) => Err(PostRequestError::Unknown(
                "Failed to extract or calculate file creation datetime".into(),
            )),
        }
    }

    pub fn extract_datetime_from_path(path: &Path) -> Option<String> {
        Self::extract_datetime_from_path_with_date_provider(path, chrono::Utc::now)
    }

    pub fn extract_datetime_from_path_with_date_provider<F>(path: &Path, current_date_provider: F) -> Option<String>
    where
        F: Fn() -> chrono::DateTime<chrono::Utc>,
    {
        let path_str = path.to_string_lossy();
        // Regular expression to find a date in the format YYYYMMDD
        let re = regex::Regex::new(r"(\d{4})(?:[-_])?(\d{2})(?:[-_])?(\d{2})").unwrap();

        if let Some(caps) = re.captures(&path_str) {
            if caps.len() == 4 {
                // Construct a date string in ISO8601 format (YYYY-MM-DD)
                let date_str = format!("{}-{}-{}", &caps[1], &caps[2], &caps[3]);
                // Try to parse the date string to ensure it's valid
                if let Ok(date) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                    // Convert the date to a DateTime<Utc> at the start of the day (00:00:00)
                    let datetime = chrono::DateTime::<chrono::Utc>::from_utc(date.and_hms(0, 0, 0), chrono::Utc);
                    // Return the datetime as an RFC3339 formatted string
                    return Some(datetime.to_rfc3339());
                }
            }
        }

        // If no full date, try to match YYYY_MM without a day
        let re_month_year = regex::Regex::new(r".*(\d{4})_(\d{2}).*").unwrap();
        if let Some(caps) = re_month_year.captures(&path_str) {
            if caps.len() == 3 {
                let year = caps[1].parse::<i32>().unwrap();
                let month = caps[2].parse::<u32>().unwrap();

                let current_date = current_date_provider();
                let current_year = current_date.year();
                let current_month = current_date.month();

                if year == current_year && month == current_month {
                    // Same month and year, return None
                    return None;
                } else if year == current_year && month < current_month || year < current_year {
                    // Calculate the first moment of the next month
                    let next_month = if month == 12 { 1 } else { month + 1 };
                    let next_month_year = if month == 12 { year + 1 } else { year };
                    let first_moment = chrono::NaiveDate::from_ymd(next_month_year, next_month, 1).and_hms(0, 0, 0);
                    let first_moment_datetime = chrono::DateTime::<chrono::Utc>::from_utc(first_moment, chrono::Utc);
                    return Some(first_moment_datetime.to_rfc3339());
                }
            }
        }

        None
    }

    pub fn choose_oldest_datetime(datetime1: &str, datetime2: &str) -> String {
        let dt1 = chrono::DateTime::parse_from_rfc3339(datetime1)
            .unwrap_or_else(|_| chrono::DateTime::from(chrono::Utc::now()));
        let dt2 = chrono::DateTime::parse_from_rfc3339(datetime2)
            .unwrap_or_else(|_| chrono::DateTime::from(chrono::Utc::now()));

        if dt1 < dt2 {
            datetime1.to_string()
        } else {
            datetime2.to_string()
        }
    }
}
