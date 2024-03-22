#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_file_synchronizer::synchronizer::{LocalOSFolderPath, SyncingFolder};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use shinkai_file_synchronizer::persistent::Storage;
    use shinkai_file_synchronizer::persistent::StorageData;
    use tempfile::tempdir;

    // TODO: fix the test and remove ignore flag
    #[ignore]
    #[test]
    fn test_write_and_read_sync_folders() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(
            dir.path().to_str().unwrap().to_string(),
            "test_sync_folders.json".to_string(),
        );

        let mut folders = HashMap::new();
        folders.insert(
            LocalOSFolderPath("path/to/local/folder".to_string()),
            SyncingFolder {
                profile_name: Some("profile1".to_string()),
                vector_fs_path: Some("path/to/vector/fs".to_string()),
                local_os_folder_path: LocalOSFolderPath("path/to/local/folder".to_string()),
                last_synchronized_file_datetime: Some(1625097600),
            },
        );

        let folders_arc_mutex = Arc::new(Mutex::new(folders));

        let folders = folders_arc_mutex.lock().unwrap();
        storage.write_sync_folders(folders.clone()).unwrap();

        let read_sync_folder = storage.read_sync_folders().unwrap();
        let read_folders: Arc<Mutex<HashMap<LocalOSFolderPath, SyncingFolder>>> =
            Arc::new(Mutex::new(read_sync_folder));

        let original_folders = folders_arc_mutex.lock().unwrap();
        let deserialized_folders = read_folders.lock().unwrap();

        assert_eq!(original_folders.len(), deserialized_folders.len());
        for (key, original_folder) in original_folders.iter() {
            let deserialized_folder = deserialized_folders.get(key).unwrap();
            assert_eq!(original_folder.profile_name, deserialized_folder.profile_name);
            assert_eq!(original_folder.vector_fs_path, deserialized_folder.vector_fs_path);
            assert_eq!(
                original_folder.local_os_folder_path,
                deserialized_folder.local_os_folder_path
            );
            assert_eq!(
                original_folder.last_synchronized_file_datetime,
                deserialized_folder.last_synchronized_file_datetime
            );
        }
    }
}
