use crate::shinkai_manager::ShinkaiManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SyncingFolder {
    pub profile_name: Option<String>,
    pub vector_fs_path: Option<String>,
    pub local_os_folder_path: LocalOSFolderPath,
    pub last_synchronized_file_datetime: Option<u64>, // UTC with milliseconds
}

#[derive(Clone, Debug)]
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

#[derive(Debug, Clone)]
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
    pub async fn synchronize(&self) -> anyhow::Result<()> {
        let syncing_folders = self.syncing_folders.clone();
        let mut shinkai_manager = self.shinkai_manager.clone();
        let syncing_queue = Arc::clone(&self.syncing_queue);

        for (path, folder) in syncing_folders.lock().unwrap().iter() {
            let dir_entries = std::fs::read_dir(&path.0)?;
            for entry in dir_entries {
                let entry = entry?;
                let metadata = entry.metadata()?;

                let modified_time = metadata.modified()?.elapsed()?.as_secs();
                let file_datetime = match folder.last_synchronized_file_datetime {
                    Some(last_sync_time) if modified_time > last_sync_time => Some(modified_time),
                    None => Some(metadata.created()?.elapsed()?.as_secs()),
                    _ => None,
                };

                if let Some(datetime) = file_datetime {
                    let sync_item = SyncQueueItem {
                        syncing_folder: folder.clone(),
                        os_file_path: entry.path(),
                        file_datetime: datetime,
                    };
                    syncing_queue.lock().unwrap().push(sync_item);
                }
            }
        }

        // Sort the syncing queue based on file_datetime
        syncing_queue.lock().unwrap().sort_by_key(|k| k.file_datetime);

        let queue = syncing_queue.lock().unwrap().clone();
        let major_dir_path = Path::new(&self.major_dir);
        for item in queue.iter() {
            let relative_path = item
                .os_file_path
                .strip_prefix(major_dir_path)
                .map_or_else(|_| item.os_file_path.clone(), PathBuf::from);
            let node_fs_path = relative_path
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap_or(&relative_path);
            println!("Syncing file: {:?}", node_fs_path);

            shinkai_manager.get_node_folder("/").await;
            shinkai_manager.create_folder("test", "/").await;

            // every few seconds (configurable) save state of the SyncingFolder, so we can rebuild syncing queue
        }
        // clear the queue after processing
        syncing_queue.lock().unwrap().clear();

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

    pub fn visit_dirs(&self, dir: &Path) -> std::io::Result<()> {
        let path_str = dir.to_str().unwrap_or_default().to_string();
        let local_os_folder_path = LocalOSFolderPath(path_str.clone());
        let syncing_folder = SyncingFolder {
            profile_name: Some(self.shinkai_manager.profile_name.to_string()),
            vector_fs_path: None,
            local_os_folder_path: local_os_folder_path.clone(),
            last_synchronized_file_datetime: None,
        };

        {
            let mut folders = self.syncing_folders.lock().unwrap();
            folders.insert(local_os_folder_path, syncing_folder);
        } // Release the lock immediately after use

        // Recursively visit subdirectories
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    println!("Directory: {:?}", path);
                    self.visit_dirs(&path)?;
                } else {
                    // Placeholder for file processing logic
                }
            }
        }

        Ok(())
    }
}
