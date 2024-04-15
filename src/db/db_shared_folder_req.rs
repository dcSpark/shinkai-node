use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use shinkai_message_primitives::schemas::shinkai_subscription_req::FolderSubscription;

impl ShinkaiDB {
    pub fn set_folder_requirements(
        &self,
        path: &str,
        subscription_requirement: FolderSubscription,
    ) -> Result<(), ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // 47 characters are required so prefix search works
        let prefix = format!("folder_subscriptions_requirements_abcde_prefix_{}", path);

        // Convert the context to bytes
        let req_bytes = bincode::serialize(&subscription_requirement).map_err(|_| {
            ShinkaiDBError::SomeError("Failed converting subscription requirements to bytes".to_string())
        })?;

        self.db.put_cf(cf_node, prefix.as_bytes(), req_bytes)?;

        Ok(())
    }

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

        // Convert the bytes back to ShinkaiSubscriptionReq
        let subscription_requirement = bincode::deserialize(&req_bytes).map_err(|_| {
            ShinkaiDBError::SomeError("Failed converting bytes back to subscription requirements".to_string())
        })?;

        Ok(subscription_requirement)
    }

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

            let subscription_requirement = bincode::deserialize(&value).map_err(|_| {
                ShinkaiDBError::SomeError("Failed converting bytes back to subscription requirements".to_string())
            })?;

            results.push((path, subscription_requirement));
        }

        Ok(results)
    }
}
