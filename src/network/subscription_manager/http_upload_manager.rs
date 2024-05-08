// it should be inside external_subscriber_manager
// it should have a queue to upload files
// it should get notified for every new subscription that needs to handle (share or unshare) maybe that's it from ext_manager

// we should have a struct that encapsulates every file so we know if it's: sync, uploading, waiting, etc
// it should be similar to mirror's logic
// we need to generate a hash of the files and then a tree of the files. can we just use the hash of the vector resources? how can we check it in the other side?
// we upload vrkais so we can manage the files granularly
// we copy the folder structure of the PATH in the storage serve

// In the other end
// the user needs to specify that they want the http files
// the user asks the node for the subscription and current state of the files (it will indicate which ones are ready to be downloaded and which ones are not)
// the user will also need an http_download_manager.rs for this purpose
// should the user actually be in charge of checking diff? or should the node do it?
// it's pull so the user should be in charge of checking the diff
// files are downloading concurrently but also added concurrently to the VR (import of vrkai)

// we need to save the links somewhere. db then?
// delete all the links on unshare

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Weak},
};

use dashmap::DashMap;
use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_subscription::SubscriptionId};
use tokio::sync::Mutex;

use crate::{db::ShinkaiDB, vector_fs::vector_fs::VectorFS};

use super::{external_subscriber_manager::SharedFolderInfo, subscription_file_uploader::{delete_all_in_folder, list_folder_contents, FileDestination, FileTransferError}};

pub enum SubscriptionStatus {
    NotStarted,
    Syncing,
    Ready,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Sync,
    Uploading,
    Waiting,
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

pub struct HttpSubscriptionUploadManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub node_name: ShinkaiName,
    pub is_syncing: bool,
    pub subscription_file_map: DashMap<SubscriptionId, HashMap<String, FileStatus>>,
    pub subscription_status: DashMap<SubscriptionId, SubscriptionStatus>,
    pub subscription_config: DashMap<SubscriptionId, FileDestination>,
    pub upload_queue: Arc<Mutex<VecDeque<FileUpload>>>,
    pub shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>, // (profile, shared_folder)
}

// Q: how do I recover the file destination?
// Q: where the file destination comes from? should it be part of the creation of the subscription so it's saved in the db? I think so

impl HttpSubscriptionUploadManager {
    pub fn new(db: Weak<ShinkaiDB>, vector_fs: Weak<VectorFS>, node_name: ShinkaiName, shared_folders_trees_ref: Arc<DashMap<String, SharedFolderInfo>>) -> Self {
        HttpSubscriptionUploadManager {
            db,
            vector_fs,
            node_name,
            is_syncing: false,
            subscription_file_map: DashMap::new(),
            subscription_status: DashMap::new(),
            subscription_config: DashMap::new(),
            upload_queue: Arc::new(Mutex::new(VecDeque::new())),
            shared_folders_trees_ref,
        }
    }

    // Note: subscription should already have the profile and the shared folder
    pub fn add_subscription(&self, subscription_id: SubscriptionId, file_destination: FileDestination) {
        self.subscription_config
            .insert(subscription_id.clone(), file_destination);
        self.subscription_status
            .insert(subscription_id, SubscriptionStatus::NotStarted);
    }

    pub async fn remove_subscription(&self, subscription_id: SubscriptionId) -> Result<(), HttpUploadError> {
        self.subscription_status.remove(&subscription_id);
        // get the files from the server
        let destination = self
            .subscription_config
            .get(&subscription_id)
            .ok_or(HttpUploadError::SubscriptionNotFound)?;
        let shared_folder = subscription_id.extract_shared_folder()?;
        let file_paths = list_folder_contents(&destination.clone(), shared_folder.as_str()).await?;

        // remove the file from the server
        // the underlying fn needs to be fixed
        delete_all_in_folder(&destination, shared_folder.as_str()).await?;

        for file_path in file_paths {
            // remove the file from the subscription_file_map
            self.subscription_file_map
                .entry(subscription_id.clone())
                .or_default()
                .remove(&file_path.path);
        }
        self.subscription_config.remove(&subscription_id);
        Ok(())
    }

    /// Triggered when files are modified in the shared folder
    pub fn shared_folder_was_updated(&self, shared_folder_updated: String) {
        // TODO: trigger a check of local files and the ones in the target destination
    }

    // fn read_all_files_subscription(&self, subscription_id: SubscriptionId) -> Vec<String> {
    //     let vector_fs = self.vector_fs.upgrade().unwrap();
    //     let files = vector_fs.get_files();
    //     files
    // }

    // pub fn get_subscription_files_links(&self, subscription_id: SubscriptionId) -> Vec<String> {
    //     // TODO: go file by file and generate the links
    //     // make them last for a day
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

    pub fn prepare_file_upload(&self, subscription_id: SubscriptionId, file_path: String) {
        // get the file from the vector fs as vrkai
        // get the file hash
        self.subscription_file_map
            .entry(subscription_id)
            .or_default()
            .insert(file_path, FileStatus::Waiting);

        // add it to the upload queue
    }
}

//

use std::fmt;

#[derive(Debug)]
pub enum HttpUploadError {
    SubscriptionNotFound,
    FileSystemError,
    DatabaseError,
    NetworkError,
}

impl std::error::Error for HttpUploadError {}

impl fmt::Display for HttpUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            HttpUploadError::SubscriptionNotFound => write!(f, "Subscription not found"),
            HttpUploadError::FileSystemError => write!(f, "Error accessing the file system"),
            HttpUploadError::DatabaseError => write!(f, "Database operation failed"),
            HttpUploadError::NetworkError => write!(f, "Network operation failed"),
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