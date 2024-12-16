use super::vector_fs_internals::VectorFSInternals;

use crate::welcome_files::shinkai_faq::SHINKAI_FAQ_VRKAI;
use crate::welcome_files::shinkai_whitepaper::SHINKAI_WHITEPAPER_VRKAI;

use super::vector_fs_error::VectorFSError;
use super::vector_fs_reader::VFSReader;
use super::vector_fs_writer::VFSWriter;
use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sqlite::SqliteManager;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
EmbeddingModelType;
use shinkai_vector_resources::vector_resource::{VRKai, VRPath, VectorResourceCore, VectorResourceSearch};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Struct that wraps all functionality of the VectorFS.
/// Of note, internals_map holds a hashmap of the VectorFSInternals
/// for all profiles on the node.
#[derive(Debug)]
pub struct VectorFS {
    pub node_name: ShinkaiName,
    pub internals_map: RwLock<HashMap<ShinkaiName, VectorFSInternals>>,
    pub db: Arc<RwLock<SqliteManager>>,
    /// Intended to be used only for generating query embeddings for Vector Search
    /// Processing content into Vector Resources should always be done outside of the VectorFS
    /// to prevent locking for long periods of time. (If VR with unsupported model is tried to be added to FS, should error, and regeneration happens externally)
    pub embedding_generator: RemoteEmbeddingGenerator,
}

impl VectorFS {
    /// Initializes the VectorFS struct. If no existing VectorFS exists in the VectorFSDB, then initializes from scratch.
    /// Otherwise reads from the FSDB. Requires supplying list of profiles setup in the node.
    /// Auto-initializes new profiles, setting their default embedding model to be based on the supplied embedding_generator.
    pub async fn new(
        embedding_generator: RemoteEmbeddingGenerator,
        supported_embedding_models: Vec<EmbeddingModelType>,
        profile_list: Vec<ShinkaiName>,
        db: Arc<RwLock<SqliteManager>>,
        node_name: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        // Read each existing profile's fs internals from fsdb
        let mut internals_map = HashMap::new();
        for profile in &profile_list {
            match db.read().await.get_profile_fs_internals(profile) {
                Ok(internals) => {
                    let internals = VectorFSInternals {
                        fs_core_resource: internals.0,
                        permissions_index: serde_json::from_slice(&internals.1)
                            .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
                        subscription_index: serde_json::from_slice(&internals.2)
                            .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
                        supported_embedding_models: internals.3,
                        last_read_index: serde_json::from_slice(&internals.4)
                            .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
                    };
                    internals_map.insert(profile.clone(), internals);
                }
                _ => continue,
            }
        }

        let internals_map = RwLock::new(internals_map);

        // Initialize the VectorFS
        let default_embedding_model = embedding_generator.model_type().clone();
        let vector_fs = Self {
            internals_map,
            db,
            embedding_generator,
            node_name: node_name.clone(),
        };

        // Initialize any new profiles which don't already exist in the VectorFS
        vector_fs
            .initialize_new_profiles(
                &node_name,
                profile_list,
                default_embedding_model,
                supported_embedding_models,
                false,
            )
            .await?;

        Ok(vector_fs)
    }

    /// Creates a new VFSReader if the `requester_name` passes read permission validation check.
    /// VFSReader can then be used to perform read actions at the specified path.
    pub async fn new_reader(
        &self,
        requester_name: ShinkaiName,
        path: VRPath,
        profile: ShinkaiName,
    ) -> Result<VFSReader, VectorFSError> {
        VFSReader::new(requester_name, path, self, profile).await
    }

    /// Creates a new VFSWriter if the `requester_name` passes write permission validation check.
    /// VFSWriter can then be used to perform write actions at the specified path.
    pub async fn new_writer(
        &self,
        requester_name: ShinkaiName,
        path: VRPath,
        profile: ShinkaiName,
    ) -> Result<VFSWriter, VectorFSError> {
        VFSWriter::new(requester_name, path, self, profile).await
    }

    /// Initializes a new profile and inserts it into the internals_map
    pub async fn initialize_profile(
        &self,
        requester_name: &ShinkaiName,
        profile: ShinkaiName,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        self._validate_node_action_permission(requester_name, &format!("Failed initializing profile {}.", profile))?;

        if let Err(_) = self.get_profile_fs_internals(&profile).await {
            // Extract just the node name from the profile name
            let fs_internals =
                VectorFSInternals::new(profile.clone(), default_embedding_model, supported_embedding_models).await;

            self.save_profile_fs_internals(fs_internals, &profile).await?;
        }

        let internals = self.get_profile_fs_internals(&profile).await?;

        // Acquire a write lock to modify internals_map
        let mut internals_map = self.internals_map.write().await;
        internals_map.insert(profile, internals);
        Ok(())
    }

    /// Checks the input profile_list and initializes a new profile for any which are not already set up in the VectorFS.
    pub async fn initialize_new_profiles(
        &self,
        requester_name: &ShinkaiName,
        profile_list: Vec<ShinkaiName>,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
        create_default_folders: bool,
    ) -> Result<(), VectorFSError> {
        // Acquire a read lock for checking existing profiles
        let mut internals_map_read = self.internals_map.read().await;

        for profile in profile_list {
            if !internals_map_read.contains_key(&profile) {
                // Drop the read lock before awaiting on the async initialize_profile
                drop(internals_map_read);

                // Since initialize_profile is async, await on it
                self.initialize_profile(
                    requester_name,
                    profile.clone(), // Assuming clone is cheap for ShinkaiName
                    default_embedding_model.clone(),
                    supported_embedding_models.clone(),
                )
                .await?;

                // Creates default folders and files if create_default_folders is true
                if create_default_folders {
                    let writer = self
                        .new_writer(profile.clone(), VRPath::root(), profile.clone())
                        .await?;
                    self.create_new_folder(&writer, "My Files (Private)").await?;
                    self.create_new_folder(&writer, "My Subscriptions").await?;
                    self.create_new_folder(&writer, "For Sharing").await?;

                    let my_files = VRPath::from_string("/My Files (Private)").unwrap();
                    let writer = self
                        .new_writer(profile.clone(), my_files.clone(), profile.clone())
                        .await?;
                    self.create_new_folder(&writer, "Shinkai").await?;

                    // Create a default file in the "My Files (Private)" folder
                    let shinkai_folder = my_files.push_cloned("Shinkai".to_string());
                    let writer = self
                        .new_writer(profile.clone(), shinkai_folder, profile.clone())
                        .await?;
                    let shinkai_faq = VRKai::from_base64(SHINKAI_FAQ_VRKAI).unwrap();
                    let shinkai_whitepaper = VRKai::from_base64(SHINKAI_WHITEPAPER_VRKAI).unwrap();
                    let _save_result = self.save_vrkai_in_folder(&writer, shinkai_faq).await;
                    let _save_result = self.save_vrkai_in_folder(&writer, shinkai_whitepaper).await;
                }

                // Re-acquire the read lock for the next iteration
                internals_map_read = self.internals_map.read().await;
            }
        }
        Ok(())
    }

    /// Reverts the internals of a profile to the last saved state in the database.
    pub async fn revert_internals_to_last_db_save(
        &self,
        requester_name: &ShinkaiName,
        profile: &ShinkaiName,
    ) -> Result<(), VectorFSError> {
        // Validate the requester's permission to perform this action
        self._validate_profile_action_permission(
            requester_name,
            profile,
            &format!("Failed reverting fs internals to last DB save for profile: {}", profile),
        )
        .await?;

        // Fetch the last saved state of the profile fs internals from the database
        let internals = self.get_profile_fs_internals(profile).await?;

        // Acquire a write lock asynchronously to modify internals_map
        let mut internals_map = self.internals_map.write().await;

        // Overwrite the current state of the profile internals in the map with the fetched state
        internals_map.insert(profile.clone(), internals);

        Ok(())
    }

    /// Sets the supported embedding models for a specific profile
    pub async fn set_profile_supported_models(
        &self,
        requester_name: &ShinkaiName,
        profile: &ShinkaiName,
        supported_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        self._validate_node_action_permission(requester_name, "Failed setting all profile supported models.")?;

        // Acquire a write lock asynchronously to modify internals_map
        let mut internals_map = self.internals_map.write().await;

        if let Some(fs_internals) = internals_map.get_mut(profile) {
            fs_internals.supported_embedding_models = supported_models;
            // Assuming save_profile_fs_internals is async, you need to await it
            self.save_profile_fs_internals(fs_internals.clone(), profile).await?;
        }
        Ok(())
    }

    /// Get a prepared Embedding Generator that is setup with the correct default EmbeddingModelType
    /// for the profile's VectorFS.
    pub async fn _get_embedding_generator(
        &self,
        profile: &ShinkaiName,
    ) -> Result<RemoteEmbeddingGenerator, VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(profile).await?;
        let generator = internals.fs_core_resource.initialize_compatible_embeddings_generator(
            &self.embedding_generator.api_url,
            self.embedding_generator.api_key.clone(),
        );
        Ok(generator)
    }

    /// Validates the permission for a node action for a given requester ShinkaiName. Internal method.
    /// In case of error, includes requester_name automatically together with your error message
    pub fn _validate_node_action_permission(
        &self,
        requester_name: &ShinkaiName,
        error_message: &str,
    ) -> Result<(), VectorFSError> {
        if self.node_name.node_name == requester_name.node_name {
            return Ok(());
        }
        Err(VectorFSError::InvalidNodeActionPermission(
            requester_name.clone(),
            error_message.to_string(),
        ))
    }

    /// Validates the permission for a profile action for a given requester ShinkaiName. Internal method.
    /// In case of error, includes requester_name automatically together with your error message
    pub async fn _validate_profile_action_permission(
        &self,
        requester_name: &ShinkaiName,
        profile: &ShinkaiName,
        error_message: &str,
    ) -> Result<(), VectorFSError> {
        if let Ok(_) = self.get_profile_fs_internals_cloned(profile).await {
            if profile.profile_name == requester_name.profile_name {
                return Ok(());
            }
        }
        Err(VectorFSError::InvalidProfileActionPermission(
            requester_name.clone(),
            error_message.to_string(),
        ))
    }

    /// Attempts to fetch a copy of the profile VectorFSInternals (from memory)
    /// in the internals_map. ANY MUTATION DOESN'T PROPAGATE.
    pub async fn get_profile_fs_internals_cloned(
        &self,
        profile: &ShinkaiName,
    ) -> Result<VectorFSInternals, VectorFSError> {
        let internals_map = self.internals_map.read().await;
        let internals = internals_map
            .get(profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(profile.to_string()))?
            .clone();

        Ok(internals)
    }

    /// Updates the fs_internals for a specific profile. Applies only in memory.
    /// This function should be used with caution as it directly modifies the internals.
    pub async fn _update_fs_internals(
        &self,
        profile: ShinkaiName,
        new_internals: VectorFSInternals,
    ) -> Result<(), VectorFSError> {
        // Acquire a write lock to modify internals_map
        let mut internals_map = self.internals_map.write().await;

        // Update the internals for the specified profile
        internals_map.insert(profile, new_internals);

        Ok(())
    }

    /// Updates the last read path and time for a given profile.
    pub async fn update_last_read_path(
        &self,
        profile: &ShinkaiName,
        path: VRPath,
        current_datetime: DateTime<Utc>,
        requester_name: ShinkaiName,
    ) -> Result<(), VectorFSError> {
        let mut internals_map = self.internals_map.write().await;
        let internals = internals_map
            .get_mut(profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(profile.to_string()))?;

        internals
            .last_read_index
            .update_path_last_read(path, current_datetime, requester_name);
        Ok(())
    }

    /// Prints the internal nodes (of the core VR) of a Profile's VectorFS
    pub async fn print_profile_vector_fs_resource(&self, profile: ShinkaiName) {
        let internals = self.get_profile_fs_internals_cloned(&profile).await.unwrap();
        println!(
            "\n\n{}'s VectorFS Internal Resource Representation\n------------------------------------------------",
            profile.clone()
        );
        internals.fs_core_resource.print_all_nodes_exhaustive(None, true, false);
    }

    pub async fn save_profile_fs_internals(
        &self,
        fs_internals: VectorFSInternals,
        profile: &ShinkaiName,
    ) -> Result<(), VectorFSError> {
        self.db
            .write()
            .await
            .save_profile_fs_internals(
                profile,
                fs_internals.fs_core_resource,
                serde_json::to_vec(&fs_internals.permissions_index)
                    .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
                serde_json::to_vec(&fs_internals.subscription_index)
                    .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
                fs_internals.supported_embedding_models,
                serde_json::to_vec(&fs_internals.last_read_index)
                    .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
            )
            .map_err(|e| VectorFSError::SqliteManagerError(e))
    }

    pub async fn get_profile_fs_internals(&self, profile: &ShinkaiName) -> Result<VectorFSInternals, VectorFSError> {
        let (core_resource, permissions_index, subscription_index, supported_embedding_models, last_read_index) =
            self.db.read().await.get_profile_fs_internals(profile)?;

        Ok(VectorFSInternals {
            fs_core_resource: core_resource,
            permissions_index: serde_json::from_slice(&permissions_index)
                .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
            subscription_index: serde_json::from_slice(&subscription_index)
                .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
            supported_embedding_models,
            last_read_index: serde_json::from_slice(&last_read_index)
                .map_err(|e| VectorFSError::DataConversionError(e.to_string()))?,
        })
    }
}
