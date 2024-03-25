// use alloc::sync;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use shinkai_vector_resources::vector_resource::SimplifiedFSRoot;

use crate::shinkai_manager::ShinkaiManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncingFolder {
    pub profile_name: ProfileName,
    pub vector_fs_path: Option<String>,
    pub local_os_folder_path: LocalOSFolderPath,
    pub last_synchronized_file_datetime: Option<u64>, // UTC with milliseconds
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalOSFolderPath(pub String);

impl PartialEq for LocalOSFolderPath {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for LocalOSFolderPath {}

impl Hash for LocalOSFolderPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQueueItem {
    pub syncing_folder: SyncingFolder,
    pub os_file_path: PathBuf,
    pub file_datetime: u64, // Assuming this is the datetime format used
}

#[derive(Clone)]
pub struct FilesystemSynchronizer {
    syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>, // LocalOSFolderPath, SyncingFolder
    // because we're just sending content, we should only need sender here
    sender: std::sync::mpsc::Sender<String>,
    shinkai_manager: ShinkaiManager,
    syncing_queue: Arc<Mutex<Vec<SyncQueueItem>>>,
    major_dir: String,
}

impl FilesystemSynchronizer {
    // treat new as a constructor, so how this should be treated
    pub fn new(
        shinkai_manager: ShinkaiManager,
        syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>,
        major_dir: String,
    ) -> Self {
        let (sender, _) = std::sync::mpsc::channel();
        FilesystemSynchronizer {
            syncing_folders,
            sender,
            shinkai_manager,
            syncing_queue: Arc::new(Mutex::new(Vec::new())),
            major_dir,
        }
    }

    // start synchronization
    pub async fn synchronize(&self, fs_root: SimplifiedFSRoot) -> anyhow::Result<()> {
        // inside syncing_folders we already store all directories and files from the disk that are newer (or were modified later) than last synced folder
        // here we have our list of tuples
        let syncing_folders = self.syncing_folders.clone();
        // let syncing_queue = Arc::clone(&self.syncing_queue);

        dbg!(syncing_folders.clone());

        // go through each syncing folder and add it to the syncing queue (oldest first in regards to OSFilePath)
        let syncing_folders_lock = syncing_folders.lock().unwrap();
        let mut syncing_queue_lock = self.syncing_queue.lock().unwrap();
        syncing_queue_lock.clear(); // Clear the existing queue before repopulating
        for (local_os_folder_path, syncing_folder) in syncing_folders_lock.iter() {
            let os_file_path = PathBuf::from(&local_os_folder_path.0);
            let file_datetime = syncing_folder.last_synchronized_file_datetime.unwrap_or(0);
            let sync_queue_item = SyncQueueItem {
                syncing_folder: syncing_folder.clone(),
                os_file_path,
                file_datetime,
            };
            syncing_queue_lock.push(sync_queue_item);
        }
        // Sort the queue by file_datetime, oldest first
        syncing_queue_lock.sort_by_key(|item| item.file_datetime);

        dbg!(self.syncing_queue.clone());

        // TODO: proceed with processsing syncing queue

        Ok(())
    }

    pub fn add_syncing_folder(&mut self, path: String, folder: SyncingFolder) -> Result<(), String> {
        let path_buf = PathBuf::from(&folder.local_os_folder_path.0);
        if !path_buf.exists() || !path_buf.is_dir() {
            return Err("Specified path does not exist or is not a directory".to_string());
        }

        let mut folders = self.syncing_folders.lock().unwrap();
        let local_os_folder_path = LocalOSFolderPath(path);
        if let std::collections::hash_map::Entry::Vacant(e) = folders.entry(local_os_folder_path) {
            e.insert(folder);
            Ok(())
        } else {
            Err("Folder already exists".to_string())
        }
    }

    pub fn get_current_syncing_folders_map(&self) -> HashMap<LocalOSFolderPath, SyncingFolder> {
        // TODO: save the current state of sync somewhere
        let folders_lock = self.syncing_folders.lock().unwrap();
        folders_lock.clone()
    }

    pub fn stop(self) -> HashMap<LocalOSFolderPath, SyncingFolder> {
        drop(self.sender); // This will close the thread
        self.syncing_folders.lock().unwrap().clone()
    }
}
