use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use shinkai_vector_resources::vector_resource::SimplifiedFSRoot;

use crate::synchronizer::{LocalOSFolderPath, SyncQueueItem, SyncingFolder};

pub trait DirectoryVisitor {
    fn visit_dirs(
        &self,
        dir: &Path,
        fs_root: SimplifiedFSRoot,
        last_synchronized_file_datetime: Option<u64>,
    ) -> std::io::Result<()>;
}

pub struct SyncFolderVisitor {
    pub syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>,
    pub syncing_queue: Arc<Mutex<Vec<SyncQueueItem>>>,
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
            syncing_queue: Arc::new(Mutex::new(Vec::new())),
            node_profile_assigned,
        }
    }
}

impl DirectoryVisitor for SyncFolderVisitor {
    fn visit_dirs(
        &self,
        dir: &Path,
        fs_root: SimplifiedFSRoot,
        last_synchronized_file_datetime: Option<u64>,
    ) -> std::io::Result<()> {
        if dir.is_dir() {
            let mut entries = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect::<Vec<_>>();

            for entry in &entries {
                let path = entry.path();

                // If the entry is a directory, recursively visit it
                if path.is_dir() {
                    self.visit_dirs(&path, fs_root.clone(), last_synchronized_file_datetime)?;
                } else if path.is_file() {
                    let metadata = entry.metadata()?;
                    let modified_time = metadata
                        .modified()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    let local_os_dir_full_str = dir.to_str().unwrap();
                    let local_os_relative_dir_str = generate_relative_path(dir);
                    let dir_components: Vec<&str> = local_os_relative_dir_str.split('/').collect();

                    let fs_root_last_modified_time = fs_root.last_written_datetime.timestamp() as u64;
                    let mut last_modified_time = 0u64;

                    'outer: for folder in &fs_root.child_folders {
                        let mut path_ids = folder.path.path_ids.iter().map(|s| s.as_str()).collect::<Vec<&str>>();
                        if path_ids == dir_components {
                            last_modified_time = folder.last_written_datetime.timestamp() as u64;
                            break 'outer;
                        }
                    }

                    if modified_time > last_modified_time {
                        let os_file_path = path.to_path_buf();
                        let file_datetime = metadata
                            .modified()?
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        let vector_fs_path = Some(dir_components.join("/"));
                        let syncing_folder = SyncingFolder {
                            profile_name: self.node_profile_assigned.clone(),
                            vector_fs_path: vector_fs_path.clone(),
                            local_os_folder_path: LocalOSFolderPath(local_os_dir_full_str.to_string()),
                            last_synchronized_file_datetime: Some(fs_root_last_modified_time),
                        };

                        let sync_queue_item = SyncQueueItem {
                            syncing_folder,
                            os_file_path,
                            file_datetime,
                        };

                        let mut sync_queue = self.syncing_queue.lock().unwrap();
                        sync_queue.push(sync_queue_item);
                        sync_queue.sort_by_key(|item| item.file_datetime);
                    }
                }
            }
        }

        Ok(())
    }

    // fn visit_dirs(
    //     &self,
    //     dir: &Path,
    //     fs_root: SimplifiedFSRoot,
    //     last_synchronized_file_datetime: Option<u64>,
    // ) -> std::io::Result<()> {
    //     if dir.is_dir() {
    //         let mut entries = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect::<Vec<_>>();

    //         for entry in entries {
    //             let path = entry.path();

    //             dbg!(&path);
    //             if path.is_file() {
    //                 // Check if the entry is a file
    //                 let metadata = entry.metadata()?;

    //                 let modified_time = metadata
    //                     .modified()
    //                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    //                     .elapsed()
    //                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    //                     .as_secs();

    //                 let created_time = metadata
    //                     .created()
    //                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    //                     .elapsed()
    //                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    //                     .as_secs();

    //                 let path_str = path.to_str().unwrap_or_default().to_string();
    //                 let local_os_folder_path = LocalOSFolderPath(path_str.clone());

    //                 let mut insert_file = false;
    //                 let syncing_folder = {
    //                     let folders = self.syncing_folders.lock().unwrap();
    //                     if let Some(folder) = folders.get(&local_os_folder_path) {
    //                         match folder.last_synchronized_file_datetime {
    //                             Some(last_sync_time)
    //                                 if modified_time > last_sync_time || created_time > last_sync_time =>
    //                             {
    //                                 insert_file = true;
    //                             }
    //                             None => {
    //                                 insert_file = true;
    //                             }
    //                             _ => {}
    //                         }
    //                         folder.clone()
    //                     } else {
    //                         insert_file = true;
    //                         SyncingFolder {
    //                             profile_name: self.node_profile_assigned.clone(),
    //                             vector_fs_path: Some(generate_relative_path(&Path::new(&local_os_folder_path.0))),
    //                             local_os_folder_path: local_os_folder_path.clone(),
    //                             last_synchronized_file_datetime: None,
    //                         }
    //                     }
    //                 };

    //                 // we only insert if something is newer than or new
    //                 if insert_file {
    //                     let mut folders = self.syncing_folders.lock().unwrap();
    //                     folders.insert(local_os_folder_path, syncing_folder);
    //                 }
    //             } else if path.is_dir() {
    //                 // Recursively visit subdirectories
    //                 self.visit_dirs(&path, fs_root.clone(), last_synchronized_file_datetime)?;
    //             }
    //         }
    //     }

    //     Ok(())
    // }
}

pub fn traverse_and_initialize_local_state<F, D>(major_directory_path: &str, fs_root: SimplifiedFSRoot, visitor: &D)
where
    D: DirectoryVisitor,
{
    let major_directory_path = Path::new(major_directory_path);

    if major_directory_path.is_dir() {
        match visitor.visit_dirs(major_directory_path, fs_root, None) {
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
