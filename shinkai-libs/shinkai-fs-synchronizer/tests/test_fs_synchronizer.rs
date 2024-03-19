use ed25519_dalek::SigningKey;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[cfg(test)]
mod tests {
    use shinkai_file_synchronizer::{
        shinkai_manager::ShinkaiManager,
        synchronizer::FilesystemSynchronizer,
        visitor::{traverse_and_synchronize, DirectoryVisitor, SyncFolderVisitor},
    };
    use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;

    use super::*;
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

    // custom directory visitor to be able to verify what we need
    struct MockDirectoryVisitor {
        visited_files: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl DirectoryVisitor for MockDirectoryVisitor {
        // TODO: identify what's better wayt to reue visit_dirs function
        fn visit_dirs(&self, dir: &Path) -> std::io::Result<()> {
            if dir.is_dir() {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.is_dir() {
                        println!("Directory: {:?}", path);

                        // Recursively visit subdirectories
                        self.visit_dirs(&path)?;
                    } else {
                        // After handling the file, add it to the visited list
                        let mut visited = self.visited_files.lock().unwrap();
                        visited.push(path);
                    }
                }
            }

            Ok(())
        }
    }

    #[test]
    fn test_traverse_and_synchronize_visits_all_files() {
        use std::path::Path;

        // Setup - specify the main directory structure
        let knowledge_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/knowledge/");

        let my_encryption_secret_key = EncryptionStaticKey::new(rand::rngs::OsRng);
        let my_signature_secret_key = SigningKey::from_bytes(&[0; 32]);
        let receiver_public_key = EncryptionPublicKey::from([0; 32]);

        let shinkai_manager = ShinkaiManager::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            ProfileName::default(),
            String::default(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        );

        let syncing_folders = HashMap::new();
        let _synchronizer = FilesystemSynchronizer::new(shinkai_manager, syncing_folders);

        let visited_files = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let mock_visitor = MockDirectoryVisitor {
            visited_files: visited_files.clone(),
        };

        traverse_and_synchronize::<(), MockDirectoryVisitor>(knowledge_dir.to_str().unwrap(), &mock_visitor);
        let visited = visited_files.lock().unwrap();

        assert_eq!(visited.len(), 7);
    }

    #[tokio::test]
    async fn test_create_initial_syncfolder_hashmap() {
        use std::path::Path;

        // Setup - specify the main directory structure
        let knowledge_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/knowledge/");

        let my_encryption_secret_key = EncryptionStaticKey::new(rand::rngs::OsRng);
        let my_signature_secret_key = SigningKey::from_bytes(&[0; 32]);
        let receiver_public_key = EncryptionPublicKey::from([0; 32]);

        let shinkai_manager = ShinkaiManager::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            ProfileName::default(),
            String::default(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        );

        let syncing_folders = Arc::new(Mutex::new(HashMap::new()));
        let sync_visitor = SyncFolderVisitor::new(syncing_folders);
        traverse_and_synchronize::<(), SyncFolderVisitor>(knowledge_dir.to_str().unwrap(), &sync_visitor);

        let syncing_folders = sync_visitor.syncing_folders.lock().unwrap().clone();

        let synchronizer = FilesystemSynchronizer::new(shinkai_manager, syncing_folders);
        synchronizer.synchronize().await;
    }
}
