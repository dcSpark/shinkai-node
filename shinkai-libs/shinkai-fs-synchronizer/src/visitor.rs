use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::synchronizer::{LocalOSFolderPath, SyncingFolder};

pub trait DirectoryVisitor {
    fn visit_dirs(&self, dir: &Path) -> std::io::Result<()>;
}

pub struct SyncFolderVisitor {
    pub syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>,
}

impl SyncFolderVisitor {
    pub fn new(syncing_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>>) -> Self {
        SyncFolderVisitor { syncing_folders }
    }
}

impl DirectoryVisitor for SyncFolderVisitor {
    fn visit_dirs(&self, dir: &Path) -> std::io::Result<()> {
        let path_str = dir.to_str().unwrap_or_default().to_string();
        let local_os_folder_path = LocalOSFolderPath(path_str.clone());
        let syncing_folder = SyncingFolder {
            profile_name: None,
            vector_fs_path: None,
            local_os_folder_path: local_os_folder_path.clone(),
            last_synchronized_file_datetime: None,
        };

        {
            let mut folders = self.syncing_folders.lock().unwrap();
            folders.insert(local_os_folder_path, syncing_folder);
        } // Release the lock immediately after use

        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.visit_dirs(&path)?;
                }
            }
        }

        Ok(())
    }
}

pub fn traverse_and_synchronize<F, D>(major_directory_path: &str, visitor: &D)
where
    D: DirectoryVisitor,
{
    let major_directory_path = Path::new(major_directory_path);

    if major_directory_path.is_dir() {
        match visitor.visit_dirs(major_directory_path) {
            Ok(_) => println!("Traversal complete."),
            Err(e) => println!("Error during traversal: {}", e),
        }
    } else {
        println!("The provided path is not a directory.");
    }
}
