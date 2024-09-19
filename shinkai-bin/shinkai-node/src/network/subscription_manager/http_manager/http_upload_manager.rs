use std::{
    collections::HashMap,
    env,
    sync::{Arc, Weak},
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::{
    schemas::{
        file_links::{FileLink, FileMapPath, FileStatus, FolderSubscriptionWithPath, SubscriptionStatus},
        shinkai_name::ShinkaiName,
    },
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use shinkai_vector_fs::vector_fs::{vector_fs::VectorFS, vector_fs_permissions::ReadPermission};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, VRPath};
use tokio::sync::Semaphore;

use crate::network::subscription_manager::{
    external_subscriber_manager::SharedFolderInfo, fs_entry_tree::FSEntryTree,
    fs_entry_tree_generator::FSEntryTreeGenerator,
};

use super::{
    http_upload_error::HttpUploadError,
    subscription_file_uploader::{
        delete_file_or_folder, generate_temporary_shareable_link, list_folder_contents, upload_file_http,
        FileDestination,
    },
};

#[allow(dead_code)]
const UPLOAD_CONCURRENCY: usize = 2;

pub struct HttpSubscriptionUploadManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub node_name: ShinkaiName,
    pub is_syncing: bool,
    pub subscription_file_map: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>>,
    pub subscription_status: Arc<DashMap<FolderSubscriptionWithPath, SubscriptionStatus>>, // TODO: extend to support profiles
    pub shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>, // (streamer_profile:::path, shared_folder)
    pub file_links: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>>,
    pub subscription_processing_task: tokio::task::JoinHandle<()>,
    pub semaphore: Arc<Semaphore>, // Semaphore to control concurrent execution
    pub subscription_http_upload_concurrency: usize,
}

impl Clone for HttpSubscriptionUploadManager {
    fn clone(&self) -> Self {
        HttpSubscriptionUploadManager {
            db: self.db.clone(),
            vector_fs: self.vector_fs.clone(),
            node_name: self.node_name.clone(),
            is_syncing: self.is_syncing,
            subscription_file_map: self.subscription_file_map.clone(),
            subscription_status: self.subscription_status.clone(),
            shared_folders_trees_ref: self.shared_folders_trees_ref.clone(),
            file_links: self.file_links.clone(),
            // We cannot clone a JoinHandle, so we provide a new no-op task or a default placeholder
            subscription_processing_task: tokio::task::spawn(async {}),
            semaphore: self.semaphore.clone(),
            subscription_http_upload_concurrency: self.subscription_http_upload_concurrency,
        }
    }
}

impl HttpSubscriptionUploadManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>,
    ) -> Self {
        eprintln!(">>> Starting HttpSubscriptionUploadManager");
        let subscription_file_map = Arc::new(DashMap::new());
        let subscription_status = Arc::new(DashMap::new());
        let file_links = Arc::new(DashMap::new());
        let semaphore = Arc::new(Semaphore::new(1)); // so we can force updates without race conditions

        let subscription_http_upload_concurrency = env::var("SUBSCRIPTION_HTTP_UPLOAD_CONCURRENCY")
            .unwrap_or(UPLOAD_CONCURRENCY.to_string())
            .parse::<usize>()
            .unwrap_or(UPLOAD_CONCURRENCY); // Start processing the job queue

        // Restore subscription_file_map from the database
        if let Some(db_strong) = db.upgrade() {
            match db_strong.read_all_file_links() {
                Ok(all_file_links) => {
                    for (folder_subs_with_path, file_links_map) in all_file_links {
                        // Update file_links instead of file_status_map
                        file_links.insert(folder_subs_with_path.clone(), file_links_map);

                        // Update subscription status to Ready
                        subscription_status.insert(folder_subs_with_path.clone(), SubscriptionStatus::Ready);
                    }
                }
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::SubscriptionHTTPUploader,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to read file links from database: {:?}", e),
                    );
                }
            }
        }

        let subscription_processing_task = HttpSubscriptionUploadManager::process_subscription_http_checks(
            db.clone(),
            vector_fs.clone(),
            node_name.clone(),
            subscription_file_map.clone(),
            subscription_status.clone(),
            shared_folders_trees_ref.clone(),
            file_links.clone(),
            subscription_http_upload_concurrency,
            semaphore.clone(),
        )
        .await;

        HttpSubscriptionUploadManager {
            db,
            vector_fs,
            node_name,
            is_syncing: false,
            subscription_file_map,
            subscription_status,
            shared_folders_trees_ref,
            file_links,
            subscription_processing_task,
            semaphore,
            subscription_http_upload_concurrency,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_subscription_http_checks(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        subscription_file_map: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>>,
        subscription_status: Arc<DashMap<FolderSubscriptionWithPath, SubscriptionStatus>>,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>, // (streamer_profile:::path, shared_folder)
        file_links: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>>,
        subscription_http_upload_concurrency: usize, // simultaneous uploads
        semaphore: Arc<Semaphore>,
    ) -> tokio::task::JoinHandle<()> {
        let interval_minutes = env::var("SUBSCRIPTION_HTTP_UPLOAD_INTERVAL_MINUTES")
            .unwrap_or("5".to_string())
            .parse::<u64>()
            .unwrap_or(5);

        let is_testing = env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);
        if is_testing {
            // If we are testing, we don't want to run the subscription processing task
            return tokio::task::spawn(async {});
        }

        tokio::spawn(async move {
            loop {
                match Self::controlled_subscription_http_check_loop(
                    db.clone(),
                    vector_fs.clone(),
                    node_name.clone(),
                    subscription_file_map.clone(),
                    subscription_status.clone(),
                    shared_folders_trees_ref.clone(),
                    file_links.clone(),
                    subscription_http_upload_concurrency,
                    semaphore.clone(),
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::SubscriptionHTTPUploader,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to process subscription: {:?}", e),
                        );
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(interval_minutes * 60)).await;
            }
        })
    }

    #[allow(dead_code)]
    pub async fn trigger_controlled_subscription_http_check(manager: &HttpSubscriptionUploadManager) {
        let _result = Self::controlled_subscription_http_check_loop(
            manager.db.clone(),
            manager.vector_fs.clone(),
            manager.node_name.clone(),
            manager.subscription_file_map.clone(),
            manager.subscription_status.clone(),
            manager.shared_folders_trees_ref.clone(),
            manager.file_links.clone(),
            manager.subscription_http_upload_concurrency,
            manager.semaphore.clone(),
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn controlled_subscription_http_check_loop(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        subscription_file_map: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>>,
        subscription_status: Arc<DashMap<FolderSubscriptionWithPath, SubscriptionStatus>>,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>,
        file_links: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>>,
        subscription_http_upload_concurrency: usize, // simultaneous uploads
        semaphore: Arc<Semaphore>,
    ) -> Result<(), HttpUploadError> {
        let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore permit");

        Self::subscription_http_check_loop(
            db,
            vector_fs,
            node_name,
            subscription_file_map,
            subscription_status,
            shared_folders_trees_ref,
            file_links,
            subscription_http_upload_concurrency,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn subscription_http_check_loop(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        subscription_file_map: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>>,
        subscription_status: Arc<DashMap<FolderSubscriptionWithPath, SubscriptionStatus>>,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>, // (streamer_profile:::path, shared_folder)
        file_links: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>>,
        subscription_http_upload_concurrency: usize, // simultaneous uploads
    ) -> Result<(), HttpUploadError> {
        match Self::get_profiles_and_shared_folders_with_empty_tree(db.clone(), vector_fs.clone(), node_name.clone())
            .await
        {
            Ok(profiles_and_folders) => {
                for (profile, shared_folders) in profiles_and_folders {
                    for shared_folder_info in shared_folders {
                        let result = Self::process_single_folder_subscription(
                            shared_folder_info,
                            node_name.clone(),
                            profile.clone(),
                            subscription_file_map.clone(),
                            subscription_status.clone(),
                            &db,
                            &vector_fs,
                            shared_folders_trees_ref.clone(),
                            file_links.clone(),
                            subscription_http_upload_concurrency,
                        )
                        .await;

                        if let Err(e) = result {
                            shinkai_log(
                                ShinkaiLogOption::SubscriptionHTTPUploader,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to process subscription: {:?}", e),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::SubscriptionHTTPUploader,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to get profiles and shared folders: {:?}", e),
                );
                return Err(HttpUploadError::DatabaseError(
                    "Failed to get profiles and shared folders".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub async fn get_profiles_and_shared_folders_with_empty_tree(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
    ) -> Result<HashMap<String, Vec<SharedFolderInfo>>, HttpUploadError> {
        let db_strong = db
            .upgrade()
            .ok_or_else(|| HttpUploadError::DatabaseError("Database instance is not available".to_string()))?;
        let identities = db_strong
            .get_all_profiles(node_name.clone())
            .map_err(|e| HttpUploadError::DatabaseError(e.to_string()))?;

        let mut profiles_folders_map: HashMap<String, Vec<SharedFolderInfo>> = HashMap::new();

        for identity in identities {
            let profile_name = identity.full_identity_name.clone().get_profile_name_string();
            if let Some(profile) = profile_name {
                let shared_folders = HttpSubscriptionUploadManager::fetch_shared_folders_for_profile_with_empty_tree(
                    db.clone(),
                    vector_fs.clone(),
                    node_name.clone(),
                    profile.clone(),
                )
                .await
                .map_err(|_| {
                    HttpUploadError::FileSystemError("Failed to fetch shared folders for profile".to_string())
                })?;
                profiles_folders_map.insert(profile, shared_folders);
            }
        }

        Ok(profiles_folders_map)
    }

    async fn fetch_shared_folders_for_profile_with_empty_tree(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        profile: String,
    ) -> Result<Vec<SharedFolderInfo>, HttpUploadError> {
        if profile.is_empty() {
            return Err(HttpUploadError::InvalidRequest("Profile cannot be empty".to_string()));
        };

        let db_strong = db
            .upgrade()
            .ok_or_else(|| HttpUploadError::DatabaseError("DB instance is not available".to_string()))?;

        let vector_fs_strong = vector_fs
            .upgrade()
            .ok_or_else(|| HttpUploadError::VectorFSNotAvailable("VectorFS instance is not available".to_string()))?;

        let root_path = VRPath::from_string("/").map_err(|e| HttpUploadError::InvalidRequest(e.to_string()))?;
        let full_requester = ShinkaiName::from_node_and_profile_names(node_name.node_name, profile.clone())?;

        let reader = vector_fs_strong
            .new_reader(full_requester.clone(), root_path, full_requester.clone())
            .await
            .map_err(|e| HttpUploadError::InvalidRequest(e.to_string()))?;

        let mut paths = vector_fs_strong
            .find_paths_with_read_permissions_as_vec(&reader, vec![ReadPermission::Public])
            .await
            .map_err(|_| HttpUploadError::FileSystemError("Failed to find paths with read permissions".to_string()))?;

        // Use the new function to filter results to only include top-level folders
        paths = FSEntryTreeGenerator::filter_to_top_level_folders(paths);

        let shared_folders = paths
            .into_iter()
            .map(|(path, permission)| {
                // clone the variables for the async block
                let db_clone = db_strong.clone();
                // let vector_fs_clone = vector_fs.clone();
                // let full_requester_clone = full_requester.clone();
                let profile = profile.clone();
                async move {
                    let path_str = path.to_string();
                    let permission_str = format!("{:?}", permission);
                    let subscription_requirement = match db_clone.get_folder_requirements(&path_str) {
                        Ok(req) => Some(req),
                        Err(_) => None,
                    };
                    let tree = FSEntryTree::new_empty();

                    Some(SharedFolderInfo {
                        path: path_str,
                        permission: permission_str,
                        profile: profile.clone(),
                        tree,
                        subscription_requirement,
                    })
                }
            })
            .filter_map(futures::executor::block_on) // Execute async block and filter out None results
            .collect();

        Ok(shared_folders)
    }

    #[allow(dead_code)]
    async fn generate_tree_for_shared_folder(
        vector_fs: Weak<VectorFS>,
        full_requester: ShinkaiName,
        path: String,
    ) -> Option<FSEntryTree> {
        FSEntryTreeGenerator::shared_folders_to_tree(vector_fs, full_requester.clone(), full_requester, path, vec![])
            .await
            .ok()
    }

    // Helper method to fetch subscriptions that require HTTP support
    #[allow(dead_code)]
    pub async fn fetch_subscriptions_with_http_support(db: &Weak<ShinkaiDB>) -> Vec<FolderSubscriptionWithPath> {
        let db = match db.upgrade() {
            Some(db) => db,
            None => {
                shinkai_log(
                    ShinkaiLogOption::SubscriptionHTTPUploader,
                    ShinkaiLogLevel::Error,
                    "Failed to upgrade Weak<ShinkaiDB> to a strong reference",
                );
                return Vec::new(); // Handle error appropriately
            }
        };

        match db.get_all_folder_requirements() {
            Ok(subscriptions) => subscriptions
                .into_iter()
                .filter_map(|(path, folder_subscription)| {
                    if folder_subscription.has_web_alternative.unwrap_or(false) {
                        Some(FolderSubscriptionWithPath {
                            path,
                            folder_subscription,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Database,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to fetch folder requirements: {:?}", e),
                );
                Vec::new() // Handle error appropriately
            }
        }
    }

    // Method to update the subscription status to NotStarted
    pub fn update_subscription_status_to_not_started(&self, folder_subs_with_path: &FolderSubscriptionWithPath) {
        self.subscription_status
            .insert(folder_subs_with_path.clone(), SubscriptionStatus::NotStarted);
    }

    // Extracted method to process individual folder subscriptions
    #[allow(clippy::too_many_arguments)]
    pub async fn process_single_folder_subscription(
        shared_folder_subs: SharedFolderInfo,
        node_name: ShinkaiName,
        profile: String,
        subscription_file_map: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>>,
        subscription_status: Arc<DashMap<FolderSubscriptionWithPath, SubscriptionStatus>>,
        db: &Weak<ShinkaiDB>,
        vector_fs: &Weak<VectorFS>,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>,
        file_links: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>>,
        subscription_http_upload_concurrency: usize, // simultaneous uploads
    ) -> Result<(), HttpUploadError> {
        // Check if the subscription requirement has_web_alternative set to true
        if let Some(subscription_requirement) = &shared_folder_subs.subscription_requirement {
            if subscription_requirement.has_web_alternative != Some(true) {
                return Ok(()); // No web alternative, so we skip this subscription
            }
        } else {
            return Ok(()); // No subscription requirement, so we skip this subscription
        }

        let key = format!("{}:::{}", profile.clone(), shared_folder_subs.path.clone());
        let streamer = ShinkaiName::from_node_and_profile_names(node_name.node_name, profile.clone())?;

        let subscription_expected_files = shared_folders_trees_ref
            .get(&key)
            .map(|shared_folder_info| shared_folder_info.tree.collect_all_paths())
            .unwrap_or_default();

        if subscription_expected_files.is_empty() {
            return Err(HttpUploadError::FileSystemError(
                "No files found in the shared folder tree".to_string(),
            )); // No files found in the shared folder tree
        }

        let folder_subs_with_path = FolderSubscriptionWithPath {
            path: shared_folder_subs.path.clone(),
            folder_subscription: shared_folder_subs.subscription_requirement.clone().ok_or(
                HttpUploadError::InvalidRequest("Missing subscription requirement".to_string()),
            )?,
        };

        // Update subscription status to Syncing
        subscription_status.insert(folder_subs_with_path.clone(), SubscriptionStatus::Syncing);

        let subscription_files = subscription_file_map
            .entry(folder_subs_with_path.clone())
            .or_default()
            .clone();

        let mut sync_file_paths: Vec<String> = subscription_files.keys().cloned().collect();

        drop(subscription_files);

        // Retrieve upload credentials from the database
        let db_strong = match db.upgrade() {
            Some(db) => db,
            None => {
                return Err(HttpUploadError::DatabaseError(
                    "Failed to upgrade Weak<ShinkaiDB> to a strong reference".to_string(),
                ))
            }
        };

        let credentials = db_strong
            .get_upload_credentials(&shared_folder_subs.path, &profile)
            .map_err(|e| HttpUploadError::DatabaseError(format!("Failed to retrieve upload credentials: {}", e)))?;

        let destination = FileDestination::from_credentials(credentials).await?;

        let was_sync_files_paths_empty = sync_file_paths.is_empty();
        if sync_file_paths.is_empty() {
            // Only required if subscription_files is empty (we just started). Otherwise use the local cache that should keep a 1 to 1 with the server
            let files = match list_folder_contents(&destination, &shared_folder_subs.path.clone()).await {
                Ok(files) => files
                    .into_iter()
                    .filter(|file| !file.is_folder)
                    .map(|file| {
                        let mut path = file.path;
                        if !path.starts_with('/') {
                            path = format!("/{}", path);
                        }
                        path
                    })
                    .collect::<Vec<String>>(),
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to list folder contents: {:?}", e),
                    );
                    return Err(HttpUploadError::ErrorGettingFolderContents);
                }
            };
            sync_file_paths = files;
        }

        // Create a hashmap to map each file to its checksum file if it exists
        let checksum_map: HashMap<String, String> = Self::extract_checksum_map(&sync_file_paths);

        // Update the subscription_file_map with the new files from the server
        if was_sync_files_paths_empty {
            for file_path in &sync_file_paths {
                let checksum = checksum_map.get(file_path).cloned().unwrap_or_default();
                subscription_file_map
                    .entry(folder_subs_with_path.clone())
                    .and_modify(|e| {
                        e.insert(file_path.clone(), FileStatus::Sync(checksum.clone()));
                    })
                    .or_insert_with(|| {
                        let mut map = HashMap::new();
                        map.insert(file_path.clone(), FileStatus::Sync(checksum.clone()));
                        map
                    });
            }
        }

        // We check file by file if it's in sync with the local storage or if anything needs to be deleted "extra"
        // Then we check if there are local files missing in the cloud provider

        // Check if all files are in sync
        let mut items_to_delete = Vec::new();
        let mut items_to_reupload = Vec::new();

        for potentially_sync_file in sync_file_paths.clone() {
            // Skip processing if the file ends with ".checksum"
            if potentially_sync_file.ends_with(".checksum") {
                continue;
            }

            let resource = match Self::retrieve_base_vr(&vector_fs.clone(), &potentially_sync_file, &streamer).await {
                Ok(res) => res,
                Err(e) => {
                    println!("Error retrieving base VR for file {}: {:?}", potentially_sync_file, e);
                    // We couldn't retrieve the file, so we mark it for deletion as it's not locally available anymore
                    items_to_delete.push(potentially_sync_file.clone());
                    continue;
                }
            };
            let current_hash = match resource.as_trait_object().get_merkle_root() {
                Ok(hash) => hash,
                Err(_) => {
                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Error,
                        "Failed to get the merkle root hash",
                    );
                    "".to_string() // Return an empty string to indicate failure
                }
            };

            // Check if the checksum matches
            let checksum_matches = if let Some(checksum_path) = checksum_map.get(&potentially_sync_file) {
                // Extract the last 8 characters of the hash from the checksum filename
                let expected_hash = checksum_path.to_string();

                // Extract the last 8 characters of the current hash
                let current_hash_last_8 = current_hash
                    .chars()
                    .rev()
                    .take(8)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();

                // Compare the last 8 characters of the expected hash with the last 8 characters of the current hash
                expected_hash == current_hash_last_8
            } else {
                false // No checksum file means we can't verify it, so assume it doesn't match
            };

            if !checksum_matches {
                items_to_delete.push(potentially_sync_file.clone());
                items_to_reupload.push(potentially_sync_file.clone());
            }
        }

        // Delete all the files that no longer exist locally
        // We also remove their checksum files
        for item_to_delete in items_to_delete {
            let delete_result = delete_file_or_folder(&destination, &item_to_delete).await;
            if let Err(e) = delete_result {
                return Err(HttpUploadError::from(e));
            }

            // Check if there is a checksum file associated with the file and delete it
            if let Some(checksum) = checksum_map.get(&item_to_delete) {
                let checksum_file_path = format!("{}.{}.checksum", item_to_delete, checksum);
                let delete_checksum_result = delete_file_or_folder(&destination, &checksum_file_path).await;
                if let Err(e) = delete_checksum_result {
                    return Err(HttpUploadError::from(e));
                }

                // Log the contents of the subscription_file_map before and after removing the checksum file
                if let Some(mut file_statuses) = subscription_file_map.get_mut(&folder_subs_with_path) {
                    file_statuses.remove(&checksum_file_path);
                }
            }

            // Log the contents of the subscription_file_map before and after removing the file
            if let Some(mut file_statuses) = subscription_file_map.get_mut(&folder_subs_with_path) {
                file_statuses.remove(&item_to_delete);
            }
        }

        // Check for files missing in the cloud and upload them.
        // Additionally, upload a checksum file for each file.
        // There is no need to upload outdated files as they were handled in the previous step.
        {
            // Convert sync_file_paths to a hashmap for quick lookup
            let sync_file_paths_map: HashMap<String, ()> = sync_file_paths.into_iter().map(|path| (path, ())).collect();

            // List of files missing in the cloud
            let mut missing_files: Vec<String> = Vec::new();

            // Add the files that need to be reuploaded
            missing_files.extend(items_to_reupload);

            // Iterate over expected files and check if they are in the sync_file_paths_map
            for expected_file in subscription_expected_files {
                if !sync_file_paths_map.contains_key(&expected_file) {
                    missing_files.push(expected_file);
                }
            }

            // Create a semaphore with a given number of permits
            let semaphore = Arc::new(tokio::sync::Semaphore::new(subscription_http_upload_concurrency));

            // Collect all tasks
            let mut tasks = Vec::new();

            // Define a struct to hold the results needed for updating the map
            struct TaskResult {
                missing_in_cloud_file_path: String,
                checksum_file_name: String,
                checksum: String,
            }

            // Upload missing files
            for missing_in_cloud_file_path in missing_files {
                let resource =
                    match Self::retrieve_base_vr(&vector_fs.clone(), &missing_in_cloud_file_path, &streamer).await {
                        Ok(res) => res,
                        Err(_) => {
                            continue;
                        }
                    };

                let cloned_resource = resource.clone();
                let vrkai_vec = cloned_resource.to_vrkai().encode_as_bytes().unwrap();
                let path = VRPath::from_string(&missing_in_cloud_file_path)?;
                let parent_path = path.parent_path().to_string();
                let destination_clone = destination.clone();
                let sema_clone = semaphore.clone();

                let task = tokio::spawn(async move {
                    let _permit = sema_clone.acquire().await.expect("Failed to acquire semaphore permit");
                    let cloned_resource = resource.clone();
                    let resource_trait = cloned_resource.as_trait_object();
                    let file_name = resource_trait.name();

                    let upload_result =
                        upload_file_http(vrkai_vec, &parent_path, file_name, destination_clone.clone()).await;
                    if let Err(e) = upload_result {
                        return Err(HttpUploadError::from(e));
                    }

                    // Generate and upload checksum file
                    let checksum = resource_trait.get_merkle_root().unwrap_or_default();
                    let checksum_file_name = Self::generate_checksum_filename(file_name, &checksum);
                    let checksum_contents = checksum.to_string().into_bytes();
                    let checksum_upload_result =
                        upload_file_http(checksum_contents, &parent_path, &checksum_file_name, destination_clone).await;
                    if let Err(e) = checksum_upload_result {
                        return Err(HttpUploadError::from(e));
                    }

                    Ok(TaskResult {
                        missing_in_cloud_file_path,
                        checksum_file_name,
                        checksum,
                    })
                });

                tasks.push(task);
            }

            // Wait for all tasks to complete
            let results = futures::future::join_all(tasks).await;
            for result in results {
                match result {
                    Ok(Ok(task_result)) => {
                        let path = VRPath::from_string(&task_result.missing_in_cloud_file_path)?;
                        let parent_path = path.parent_path().to_string();
                        let full_checksum_path = format!("{}/{}", parent_path, task_result.checksum_file_name);
                        subscription_file_map.alter(&folder_subs_with_path.clone(), |_key, mut existing_entry| {
                            existing_entry.insert(
                                task_result.missing_in_cloud_file_path.clone(),
                                FileStatus::Sync(task_result.checksum.clone()),
                            );
                            existing_entry.insert(full_checksum_path, FileStatus::Sync(task_result.checksum.clone()));
                            existing_entry
                        });
                    }
                    Ok(Err(e)) => {
                        // Log the HttpUploadError using shinkai_log
                        shinkai_log(
                            ShinkaiLogOption::ExtSubscriptions,
                            ShinkaiLogLevel::Error,
                            &format!("Upload task failed with HttpUploadError: {:?}", e),
                        );
                        return Err(e);
                    }
                    Err(e) => {
                        // Log the JoinError using shinkai_log
                        shinkai_log(
                            ShinkaiLogOption::ExtSubscriptions,
                            ShinkaiLogLevel::Error,
                            &format!("Upload task failed with JoinError: {:?}", e),
                        );
                        return Err(HttpUploadError::TaskJoinError(format!(
                            "Task failed with JoinError: {}",
                            e
                        )));
                    }
                }
            }

            // {
            //     // Print out the content of subscription_file_map
            //     for entry in subscription_file_map.iter() {
            //         let key = entry.key();
            //         let value = entry.value();
            //         println!("After everything - Folder Subscription: {:?}", key);
            //         for (file_path, status) in value.iter() {
            //             println!("  {} - {:?}", file_path, status);
            //         }
            //     }
            // }

            // Update subscription status to Syncing
            subscription_status.insert(folder_subs_with_path.clone(), SubscriptionStatus::WaitingForLinks);

            // Generate temporary shareable links for the files
            match Self::update_file_links(
                db,
                file_links,
                subscription_file_map,
                subscription_status,
                &destination,
                folder_subs_with_path.clone(),
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to update file links: {:?}", e),
                    );
                    return Err(e);
                }
            }

            Ok(())
        }
    }

    /// Generates a temporary shareable link for a file.
    pub async fn update_file_links(
        db: &Weak<ShinkaiDB>,
        file_links: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>>,
        subscription_file_map: Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>>,
        subscription_status: Arc<DashMap<FolderSubscriptionWithPath, SubscriptionStatus>>,
        destination: &FileDestination,
        folder_subs_with_path: FolderSubscriptionWithPath,
    ) -> Result<(), HttpUploadError> {
        // Read the expiration duration from an environment variable or default to 7 days (604800 seconds)
        let expiration_secs = std::env::var("LINK_EXPIRATION_SECONDS")
            .unwrap_or_else(|_| "604800".to_string())
            .parse::<u64>()
            .unwrap_or(604800);

        // Define a safe gap duration from an environment variable or default to 5 hours (18000 seconds)
        let safe_gap_secs = std::env::var("LINK_SAFE_GAP_SECONDS")
            .unwrap_or_else(|_| "18000".to_string())
            .parse::<u64>()
            .unwrap_or(18000);

        let mut needs_update_occurred = false;

        //  // Print out all the values and keys of subscription_file_map
        //  println!("Subscription File Map:");
        //  for entry in subscription_file_map.iter() {
        //      let key = entry.key();
        //      let value = entry.value();
        //      println!("Folder Subscription: {:?}", key);
        //      for (file_path, status) in value.iter() {
        //          println!("  {} - {:?}", file_path, status);
        //      }
        //  }

        // Access the specific subscription's files
        if let Some(files_status) = subscription_file_map.get(&folder_subs_with_path) {
            for (file_path, file_status) in files_status.iter() {
                // we assume that the file status is Sync
                let FileStatus::Sync(current_hash) = file_status;

                let needs_update = match file_links.get(&folder_subs_with_path) {
                    Some(links) => {
                        match links.get(file_path) {
                            Some(link) => {
                                // Check if the link is outdated or the hash has changed
                                link.expiration < SystemTime::now() + Duration::from_secs(safe_gap_secs)
                                    || link.last_8_hash != *current_hash
                            }
                            None => true, // No link exists
                        }
                    }
                    None => true, // No entry exists for this subscription
                };

                if needs_update {
                    needs_update_occurred = true;

                    // Generate a new link
                    let link_result = generate_temporary_shareable_link(file_path, destination, expiration_secs).await;
                    match link_result {
                        Ok(new_link) => {
                            let new_file_link = FileLink {
                                link: new_link,
                                path: file_path.clone(),
                                last_8_hash: current_hash.clone(), // Store the current hash
                                expiration: SystemTime::now() + Duration::from_secs(expiration_secs),
                            };
                            // Update or insert the new link
                            file_links
                                .entry(folder_subs_with_path.clone())
                                .and_modify(|e| {
                                    e.insert(file_path.clone(), new_file_link.clone());
                                })
                                .or_insert_with(|| {
                                    let mut map = HashMap::new();
                                    map.insert(file_path.clone(), new_file_link);
                                    map
                                });

                            // For Debugging
                            println!("File Links After Individual Update:");
                            for entry in file_links.iter() {
                                let folder_subscription = entry.key();
                                let links_map = entry.value();
                                println!("Folder Subscription: {:?}", folder_subscription);
                                for (file_path, link) in links_map.iter() {
                                    println!("  {} - {} - {}", file_path, link.last_8_hash, link.link);
                                }
                            }
                        }
                        Err(e) => {
                            return Err(e.into());
                        }
                    }
                }
            }
        }
        // Store the updated file links to disk if any update occurred
        if needs_update_occurred {
            if let Err(e) = Self::store_file_links_to_disk(db, &folder_subs_with_path, &file_links) {
                shinkai_log(
                    ShinkaiLogOption::ExtSubscriptions,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to store file links to disk: {:?}", e),
                );
            }
        }

        // Update subscription status to Syncing
        subscription_status.insert(folder_subs_with_path.clone(), SubscriptionStatus::Ready);

        Ok(())
    }

    // Helper method to store file links to disk
    fn store_file_links_to_disk(
        db: &Weak<ShinkaiDB>,
        folder_subs_with_path: &FolderSubscriptionWithPath,
        file_links: &Arc<DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>>,
    ) -> Result<(), HttpUploadError> {
        let db_strong = db.upgrade().ok_or_else(|| {
            HttpUploadError::DatabaseError("Failed to upgrade Weak<ShinkaiDB> to a strong reference".to_string())
        })?;

        if let Some(links) = file_links.get(folder_subs_with_path) {
            db_strong.write_file_links(folder_subs_with_path, &links)?;
        }

        Ok(())
    }

    async fn retrieve_base_vr(
        vector_fs: &Weak<VectorFS>,
        file_path: &str,
        streamer: &ShinkaiName,
    ) -> Result<BaseVectorResource, HttpUploadError> {
        let vector_fs_strong = vector_fs.upgrade().ok_or_else(|| {
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Error,
                "VectorFS instance is not available",
            );
            HttpUploadError::VectorFSNotAvailable("VectorFS instance is not available".to_string())
        })?;

        let vr_path = VRPath::from_string(file_path)
            .map_err(|_| HttpUploadError::InvalidRequest("Invalid VRPath".to_string()))?;

        let reader = vector_fs_strong
            .new_reader(streamer.clone(), vr_path.clone(), streamer.clone())
            .await
            .map_err(|_e| HttpUploadError::FileSystemError("Failed to create reader".to_string()))?;

        let resource = vector_fs_strong
            .retrieve_vector_resource(&reader)
            .await
            .map_err(|_| HttpUploadError::FileSystemError("Failed to retrieve vector resource".to_string()))?;

        Ok(resource)
    }

    /// Generates a new filename based on the original filename and a hash.
    /// It appends the last 8 characters of the hash to the filename if the file is a checksum file.
    fn generate_checksum_filename(file_name: &str, hash: &str) -> String {
        if file_name.ends_with(".checksum") {
            return file_name.to_string();
        }

        let hash_part = hash
            .chars()
            .rev()
            .take(8)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();

        format!("{}.{}.checksum", file_name, hash_part)
    }

    pub async fn remove_http_support_for_subscription(
        &self,
        folder_subs_with_path: FolderSubscriptionWithPath,
        profile: &str,
    ) -> Result<(), HttpUploadError> {
        // Remove the subscription status
        self.subscription_status.remove(&folder_subs_with_path);

        // Retrieve the destination from the database
        let db_strong = self.db.upgrade().ok_or_else(|| {
            HttpUploadError::DatabaseError("Failed to upgrade Weak<ShinkaiDB> to a strong reference".to_string())
        })?;

        let credentials = db_strong
            .get_upload_credentials(&folder_subs_with_path.path, profile)
            .map_err(|e| HttpUploadError::DatabaseError(format!("Failed to retrieve upload credentials: {}", e)))?;

        let destination = FileDestination::from_credentials(credentials).await?;

        // Remove the main folder from the server
        let delete_result = delete_file_or_folder(&destination, &folder_subs_with_path.path).await;
        if let Err(e) = delete_result {
            return Err(HttpUploadError::from(e));
        }

        // Remove the subscription from the file_links, subscription_file_map, and subscription_status
        self.file_links.remove(&folder_subs_with_path);
        self.subscription_file_map.remove(&folder_subs_with_path);
        self.subscription_status.remove(&folder_subs_with_path);

        Ok(())
    }

    /// Retrieves cached subscription file links that are in sync.
    pub fn get_cached_subscription_files_links(
        &self,
        folder_subs_with_path: &FolderSubscriptionWithPath,
    ) -> Vec<FileLink> {
        let links = self
            .file_links
            .get(folder_subs_with_path)
            .map(|files| {
                files
                    .iter()
                    .filter(|(_, link)| link.expiration > SystemTime::now()) // Filter links that are still valid
                    .map(|(_, link)| link.clone()) // Collect FileLink objects
                    .collect()
            })
            .unwrap_or_default();

        links
    }

    fn extract_checksum_map(sync_file_paths: &[String]) -> HashMap<String, String> {
        let mut checksum_map = HashMap::new();
        for file in sync_file_paths {
            if let Some(stripped) = file.strip_suffix(".checksum") {
                let parts: Vec<&str> = stripped.rsplitn(2, '.').collect();
                if parts.len() == 2 {
                    let hash_part = parts[0];
                    let base_file = parts[1];
                    if hash_part.len() == 8 {
                        checksum_map.insert(base_file.to_string(), hash_part.to_string());
                    }
                }
            }
        }
        checksum_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_checksum_map() {
        let sync_file_paths = vec![
            "shinkai_sharing/dummy_file1.4aaabb39.checksum".to_string(),
            "shinkai_sharing/dummy_file1".to_string(),
            "shinkai_sharing/dummy_file2.2bbbbb39.checksum".to_string(),
            "shinkai_sharing/dummy_file2".to_string(),
            "shinkai_sharing/shinkai_intro.aaaaaaaa.checksum".to_string(),
            "shinkai_sharing/shinkai_intro".to_string(),
        ];
        let checksum_map = HttpSubscriptionUploadManager::extract_checksum_map(&sync_file_paths);

        let expected_map = [
            ("shinkai_sharing/dummy_file1", "4aaabb39"),
            ("shinkai_sharing/dummy_file2", "2bbbbb39"),
            ("shinkai_sharing/shinkai_intro", "aaaaaaaa"),
        ]
        .iter()
        .cloned()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<HashMap<String, String>>();

        assert_eq!(checksum_map, expected_map);
    }

    #[test]
    fn test_iso8601_serialization() {
        let file_link = FileLink {
            link: "http://example.com".to_string(),
            last_8_hash: "12345678".to_string(),
            expiration: SystemTime::now(),
            path: "shinkai_sharing/dummy_file1".to_string(),
        };

        let serialized = serde_json::to_string(&file_link).unwrap();
        println!("Serialized FileLink: {}", serialized);

        let deserialized: FileLink = serde_json::from_str(&serialized).unwrap();
        assert_eq!(file_link, deserialized);
    }
}
