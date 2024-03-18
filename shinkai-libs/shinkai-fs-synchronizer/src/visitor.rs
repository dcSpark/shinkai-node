use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
};

use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;

use crate::synchronizer::{FilesystemSynchronizer, LocalOSFolderPath, SyncingFolder};

pub trait DirectoryVisitor {
    fn visit_dirs(&self, dir: &Path) -> std::io::Result<()>;
}

// TODO: define if we really need that - to be removed
// impl DirectoryVisitor for FilesystemSynchronizer {
//     fn visit_dirs(&self, dir: &Path) -> std::io::Result<()> {
//         if dir.is_dir() {
//             for entry in fs::read_dir(dir)? {
//                 let entry = entry?;
//                 let path = entry.path();

//                 if path.is_dir() {
//                     println!("Directory: {:?}", path);
//                     // check if directory already exists in specific place on the Node
//                     // if it does, proceed
//                     // if it doesn't create it

//                     // TODO: edge case to be handled differently: if the folder on the disk was moved or deleted, but it is found in specific place on the node vector_fs, remove the whole directory on the vector_fs

//                     self.visit_dirs(&path)?;
//                 } else {
//                     // check all the files inside the directory - one by one
//                     // if the file is not found in the specific place on the node vector_fs, save it there

//                     // if the file is found in the specific place on the node vector_fs, check if it is up to date
//                     // if it is up to date, do nothing
//                     // if it is not up to date, save the new one (it will be overwritten)

//                     // because we're doing recursive search, we just need to exit at this point in here
//                 }
//             }
//         }

//         Ok(())
//     }
// }

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
            profile_name: "".to_string(),
            vector_fs_path: "".to_string(),
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
                    self.visit_dirs(&path)?; // Recursive call to visit subdirectories
                }
                // If you also want to process files, you can do it here
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
