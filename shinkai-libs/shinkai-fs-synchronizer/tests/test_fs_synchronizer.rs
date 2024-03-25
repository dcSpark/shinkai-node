// TODO: if we want to run the node in the tests, how to import that? Or maybe just run a binary?
// use shinkai_node::network::node_api::{APIError, SendResponseBodyData};
// use shinkai_node::network::Node;

#[cfg(test)]
mod tests {
    use shinkai_file_synchronizer::{
        shinkai_manager::ShinkaiManager,
        synchronizer::{FilesystemSynchronizer, LocalOSFolderPath, SyncingFolder},
        visitor::{traverse_and_initialize_local_state, DirectoryVisitor, SyncFolderVisitor},
    };

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
        fn visit_dirs(&self, dir: &Path, last_synced_time: Option<u64>) -> std::io::Result<()> {
            if dir.is_dir() {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.is_dir() {
                        println!("Directory: {:?}", path);

                        // Recursively visit subdirectories
                        self.visit_dirs(&path, None)?;
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

    #[tokio::test]
    async fn test_traverse_and_synchronize_visits_all_files() {
        use std::path::Path;
        dotenv::dotenv().ok();

        let major_directory = "tests/knowledge/";
        let knowledge_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(major_directory);

        let shinkai_manager = ShinkaiManager::initialize_from_encrypted_file().unwrap();

        let syncing_folders = Arc::new(Mutex::new(HashMap::<LocalOSFolderPath, SyncingFolder>::new()));
        let _synchronizer = FilesystemSynchronizer::new(shinkai_manager, syncing_folders, major_directory.to_string());

        let visited_files = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let mock_visitor = MockDirectoryVisitor {
            visited_files: visited_files.clone(),
        };

        traverse_and_initialize_local_state::<(), MockDirectoryVisitor>(knowledge_dir.to_str().unwrap(), &mock_visitor);
        let visited = visited_files.lock().unwrap();

        assert_eq!(visited.len(), 7);
    }

    #[tokio::test]
    async fn test_create_initial_syncfolder_hashmap() {
        use std::path::Path;
        dotenv::dotenv().ok();

        // Setup - specify the main directory structure
        let major_directory = "tests/knowledge/";
        let knowledge_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(major_directory);

        let shinkai_manager = ShinkaiManager::initialize_from_encrypted_file()
            .expect("Couldn't initialize ShinkaiManager from the provided encrypted file.");

        let syncing_folders = Arc::new(Mutex::new(HashMap::new()));
        let sync_visitor = SyncFolderVisitor::new(syncing_folders, None, shinkai_manager.profile_name.clone());
        traverse_and_initialize_local_state::<(), SyncFolderVisitor>(knowledge_dir.to_str().unwrap(), &sync_visitor);

        let synchronizer = FilesystemSynchronizer::new(
            shinkai_manager,
            sync_visitor.syncing_folders,
            major_directory.to_string(),
        );
        synchronizer.synchronize().await;
    }

    // #[tokio::test]
    // async fn test_shinkai_node_initializer() {
    //     Box::pin(async move {
    //         // initialize shinkai manager only after the node is initialized and started locally
    //         // let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    //         // let mut node1 = Node::new(
    //         //     node1_identity_name.to_string(),
    //         //     addr1,
    //         //     clone_signature_secret_key(&node1_identity_sk),
    //         //     node1_encryption_sk,
    //         //     0,
    //         //     node1_commands_receiver,
    //         //     node1_db_path,
    //         //     true,
    //         //     vec![],
    //         //     None,
    //         //     node1_fs_db_path,
    //         //     None,
    //         //     None,
    //         // );

    //         let shinkai_manager = node_init().await;

    //         // {
    //         //     // Register a Profile in Node1 and verifies it
    //         //     eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
    //         //     api_initial_registration_with_no_code_for_device(
    //         //         node1_commands_sender.clone(),
    //         //         env.node1_profile_name.as_str(),
    //         //         env.node1_identity_name.as_str(),
    //         //         node1_encryption_pk,
    //         //         node1_device_encryption_sk.clone(),
    //         //         clone_signature_secret_key(&node1_device_identity_sk),
    //         //         node1_profile_encryption_sk.clone(),
    //         //         clone_signature_secret_key(&node1_profile_identity_sk),
    //         //         node1_device_name.as_str(),
    //         //     )
    //         //     .await;
    //         // }
    //     });
    // }
}
