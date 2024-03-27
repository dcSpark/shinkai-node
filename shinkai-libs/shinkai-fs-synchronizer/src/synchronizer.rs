// use alloc::sync;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use shinkai_vector_resources::vector_resource::SimplifiedFSRoot;

use crate::communication::PostRequestError;
use crate::shinkai_manager::ShinkaiManager;
use std::collections::HashMap;
use std::fmt::Debug;
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
    pub syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>, // LocalOSFolderPath, SyncingFolder
    shinkai_manager: ShinkaiManager,

    // it's more convenient to have a vector instead of tuple map here
    syncing_queue: Arc<Mutex<Vec<SyncQueueItem>>>,
}

impl FilesystemSynchronizer {
    // treat new as a constructor, so how this should be treated
    pub fn new(shinkai_manager: ShinkaiManager) -> Self {
        FilesystemSynchronizer {
            syncing_folders: Arc::new(Mutex::new(HashMap::new())),
            syncing_queue: Arc::new(Mutex::new(Vec::new())),
            shinkai_manager,
        }
    }

    // start synchronization
    pub async fn synchronize(&self) -> anyhow::Result<()> {
        let mut syncing_folders_lock = self.syncing_folders.lock().unwrap();
        let mut syncing_queue_lock = self.syncing_queue.lock().unwrap();

        dbg!(syncing_queue_lock.clone());

        while let Some(sync_queue_item) = syncing_queue_lock.pop() {
            // Check if the folder exists on the node, if not, create it
            let node_folder_path = &sync_queue_item
                .syncing_folder
                .vector_fs_path
                .clone()
                .unwrap_or_default();

            self.ensure_folder_path_exists(node_folder_path).await?;

            println!(
                "local os path: {:?}, node vector_fs path: {}",
                sync_queue_item.clone().os_file_path.clone(),
                node_folder_path
            );

            let file_bytes = std::fs::read(&sync_queue_item.os_file_path).expect("Failed to read file.");
            let filename = sync_queue_item
                .os_file_path
                .file_name()
                .expect("Filename not found.")
                .to_str()
                .expect("Couldn't convert file name to str");

            let uploaded_file = self.shinkai_manager.clone().upload_file(&file_bytes, filename).await;

            if uploaded_file.is_ok() {
                if let Some(vector_fs_path) = &sync_queue_item.syncing_folder.vector_fs_path {
                    let node_folder_path_key = LocalOSFolderPath(vector_fs_path.clone());
                    if let Some(syncing_folder) = syncing_folders_lock.get_mut(&node_folder_path_key) {
                        syncing_folder.last_synchronized_file_datetime = Some(sync_queue_item.file_datetime);
                    }
                }
                syncing_queue_lock.retain(|item| item.os_file_path != sync_queue_item.os_file_path);
            }
        }

        Ok(())
    }

    pub fn add_syncing_folder(&mut self, folder: SyncingFolder) -> Result<(), String> {
        let path_buf = PathBuf::from(&folder.local_os_folder_path.0);
        if !path_buf.exists() || !path_buf.is_dir() {
            return Err("Specified path does not exist or is not a directory".to_string());
        }

        let mut folders = self.syncing_folders.lock().unwrap();
        let local_os_folder_path = folder.local_os_folder_path.clone();
        if let std::collections::hash_map::Entry::Vacant(e) = folders.entry(local_os_folder_path) {
            e.insert(folder);
            Ok(())
        } else {
            Err("Folder already exists".to_string())
        }
    }

    pub fn get_current_syncing_folders_map(&self) -> HashMap<LocalOSFolderPath, SyncingFolder> {
        let folders_lock = self.syncing_folders.lock().unwrap();
        folders_lock.clone()
    }

    pub fn stop(self) -> Result<HashMap<LocalOSFolderPath, SyncingFolder>, String> {
        self.syncing_folders
            .lock()
            .map_err(|e| format!("Failed to lock syncing_folders: {}", e))
            .map(|folders| folders.clone())
    }

    pub async fn ensure_folder_path_exists(&self, folder_path: &str) -> anyhow::Result<()> {
        let parts: Vec<&str> = folder_path
            .split('/')
            .filter(|p| !p.is_empty() && !p.contains('.'))
            .collect();
        let mut current_path = String::from("/");

        for part in parts.iter() {
            let check_path = format!("{}{}", current_path, part);

            match self.shinkai_manager.clone().get_node_folder(&check_path).await {
                Ok(_) => println!("{} already exists on the node.", check_path),
                Err(e) => {
                    eprintln!("Folder not found {:?}", e);
                    if let PostRequestError::FSFolderNotFound(_) = e {
                        if let Err(e) = self.shinkai_manager.clone().create_folder(part, &current_path).await {
                            eprintln!("Failed to create folder {} on the node: {:?}", check_path, e);
                        }
                    }
                }
            }
            current_path = format!("{}/", check_path);
        }

        Ok(())
    }

    pub fn visit_dirs(&mut self, dir: &Path) -> std::io::Result<()> {
        if dir.is_dir() {
            let entries = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect::<Vec<_>>();

            for entry in entries {
                let path = entry.path();

                dbg!(&path);
                if path.is_dir() {
                    let path_str = path.to_str().unwrap_or_default().to_string();
                    let local_os_folder_path = LocalOSFolderPath(path_str.clone());

                    let syncing_folder = SyncingFolder {
                        profile_name: self.shinkai_manager.profile_name.clone(),
                        vector_fs_path: Some(generate_relative_path(&Path::new(&local_os_folder_path.0))),
                        local_os_folder_path: local_os_folder_path.clone(),
                        last_synchronized_file_datetime: None,
                    };
                    self.add_syncing_folder(syncing_folder).unwrap();
                    self.visit_dirs(&path)?;
                }
            }
        }
        Ok(())
    }

    pub fn scan_local_os_syncing_updates(&self, syncing_folder: &SyncingFolder) -> std::io::Result<()> {
        let path = Path::new(&syncing_folder.local_os_folder_path.0);
        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.scan_local_os_syncing_updates(syncing_folder).unwrap();
                } else {
                    let metadata = std::fs::metadata(&path)?;
                    let modified_time = metadata
                        .modified()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    if syncing_folder
                        .last_synchronized_file_datetime
                        .map_or(true, |last_sync| modified_time > last_sync)
                    {
                        let mut syncing_queue_lock = self.syncing_queue.lock().unwrap();
                        syncing_queue_lock.push(SyncQueueItem {
                            syncing_folder: syncing_folder.clone(),
                            os_file_path: path,
                            file_datetime: modified_time,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

fn generate_relative_path(os_file_path: &Path) -> String {
    let node_fs_path = os_file_path
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(&os_file_path)
        .to_path_buf();
    node_fs_path.to_string_lossy().into_owned()
}
