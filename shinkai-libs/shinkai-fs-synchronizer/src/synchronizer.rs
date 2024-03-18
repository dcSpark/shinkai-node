use crate::shinkai_manager::ShinkaiManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{path, thread};

use std::fs::{self, DirEntry};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SyncingFolder {
    pub profile_name: String,
    pub vector_fs_path: String,
    pub local_os_folder_path: LocalOSFolderPath,
    pub last_synchronized_file_datetime: Option<u64>, // UTC with milliseconds
}

// for simplicity we don't use this wrapper right now
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

#[derive(Clone)]
pub struct FilesystemSynchronizer {
    syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>, // LocalOSFolderPath, SyncingFolder
    // because we're just sending content, we should only need sender here
    sender: std::sync::mpsc::Sender<String>,
    shinkai_manager: ShinkaiManager,
}

impl FilesystemSynchronizer {
    // treat new as a constructor, so how this should be treated
    pub fn new(shinkai_manager: ShinkaiManager, syncing_folders: HashMap<LocalOSFolderPath, SyncingFolder>) -> Self {
        let (sender, _) = std::sync::mpsc::channel();
        let syncing_folders = Arc::new(Mutex::new(syncing_folders));
        FilesystemSynchronizer {
            syncing_folders,
            sender,
            shinkai_manager,
        }
    }

    // start synchronization
    pub async fn synchronize(&self) {
        let syncing_folders = self.syncing_folders.clone();
        let shinkai_manager_profile_name = self.shinkai_manager.profile_name.clone();

        // the main loop is happening here
        thread::spawn(move || loop {
            let folders = syncing_folders.lock().unwrap();
            for (path, folder) in folders.iter() {
                // Clone or copy necessary data before using it in the thread
                let syncing_folder_for_os_path = SyncingFolder {
                    profile_name: shinkai_manager_profile_name.clone(),
                    vector_fs_path: "".to_string(),
                    local_os_folder_path: path.clone(),
                    last_synchronized_file_datetime: None,
                };

                println!("Checking if folder at path {} is out of date.", path.0);
            }

            std::thread::sleep(std::time::Duration::from_secs(60));
        });
    }

    // pub fn add_syncing_folder(&mut self, path: String, folder: SyncingFolder) -> Result<(), String> {
    //     let mut folders = self.syncing_folders.lock().unwrap();
    //     if let std::collections::hash_map::Entry::Vacant(e) = folders.entry(path) {
    //         e.insert(folder);
    //         Ok(())
    //     } else {
    //         Err("Folder already exists".to_string())
    //     }
    // }

    pub fn get_current_syncing_folders_map(&self) -> HashMap<LocalOSFolderPath, SyncingFolder> {
        let folders = self.syncing_folders.lock().unwrap();

        // TODO: save the current state of sync somewhere
        folders.clone()
    }

    pub fn stop(self) -> HashMap<LocalOSFolderPath, SyncingFolder> {
        drop(self.sender); // This will close the thread
        self.syncing_folders.lock().unwrap().clone()
    }
}
