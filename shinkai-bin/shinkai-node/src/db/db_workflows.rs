use super::{db_errors::ShinkaiDBError, db_main::Topic, ShinkaiDB};
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

impl ShinkaiDB {
    /// Saves a Workflow to the database under the Toolkits topic.
    pub fn save_workflow(&self, workflow: Workflow, profile: ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Generate the key for the workflow using the profile and workflow's generated key
        let key = format!(
            "userworkflows_{}_{}",
            Self::user_profile_to_half_hash(profile),
            workflow.generate_key()
        );

        // Serialize the workflow to bytes
        let workflow_bytes = bincode::serialize(&workflow).expect("Failed to serialize workflow");

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and add the workflow to the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_toolkits, key.as_bytes(), &workflow_bytes);

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Removes a Workflow from the database for the given profile and workflow key.
    pub fn remove_workflow(&self, workflow_key: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Generate the key for the workflow using the profile and workflow key
        let key = format!(
            "userworkflows_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            workflow_key
        );

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and delete the workflow from the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_toolkits, key.as_bytes());

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Lists all Workflows for a specific user profile.
    pub fn list_all_workflows_for_user(&self, profile: &ShinkaiName) -> Result<Vec<Workflow>, ShinkaiDBError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());
        let prefix_search_key = format!("userworkflows_{}_", profile_hash);
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        let mut workflows = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_toolkits, prefix_search_key.as_bytes());

        for item in iterator {
            let (_, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let workflow: Workflow = bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            workflows.push(workflow);
        }

        Ok(workflows)
    }

    /// Gets a specific Workflow for a user profile.
    pub fn get_workflow(&self, workflow_key: &str, profile: &ShinkaiName) -> Result<Workflow, ShinkaiDBError> {
        // Generate the key for the workflow using the profile and workflow key
        let key = format!(
            "userworkflows_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            workflow_key
        );

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Fetch the workflow bytes from the database
        let workflow_bytes = self
            .db
            .get_cf(cf_toolkits, key.as_bytes())?
            .ok_or_else(|| ShinkaiDBError::WorkflowNotFound(format!("Workflow not found for key: {}", workflow_key)))?;

        // Deserialize the workflow from bytes
        let workflow: Workflow = bincode::deserialize(&workflow_bytes)
            .map_err(|_| ShinkaiDBError::DeserializationFailed("Failed to deserialize workflow".to_string()))?;

        Ok(workflow)
    }
}
