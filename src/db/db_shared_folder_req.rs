use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use shinkai_message_primitives::schemas::shinkai_subscription_req::FolderSubscription;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::FileDestinationCredentials;

impl ShinkaiDB {
    // TODO: extend to take a profile as well
    pub fn set_folder_requirements(
        &self,
        path: &str,
        subscription_requirement: FolderSubscription,
    ) -> Result<(), ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // 47 characters are required so prefix search works
        let prefix = format!("folder_subscriptions_requirements_abcde_prefix_{}", path);

        // Convert the context to JSON bytes
        let req_bytes = serde_json::to_vec(&subscription_requirement).map_err(|e| {
            eprintln!("Serialization error: {:?}", e);
            ShinkaiDBError::SomeError("Failed converting subscription requirements to JSON bytes".to_string())
        })?;

        self.db.put_cf(cf_node, prefix.as_bytes(), req_bytes)?;

        Ok(())
    }

    // TODO: extend to take a profile as well
    pub fn get_folder_requirements(&self, path: &str) -> Result<FolderSubscription, ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // 47 characters are required so prefix search works
        let prefix = format!("folder_subscriptions_requirements_abcde_prefix_{}", path);

        // Retrieve the bytes from the database
        let req_bytes = self
            .db
            .get_cf(cf_node, prefix.as_bytes())
            .map_err(|_| {
                ShinkaiDBError::SomeError("Failed to retrieve subscription requirements from the database".to_string())
            })?
            .ok_or(ShinkaiDBError::SomeError(
                "No subscription requirements found for the given path".to_string(),
            ))?;

        // Convert the bytes back to FolderSubscription
        let subscription_requirement: FolderSubscription = serde_json::from_slice(&req_bytes).map_err(|e| {
            eprintln!("Deserialization error: {:?}", e);
            ShinkaiDBError::SomeError(format!(
                "Failed converting JSON bytes back to subscription requirements: {:?}",
                e
            ))
        })?;

        Ok(subscription_requirement)
    }

    // TODO: extend to take a profile as well
    pub fn remove_folder_requirements(&self, path: &str) -> Result<(), ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // 47 characters are required so prefix search works
        let prefix = format!("folder_subscriptions_requirements_abcde_prefix_{}", path);

        // Remove the subscription requirements from the database
        self.db.delete_cf(cf_node, prefix.as_bytes()).map_err(|_| {
            ShinkaiDBError::SomeError("Failed to remove subscription requirements from the database".to_string())
        })?;

        Ok(())
    }

    pub fn get_all_folder_requirements(&self) -> Result<Vec<(String, FolderSubscription)>, ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
    
        // Prefix for searching all folder subscriptions
        let prefix = "folder_subscriptions_requirements_abcde_prefix_".as_bytes();
    
        // Prepare an iterator for prefix search
        let iter = self.db.prefix_iterator_cf(cf_node, prefix);
    
        let mut results = Vec::new();
    
        // Iterate over all keys that match the prefix
        for item in iter {
            // Handle the Result type
            let (key, value) = match item {
                Ok((key, value)) => (key, value),
                Err(_) => return Err(ShinkaiDBError::SomeError("Iterator error".to_string())),
            };
    
            let path = String::from_utf8(key[prefix.len()..].to_vec())
                .map_err(|_| ShinkaiDBError::SomeError("Failed to convert bytes to string for path".to_string()))?;
    
            let subscription_requirement: FolderSubscription = serde_json::from_slice(&value).map_err(|e| {
                ShinkaiDBError::SomeError(format!("Failed converting JSON bytes back to subscription requirements: {:?}", e))
            })?;
    
            results.push((path, subscription_requirement));
        }
    
        Ok(results)
    }

    pub fn set_upload_credentials(
        &self,
        path: &str,
        profile: &str,
        credentials: FileDestinationCredentials,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("folder_subscriptions_upload_credentials_prefix_{}_{}", path, profile);
        let cred_bytes = serde_json::to_vec(&credentials).map_err(|e| {
            ShinkaiDBError::SomeError(format!("Failed to serialize upload credentials: {:?}", e))
        })?;
        self.db.put_cf(cf_node, prefix.as_bytes(), cred_bytes)?;
        Ok(())
    }

    pub fn get_upload_credentials(
        &self,
        path: &str,
        profile: &str,
    ) -> Result<FileDestinationCredentials, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("folder_subscriptions_upload_credentials_prefix_{}_{}", path, profile);
        let cred_bytes = self
            .db
            .get_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to retrieve upload credentials".to_string()))?
            .ok_or(ShinkaiDBError::SomeError("No upload credentials found".to_string()))?;
        let credentials: FileDestinationCredentials = serde_json::from_slice(&cred_bytes).map_err(|e| {
            ShinkaiDBError::SomeError(format!("Failed to deserialize upload credentials: {:?}", e))
        })?;
        Ok(credentials)
    }

    pub fn remove_upload_credentials(&self, path: &str, profile: &str) -> Result<(), ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix = format!("folder_subscriptions_upload_credentials_prefix_{}_{}", path, profile);
        self.db
            .delete_cf(cf_node, prefix.as_bytes())
            .map_err(|_| ShinkaiDBError::SomeError("Failed to remove upload credentials".to_string()))?;
        Ok(())
    }
}
