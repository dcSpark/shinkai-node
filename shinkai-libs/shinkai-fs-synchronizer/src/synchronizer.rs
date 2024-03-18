use crate::shinkai_manager::ShinkaiManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{path, thread};

use std::fs::{self, DirEntry};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]

// Optional fields mean that there is possibility for `None` value upon first start. Then properties always will contain values
pub struct SyncingFolder {
    pub profile_name: Option<String>,
    pub vector_fs_path: Option<String>,
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
    syncing_folders: HashMap<LocalOSFolderPath, SyncingFolder>, // LocalOSFolderPath, SyncingFolder
    // because we're just sending content, we should only need sender here
    sender: std::sync::mpsc::Sender<String>,
    shinkai_manager: ShinkaiManager,
}

impl FilesystemSynchronizer {
    // treat new as a constructor, so how this should be treated
    pub fn new(shinkai_manager: ShinkaiManager, syncing_folders: HashMap<LocalOSFolderPath, SyncingFolder>) -> Self {
        let (sender, _) = std::sync::mpsc::channel();
        FilesystemSynchronizer {
            syncing_folders,
            sender,
            shinkai_manager,
        }
    }

    // start synchronization
    pub async fn synchronize(&self) {
        let syncing_folders = self.syncing_folders.clone();
        let mut shinkai_manager = self.shinkai_manager.clone();

        dbg!(syncing_folders.clone());

        // TODO: identify why not entering this one
        tokio::spawn(async move {
            loop {
                for (path, _folder) in syncing_folders.iter() {
                    let dir_exists = shinkai_manager.get_node_folder(&path.0).await;

                    match dir_exists {
                        Ok(result) => {
                            println!("test test {}", result);
                        }
                        Err(res) => {}
                    }
                }

                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
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
        let folders = self.syncing_folders.clone();

        // TODO: save the current state of sync somewhere
        folders.clone()
    }

    pub fn stop(self) -> HashMap<LocalOSFolderPath, SyncingFolder> {
        drop(self.sender); // This will close the thread
        self.syncing_folders.clone()
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
            let mut folders = self.syncing_folders.clone();
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
