use ed25519_dalek::SigningKey;
use shinkai_file_synchronizer::synchronizer::FilesystemSynchronizer;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ProfileName;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[cfg(test)]
mod tests {
    use shinkai_file_synchronizer::{shinkai_manager::ShinkaiManager, synchronizer::DirectoryVisitor};

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
        use std::fs::{self, File};
        use std::io::Write;
        use std::path::Path;

        // Setup - specify the main directory structure
        let knowledge_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/knowledge/");

        let shinkai_manager = ShinkaiManager::new(
            EncryptionStaticKey::new(rand::rngs::OsRng),
            SigningKey::from_bytes(&[0; 32]),
            EncryptionPublicKey::from([0; 32]),
            ProfileName::default(),
            String::default(),
            "".to_string(),
            "".to_string(),
        );

        // Initialize the synchronizer with the main directory
        let client_keypairs = vec![];
        let syncing_folders = HashMap::new();
        let synchronizer = FilesystemSynchronizer::new(
            shinkai_manager,
            knowledge_dir.to_str().unwrap(),
            client_keypairs,
            syncing_folders,
        );

        // Use a shared variable to track visited files
        let visited_files = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let mock_visitor = MockDirectoryVisitor {
            visited_files: visited_files.clone(),
        };
        // Act - call the method under test with the mock
        // Specifying the type parameter explicitly due to type inference issue
        synchronizer
            .traverse_and_synchronize::<(), MockDirectoryVisitor>(knowledge_dir.to_str().unwrap(), &mock_visitor);
        // Assert - check if all files were visited
        // Note: The actual number of visited files will depend on the current state of the directory
        // For the purpose of this test, we're assuming the directory structure and files are pre-set and known
        let visited = visited_files.lock().unwrap();

        dbg!(visited.len());
        // The expected count should be adjusted based on the actual directory structure
        assert_eq!(visited.len(), 7);
        dbg!(visited);
        // assert!(visited.contains(&knowledge_dir.join("file1.txt")));
        // assert!(visited.contains(&knowledge_dir.join("test_1/file2.txt")));
    }
}
