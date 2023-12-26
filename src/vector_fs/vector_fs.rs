use super::fs_internals::VectorFSInternals;
use super::vector_fs_reader::VFSReader;
use super::{db::fs_db::VectorFSDB, fs_error::VectorFSError};
use rocksdb::Error;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::vector_resource::VectorResource;
use shinkai_vector_resources::vector_search_traversal::VRPath;
use shinkai_vector_resources::{
    embeddings::Embedding, map_resource::MapVectorResource, model_type::EmbeddingModelType, source::VRSource,
};
use std::collections::HashMap;

/// Struct that wraps all functionality of the VectorFS.
/// Of note, internals_map holds a hashmap of the VectorFSInternals
/// for all profiles on the node.
pub struct VectorFS {
    node_name: ShinkaiName,
    internals_map: HashMap<ShinkaiName, VectorFSInternals>,
    db: VectorFSDB,
    /// Intended to be used only for generating query embeddings for Vector Search
    /// Processing content into Vector Resources should always be done outside of the VectorFS
    /// to prevent locking for long periods of time. (If VR with unsupported model is tried to be added to FS, should error, and regeneration happens externally)
    embedding_generator: RemoteEmbeddingGenerator,
}

impl VectorFS {
    /// Initializes the VectorFS struct. If no existing VectorFS exists in the VectorFSDB, then initializes from scratch.
    /// Otherwise reads from the FSDB. Requires supplying list of profiles setup in the node.
    /// Auto-initializes new profiles, setting their default embedding model to be based on the supplied embedding_generator.
    pub fn new(
        embedding_generator: RemoteEmbeddingGenerator,
        supported_embedding_models: Vec<EmbeddingModelType>,
        profile_list: Vec<ShinkaiName>,
        db_path: &str,
        node_name: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        let fs_db = VectorFSDB::new(db_path)?;

        // Read each existing profile's fs internals from fsdb
        let mut internals_map = HashMap::new();
        for profile in &profile_list {
            match fs_db.get_profile_fs_internals(profile) {
                Ok(internals) => {
                    internals_map.insert(profile.clone(), internals);
                }
                _ => continue,
            }
        }

        // Initialize the VectorFS
        let default_embedding_model = embedding_generator.model_type().clone();
        let mut vector_fs = Self {
            internals_map,
            db: fs_db,
            embedding_generator,
            node_name: node_name.clone(),
        };

        // Initialize any new profiles which don't already exist in the VectorFS
        vector_fs.initialize_new_profiles(
            &node_name,
            profile_list,
            default_embedding_model,
            supported_embedding_models,
        )?;

        Ok(vector_fs)
    }

    /// IMPORTANT: Only to be used when writing tests that do not use the VectorFS.
    /// Simply creates a barebones struct to be used to satisfy required types.
    pub fn new_empty() -> Self {
        Self {
            internals_map: HashMap::new(),
            db: VectorFSDB::new_empty(),
            embedding_generator: RemoteEmbeddingGenerator::new_default(),
            node_name: ShinkaiName::from_node_name("@@node1.shinkai".to_string()).unwrap(),
        }
    }

    /// Creates a new VFSReader if the `requester_name` passes read permission validation check.
    /// VFSReader can then be used to perform read actions at the specified path.
    pub fn reader(&self, requester_name: ShinkaiName, path: VRPath) -> Result<VFSReader, VectorFSError> {
        VFSReader::new(requester_name, path, self)
    }

    /// Initializes a new profile and inserts it into the internals_map
    pub fn initialize_profile(
        &mut self,
        requester_name: &ShinkaiName,
        profile: ShinkaiName,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        self.db
            .init_profile_fs_internals(&profile, default_embedding_model.clone(), supported_embedding_models)?;
        let internals = self.db.get_profile_fs_internals(&profile)?;
        self.internals_map.insert(profile, internals);
        Ok(())
    }

    /// Checks the input profile_list and initializes profiles for any which are not already initialized.
    pub fn initialize_new_profiles(
        &mut self,
        requester_name: &ShinkaiName,
        profile_list: Vec<ShinkaiName>,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        for profile in profile_list {
            if !self.internals_map.contains_key(&profile) {
                self.initialize_profile(
                    requester_name,
                    profile,
                    default_embedding_model.clone(),
                    supported_embedding_models.clone(),
                )?;
            }
        }
        Ok(())
    }

    /// Sets the supported models for a profile
    pub fn set_profile_supported_models(
        &mut self,
        requester_name: &ShinkaiName,
        profile: &ShinkaiName,
        supported_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        self.validate_node_action_permission(requester_name, "Failed setting all profile supported models.")?;
        if let Some(fs_internals) = self.internals_map.get_mut(profile) {
            fs_internals.supported_embedding_models = supported_models;
            self.db.save_profile_fs_internals(fs_internals, profile)?;
        }
        Ok(())
    }

    /// Sets the supported models for all profiles
    pub fn set_all_profiles_supported_models(
        &mut self,
        requester_name: &ShinkaiName,
        supported_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        self.validate_node_action_permission(requester_name, "Failed setting all profile supported models.")?;
        for profile in self.internals_map.keys().cloned().collect::<Vec<ShinkaiName>>() {
            self.set_profile_supported_models(requester_name, &profile, supported_models.clone())?;
        }
        Ok(())
    }

    /// Validates the permission for a node action for a given requester ShinkaiName.
    /// In case of error, includes requester_name automatically together with your error message
    pub fn validate_node_action_permission(
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

    /// Validates the permission for a profile action for a given requester ShinkaiName.
    /// In case of error, includes requester_name automatically together with your error message
    pub fn validate_profile_action_permission(
        &self,
        requester_name: &ShinkaiName,
        profile: &ShinkaiName,
        error_message: &str,
    ) -> Result<(), VectorFSError> {
        if let Ok(_) = self.get_profile_fs_internals_read_only(profile) {
            if profile.profile_name == requester_name.profile_name {
                return Ok(());
            }
        }
        Err(VectorFSError::InvalidProfileActionPermission(
            requester_name.clone(),
            error_message.to_string(),
        ))
    }

    /// Generates an Embedding for the input query to be used in a Vector Search in the VecFS.
    /// This automatically uses the correct default embedding model for the given profile.
    pub async fn generate_query_embedding(
        &self,
        input_query: String,
        profile: &ShinkaiName,
    ) -> Result<Embedding, VectorFSError> {
        let generator = self.get_embedding_generator(profile)?;
        Ok(generator.generate_embedding_default(&input_query).await?)
    }

    /// Get a prepared Embedding Generator that is setup with the correct default EmbeddingModelType
    /// for the profile's VectorFS.
    pub fn get_embedding_generator(&self, profile: &ShinkaiName) -> Result<RemoteEmbeddingGenerator, VectorFSError> {
        let internals = self.get_profile_fs_internals_read_only(profile)?;
        let generator = internals.fs_core_resource.initialize_compatible_embeddings_generator(
            &self.embedding_generator.api_url,
            self.embedding_generator.api_key.clone(),
        );
        return Ok(generator);
    }

    /// Attempts to fetch a mutable reference to the profile VectorFSInternals (from memory)
    /// in the internals_map.
    pub fn get_profile_fs_internals(&mut self, profile: &ShinkaiName) -> Result<&mut VectorFSInternals, VectorFSError> {
        self.internals_map
            .get_mut(profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(profile.to_string()))
    }

    /// Attempts to fetch an immutable reference to the profile VectorFSInternals (from memory)
    /// in the internals_map. Used for pure reads where no updates are needed.
    pub fn get_profile_fs_internals_read_only(
        &self,
        profile: &ShinkaiName,
    ) -> Result<&VectorFSInternals, VectorFSError> {
        self.internals_map
            .get(profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(profile.to_string()))
    }
}
