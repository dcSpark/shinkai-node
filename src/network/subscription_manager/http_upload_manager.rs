// // it should be inside external_subscriber_manager
// // it should have a queue to upload files
// // it should get notified for every new subscription that needs to handle (share or unshare) maybe that's it from ext_manager

// // we should have a struct that encapsulates every file so we know if it's: sync, uploading, waiting, etc
// // it should be similar to mirror's logic
// // we need to generate a hash of the files and then a tree of the files. can we just use the hash of the vector resources? how can we check it in the other side?
// // we upload vrkais so we can manage the files granularly
// // we copy the folder structure of the PATH in the storage serve

// // In the other end
// // the user needs to specify that they want the http files
// // the user asks the node for the subscription and current state of the files (it will indicate which ones are ready to be downloaded and which ones are not)
// // the user will also need an http_download_manager.rs for this purpose
// // should the user actually be in charge of checking diff? or should the node do it?
// // it's pull so the user should be in charge of checking the diff
// // files are downloading concurrently but also added concurrently to the VR (import of vrkai)

// // we need to save the links somewhere. db then?
// // delete all the links on unshare

use std::{
    collections::{HashMap, VecDeque},
    env,
    sync::{Arc, Weak},
};

// use blake3::Hasher as Blake3Hasher;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::{
    schemas::{
        shinkai_name::ShinkaiName, shinkai_subscription::SubscriptionId, shinkai_subscription_req::FolderSubscription,
    },
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use shinkai_vector_resources::vector_resource::VRPath;
use std::hash::{Hash, Hasher};
use tokio::sync::Mutex;

use crate::{
    db::ShinkaiDB,
    vector_fs::{vector_fs::VectorFS, vector_fs_permissions::ReadPermission},
};

use super::{
    external_subscriber_manager::SharedFolderInfo,
    fs_entry_tree::FSEntryTree,
    fs_entry_tree_generator::FSEntryTreeGenerator,
    subscription_file_uploader::{
        delete_all_in_folder, list_folder_contents, FileDestination, FileDestinationError, FileTransferError,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionStatus {
    NotStarted,
    Syncing,
    Ready,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Sync(String),
    Uploading(String),
    Waiting(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileUpload {
    file_path: String,
    subscription_id: SubscriptionId,
    action: FileAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSubscriptionWithPath {
    pub path: String,
    pub folder_subscription: FolderSubscription,
}

impl Hash for FolderSubscriptionWithPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Only the path field is used for hashing
        self.path.hash(state);
    }
}

#[allow(dead_code)]
type FileMapPath = String;

#[allow(dead_code)]
const UPLOAD_CONCURRENCY: usize = 2;

pub struct HttpSubscriptionUploadManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub node_name: ShinkaiName,
    pub is_syncing: bool,
    pub subscription_file_map: DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>,
    pub subscription_status: DashMap<FolderSubscriptionWithPath, SubscriptionStatus>,
    pub subscription_config: DashMap<FolderSubscriptionWithPath, FileDestination>,
    pub upload_queue: Arc<Mutex<VecDeque<FileUpload>>>,
    pub shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>, // (streamer_profile:::path, shared_folder)
    pub subscription_processing_task: tokio::task::JoinHandle<()>,
}

impl HttpSubscriptionUploadManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>,
    ) -> Self {
        let subscription_file_map = DashMap::new();
        let subscription_status = DashMap::new();
        let subscription_config = DashMap::new();

        let subscription_http_upload_concurrency = env::var("SUBSCRIPTION_HTTP_UPLOAD_CONCURRENCY")
            .unwrap_or(UPLOAD_CONCURRENCY.to_string())
            .parse::<usize>()
            .unwrap_or(UPLOAD_CONCURRENCY); // Start processing the job queue

        let subscription_processing_task = HttpSubscriptionUploadManager::process_subscription_http_checks(
            db.clone(),
            vector_fs.clone(),
            node_name.clone(),
            subscription_file_map.clone(),
            subscription_status.clone(),
            subscription_config.clone(),
            shared_folders_trees_ref.clone(),
            subscription_http_upload_concurrency,
        )
        .await;

        HttpSubscriptionUploadManager {
            db,
            vector_fs,
            node_name,
            is_syncing: false,
            subscription_file_map,
            subscription_status,
            subscription_config,
            upload_queue: Arc::new(Mutex::new(VecDeque::new())),
            shared_folders_trees_ref,
            subscription_processing_task,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_subscription_http_checks(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        subscription_file_map: DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>,
        subscription_status: DashMap<FolderSubscriptionWithPath, SubscriptionStatus>,
        subscription_config: DashMap<FolderSubscriptionWithPath, FileDestination>,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>, // (streamer_profile:::path, shared_folder)
        subscription_http_upload_concurrency: usize,                      // simultaneous uploads
    ) -> tokio::task::JoinHandle<()> {
        let interval_minutes = env::var("SUBSCRIPTION_HTTP_UPLOAD_INTERVAL_MINUTES")
            .unwrap_or("5".to_string())
            .parse::<u64>()
            .unwrap_or(5);

        let is_testing = env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);
        if is_testing {
            match Self::subscription_http_check_loop(
                db,
                vector_fs,
                node_name,
                subscription_file_map,
                subscription_status,
                subscription_config,
                shared_folders_trees_ref,
                subscription_http_upload_concurrency,
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
            // If we are testing, we don't want to run the subscription processing task
            return tokio::task::spawn(async {});
        }

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(interval_minutes * 60)).await;
            }
        })
    }

    pub async fn subscription_http_check_loop(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        subscription_file_map: DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>,
        subscription_status: DashMap<FolderSubscriptionWithPath, SubscriptionStatus>,
        subscription_config: DashMap<FolderSubscriptionWithPath, FileDestination>,
        shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>, // (streamer_profile:::path, shared_folder)
        subscription_http_upload_concurrency: usize,                      // simultaneous uploads
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
                            &subscription_file_map,
                            &db,
                            &vector_fs,
                            &shared_folders_trees_ref,
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
                .map_err(|_| HttpUploadError::FileSystemError)?;
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

        let paths = vector_fs_strong
            .find_paths_with_read_permissions_as_vec(&reader, vec![ReadPermission::Public])
            .await
            .map_err(|_| HttpUploadError::FileSystemError)?;

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
            .filter_map(|f| futures::executor::block_on(f)) // Execute async block and filter out None results
            .collect();

        Ok(shared_folders)
    }

    #[allow(dead_code)]
    async fn generate_tree_for_shared_folder(
        vector_fs: Weak<VectorFS>,
        full_requester: ShinkaiName,
        path: String,
    ) -> Option<FSEntryTree> {
        FSEntryTreeGenerator::shared_folders_to_tree(vector_fs, full_requester.clone(), full_requester, path)
            .await
            .ok()
    }

    // Helper method to fetch subscriptions that require HTTP support
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

    // Extracted method to process individual folder subscriptions
    #[allow(clippy::too_many_arguments)]
    pub async fn process_single_folder_subscription(
        shared_folder_subs: SharedFolderInfo,
        node_name: ShinkaiName,
        profile: String,
        subscription_file_map: &DashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileStatus>>,
        db: &Weak<ShinkaiDB>,
        vector_fs: &Weak<VectorFS>,
        shared_folders_trees_ref: &Arc<DashMap<String, SharedFolderInfo>>,
        subscription_http_upload_concurrency: usize, // simultaneous uploads
    ) -> Result<(), HttpUploadError> {
        let key = format!("{}:::{}", profile.clone(), shared_folder_subs.path.clone());
        let streamer = ShinkaiName::from_node_and_profile_names(node_name.node_name, profile.clone())?;

        let subscription_expected_files = shared_folders_trees_ref
            .get(&key)
            .map(|shared_folder_info| shared_folder_info.tree.collect_all_paths())
            .unwrap_or_default();

        if subscription_expected_files.is_empty() {
            return Err(HttpUploadError::FileSystemError); // No files found in the shared folder tree
        }

        let folder_subs_with_path = FolderSubscriptionWithPath {
            path: shared_folder_subs.path.clone(),
            folder_subscription: shared_folder_subs.subscription_requirement.clone().ok_or(
                HttpUploadError::InvalidRequest("Missing subscription requirement".to_string()),
            )?,
        };

        let mut subscription_files = subscription_file_map
            .entry(folder_subs_with_path.clone())
            .or_default()
            .clone();

        let mut sync_file_paths: Vec<String> = subscription_files
            .iter()
            .filter_map(|(key, value)| {
                if let FileStatus::Sync(_) = value {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

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

        if sync_file_paths.is_empty() {
            // Only required if subscription_files is empty (we just started). Otherwise use the local cache that should keep a 1 to 1 with the server
            let files = match list_folder_contents(&destination, &shared_folder_subs.path.clone()).await {
                Ok(files) => files
                    .into_iter()
                    .filter(|file| !file.is_folder)
                    .map(|file| file.path)
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

        // TODO: we need to also grab the files' hash. A file can be uploaded but maybe it changed locally so its hash will be different
        // TODO: the files are going to be .vrkai and the checksum are going to be .vrkai.checksum
        // We only stored the last 8 of the hash in the name so it looks like: NAME.vrkai or NAME.LAST_8_OF_HASH.checksum

        // Create a hashmap to map each file to its checksum file if it exists
        let mut checksum_map: HashMap<String, String> = HashMap::new();
        // for file in &sync_file_paths {
        //     if !file.is_folder && file.path.ends_with(".checksum") {
        //         let base_file = file.path.split(".checksum").next().unwrap_or("").to_string();
        //         let hash_part = base_file.split('.').nth_back(1).unwrap_or("");
        //         if hash_part.len() == 8 {
        //             // Using the last 8 characters of the hash
        //             checksum_map.insert(base_file, file.path.clone());
        //         }
        //     }
        // }

        // Check if all files are in sync
        for file in sync_file_paths {
            eprintln!("file: {:?}", file);
            // if file.is_folder || file.path.ends_with(".checksum") {
            //     continue;
            // }

            let file_path = file.clone();
            let vector_fs_strong = match vector_fs.upgrade() {
                Some(fs) => fs,
                None => {
                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Error,
                        "VectorFS instance is not available",
                    );
                    continue; // Skip the current iteration or handle the error as needed
                }
            };
            let vr_path = VRPath::from_string(&file_path).unwrap();
            let reader: crate::vector_fs::vector_fs_reader::VFSReader = vector_fs_strong
                .new_reader(streamer.clone(), vr_path, streamer.clone())
                .await
                .unwrap();
            let resource = vector_fs_strong.retrieve_vector_resource(&reader).await.unwrap();
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
            let checksum_matches = if let Some(checksum_path) = checksum_map.get(&file_path) {
                // Extract the last 8 characters of the hash from the checksum filename
                let expected_hash = checksum_path.split('.').nth_back(1).unwrap_or("").to_string();

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

            // if !checksum_matches {
            //     match file_status {
            //         FileStatus::Sync(_) => {
            //             // File is out of sync due to checksum mismatch
            //             subscription_files.insert(file_path.clone(), FileStatus::Uploading(file_path.clone()));
            //         }
            //         FileStatus::Uploading(_) => {
            //             // File is currently being uploaded
            //             continue;
            //         }
            //         FileStatus::Waiting(_) => {
            //             // File is not in sync, add it to the upload queue
            //             subscription_files.insert(file_path.clone(), FileStatus::Uploading(file_path.clone()));
            //         }
            //     }
            // }
        }

        Ok(())
    }

    // Note: subscription should already have the profile and the shared folder
    // pub async fn add_http_support_to_subscription(
    //     &self,
    //     subscription_id: SubscriptionId,
    // ) -> Result<(), HttpUploadError> {
    //     if let Some(credentials) = subscription_id.http_upload_destination.clone() {
    //         let destination = FileDestination::from_credentials(credentials).await?;
    //         self.subscription_config.insert(subscription_id.clone(), destination);
    //         self.subscription_status
    //             .insert(subscription_id, SubscriptionStatus::NotStarted);
    //         Ok(())
    //     } else {
    //         Err(HttpUploadError::SubscriptionNotFound) // Assuming SubscriptionNotFound is appropriate; adjust as necessary
    //     }
    // }

    // pub async fn remove_http_support_from_subscription(
    //     &self,
    //     subscription_id: SubscriptionId,
    // ) -> Result<(), HttpUploadError> {
    //     self.subscription_status.remove(&subscription_id);
    //     // get the files from the server
    //     let destination = self
    //         .subscription_config
    //         .get(&subscription_id)
    //         .ok_or(HttpUploadError::SubscriptionNotFound)?;
    //     let shared_folder = subscription_id.extract_shared_folder()?;
    //     let file_paths = list_folder_contents(&destination.clone(), shared_folder.as_str()).await?;

    //     // remove the files and folders from the server
    //     delete_all_in_folder(&destination, shared_folder.as_str()).await?;

    //     for file_path in file_paths {
    //         // remove the file from the subscription_file_map
    //         self.subscription_file_map
    //             .entry(subscription_id.clone())
    //             .or_default()
    //             .remove(&file_path.path);
    //     }
    //     self.subscription_config.remove(&subscription_id);
    //     Ok(())
    // }

    /// Triggered when files are modified in the shared folder
    pub fn shared_folder_was_updated(&self, shared_folder_updated: String) {
        // TODO: trigger a check of local files and the ones in the target destination

        // overall strategy
        // do we need to check them both ways? first to make sure that target has all of the local files
        // then a 2nd time: to make sure that target doesn't have extra files
        // O(2n) using a hashmap

        // use minimal to get all the files
        // then do strategy above
    }

    // fn read_all_files_subscription(&self, subscription_id: SubscriptionId) -> Vec<String> {
    //     let vector_fs = self.vector_fs.upgrade().unwrap();
    //     let files = vector_fs.get_files();
    //     files
    // }

    // make them last for a day (we could make this configurable)

    // pub fn get_cached_subscription_files_links(&self, subscription_id: SubscriptionId) -> Vec<String> {
    //     let links = self
    //         .subscription_file_map
    //         .get(&subscription_id)
    //         .map(|files| {
    //             files
    //                 .iter()
    //                 .filter(|(_, status)| matches!(**status, FileStatus::Sync(_))) // Use matches! to check for the Sync variant
    //                 .map(|(file_path, _)| file_path.clone())
    //                 .collect()
    //         })
    //         .unwrap_or_default();

    //     links
    // }

    // // Method to add files to the upload queue
    // pub fn enqueue_file_upload(&self, subscription_id: SubscriptionId, file_path: String) {
    //     let mut queue = self.upload_queue.lock().unwrap();
    //     queue.push_back(FileUpload {
    //         file_path,
    //         status: FileStatus::Waiting,
    //     });
    //     self.subscription_file_map
    //         .entry(subscription_id)
    //         .or_default()
    //         .insert(file_path, false);
    // }

    // // Method to process the file upload queue
    // pub fn process_uploads(&self) {
    //     let queue = self.upload_queue.lock().unwrap();
    //     for file_upload in queue.iter() {
    //         // Implement the logic to handle file upload based on `file_upload.status`
    //         // This is a placeholder for actual upload logic
    //         println!("Uploading: {}", file_upload.file_path);
    //     }
    // }

    pub fn prepare_subscription_upload(&self, subscription_id: SubscriptionId) {
        // check the current status in the destination server
    }

    // pub fn prepare_file_upload(&self, subscription_id: SubscriptionId, file_path: String) {
    //     // get the file from the vector fs as vrkai
    //     // get the file hash
    //     self.subscription_file_map
    //         .entry(subscription_id)
    //         .or_default()
    //         .insert(file_path, FileStatus::Waiting());

    //     // add it to the upload queue
    // }
}

//

use std::fmt;

#[derive(Debug)]
pub enum HttpUploadError {
    SubscriptionNotFound,
    FileSystemError,
    ErrorGettingFolderContents,
    NetworkError,
    SubscriptionDoesntHaveHTTPCreds,
    IOError(std::io::Error),
    VectorFSNotAvailable(String),
    DatabaseError(String),
    InvalidRequest(String),
}

impl std::error::Error for HttpUploadError {}

impl fmt::Display for HttpUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            HttpUploadError::SubscriptionNotFound => write!(f, "Subscription not found"),
            HttpUploadError::FileSystemError => write!(f, "Error accessing the file system"),
            HttpUploadError::ErrorGettingFolderContents => write!(f, "Error getting folder contents"),
            HttpUploadError::NetworkError => write!(f, "Network operation failed"),
            HttpUploadError::SubscriptionDoesntHaveHTTPCreds => write!(f, "Subscription doesn't have HTTP credentials"),
            HttpUploadError::IOError(ref err) => write!(f, "I/O error: {}", err),
            HttpUploadError::VectorFSNotAvailable(ref err) => write!(f, "VectorFS instance is not available: {}", err),
            HttpUploadError::DatabaseError(ref err) => write!(f, "Database error: {}", err),
            HttpUploadError::InvalidRequest(ref err) => write!(f, "Invalid request: {}", err),
        }
    }
}

impl From<&str> for HttpUploadError {
    fn from(err: &str) -> Self {
        HttpUploadError::FileSystemError // Assuming FileSystemError is appropriate; adjust as necessary
    }
}

impl From<FileTransferError> for HttpUploadError {
    fn from(err: FileTransferError) -> Self {
        match err {
            FileTransferError::NetworkError(_) => HttpUploadError::NetworkError,
            FileTransferError::InvalidHeaderValue => HttpUploadError::NetworkError,
            FileTransferError::Other(_) => HttpUploadError::FileSystemError, // Map to FileSystemError or another appropriate error
        }
    }
}

impl From<FileDestinationError> for HttpUploadError {
    fn from(err: FileDestinationError) -> Self {
        match err {
            FileDestinationError::JsonError(_) => HttpUploadError::FileSystemError, // JSON errors might be considered as file system errors if they relate to file handling.
            FileDestinationError::InvalidInput(_) => HttpUploadError::FileSystemError, // Invalid input might be due to incorrect file data.
            FileDestinationError::UnknownTypeField => HttpUploadError::FileSystemError, // Unknown type field might be due to incorrect configuration or data.
        }
    }
}

impl From<std::io::Error> for HttpUploadError {
    fn from(err: std::io::Error) -> Self {
        HttpUploadError::IOError(err)
    }
}