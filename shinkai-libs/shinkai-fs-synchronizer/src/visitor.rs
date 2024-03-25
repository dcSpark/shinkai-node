use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;

use crate::synchronizer::{LocalOSFolderPath, SyncingFolder};

pub trait DirectoryVisitor {
    fn visit_dirs(&self, dir: &Path, last_synchronized_file_datetime: Option<u64>) -> std::io::Result<()>;
}

pub struct SyncFolderVisitor {
    pub syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>,
    pub node_profile_assigned: ProfileName,
    pub last_synced_time: Option<u64>,
}

impl SyncFolderVisitor {
    pub fn new(
        syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>,
        last_synced_time: Option<u64>,
        node_profile_assigned: ProfileName,
    ) -> Self {
        SyncFolderVisitor {
            syncing_folders,
            last_synced_time,
            node_profile_assigned,
        }
    }
}

impl DirectoryVisitor for SyncFolderVisitor {
    fn visit_dirs(&self, dir: &Path, last_synchronized_file_datetime: Option<u64>) -> std::io::Result<()> {
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let metadata = entry.metadata()?;

                // TODO: change comparison time to milliseconds
                let modified_time = metadata
                    .modified()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    .elapsed()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    .as_secs();

                let created_time = metadata
                    .created()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    .elapsed()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
                    .as_secs();

                let path_str = path.to_str().unwrap_or_default().to_string();
                let local_os_folder_path = LocalOSFolderPath(path_str.clone());

                let mut insert_file = false;
                let syncing_folder = {
                    let folders = self.syncing_folders.lock().unwrap();
                    if let Some(folder) = folders.get(&local_os_folder_path) {
                        match folder.last_synchronized_file_datetime {
                            Some(last_sync_time) if modified_time > last_sync_time || created_time > last_sync_time => {
                                insert_file = true;
                            }
                            None => {
                                insert_file = true;
                            }
                            _ => {}
                        }
                        folder.clone()
                    } else {
                        insert_file = true;
                        SyncingFolder {
                            profile_name: self.node_profile_assigned.clone(),
                            vector_fs_path: Some(generate_relative_path(&Path::new(&local_os_folder_path.0))),
                            local_os_folder_path: local_os_folder_path.clone(),
                            last_synchronized_file_datetime: None,
                        }
                    }
                };

                if insert_file {
                    let mut folders = self.syncing_folders.lock().unwrap();
                    folders.insert(local_os_folder_path, syncing_folder);
                }

                if path.is_dir() {
                    self.visit_dirs(&path, last_synchronized_file_datetime)?;
                }
            }
        }

        Ok(())
    }
}

pub fn traverse_and_initialize_local_state<F, D>(major_directory_path: &str, visitor: &D)
where
    D: DirectoryVisitor,
{
    let major_directory_path = Path::new(major_directory_path);

    if major_directory_path.is_dir() {
        match visitor.visit_dirs(major_directory_path, None) {
            Ok(_) => println!("Traversal complete."),
            Err(e) => println!("Error during traversal: {}", e),
        }
    } else {
        println!("The provided path is not a directory.");
    }
}

fn generate_relative_path(os_file_path: &Path) -> String {
    let node_fs_path = os_file_path
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(&os_file_path)
        .to_path_buf();
    node_fs_path.to_string_lossy().into_owned()
}
