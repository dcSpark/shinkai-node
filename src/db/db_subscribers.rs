use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use shinkai_message_primitives::schemas::shinkai_subscription::{ShinkaiSubscription, SubscriptionId};

impl ShinkaiDB {
    /// Returns the first half of the blake3 hash of the folder name value
    pub fn folder_name_to_hash(folder_name: String) -> String {
        let full_hash = blake3::hash(folder_name.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    /// Adds a subscriber to a shared folder.
    pub fn add_subscriber_subscription(&self, subscription: ShinkaiSubscription) -> Result<(), ShinkaiDBError> {
        let sub_node_name_str = subscription.subscriber_node.get_node_name_string();
        let sub_profile_name_str = subscription.subscriber_profile.clone();
        let shared_folder = subscription.shared_folder.clone();

        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // 47 characters are required so prefix search works
        let prefix_all = format!(
            "user_shared_folders_subscriptions_abcde_prefix_{}",
            subscription.subscription_id.get_unique_id()
        );

        let prefix_folder = Self::generate_prefix_folder(&shared_folder, &sub_node_name_str, &sub_profile_name_str);

        let subscription_bytes = bincode::serialize(&subscription).expect("Failed to serialize payment");

        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_node, prefix_all.as_bytes(), &subscription_bytes);
        batch.put_cf(cf_node, prefix_folder.as_bytes(), &subscription_bytes);
        self.db.write(batch)?;

        Ok(())
    }

    /// Updates a subscriber's subscription.
    #[allow(dead_code)]
    pub fn update_subscriber_subscription(&self, subscription: ShinkaiSubscription) -> Result<(), ShinkaiDBError> {
        let sub_node_name_str = subscription.subscriber_node.get_node_name_string();
        let sub_profile_name_str = subscription.subscriber_profile.clone();
        let shared_folder = subscription.shared_folder.clone();

        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // Construct the keys similar to add_subscriber but for updating
        let prefix_all = format!(
            "user_shared_folders_subscriptions_abcde_prefix_{}",
            subscription.subscription_id.get_unique_id()
        );

        let prefix_folder = Self::generate_prefix_folder(&shared_folder, &sub_node_name_str, &sub_profile_name_str);

        let subscription_bytes = bincode::serialize(&subscription).expect("Failed to serialize subscription");

        // Instead of creating a new entry, this will overwrite the existing one if it exists
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_node, prefix_all.as_bytes(), &subscription_bytes);
        batch.put_cf(cf_node, prefix_folder.as_bytes(), &subscription_bytes);
        self.db.write(batch)?;

        Ok(())
    }

    /// Retrieves all subscribers for a given shared folder, including their subscription details.
    pub fn all_subscribers_for_folder(&self, shared_folder: &str) -> Result<Vec<ShinkaiSubscription>, ShinkaiDBError> {
        let folder_hash = Self::folder_name_to_hash(shared_folder.to_string());
        let prefix_search_key = format!("subscriptions_{}_", folder_hash);
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        let mut subscriptions = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_node, prefix_search_key.as_bytes());

        for item in iterator {
            let (_, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let subscription: ShinkaiSubscription =
                bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            subscriptions.push(subscription);
        }

        Ok(subscriptions)
    }

    /// Retrieves a subscription by its SubscriptionId.
    pub fn get_subscription_by_id(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<ShinkaiSubscription, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix_search_key = format!(
            "user_shared_folders_subscriptions_abcde_prefix_{}",
            subscription_id.get_unique_id()
        );

        let value = self
            .db
            .get_cf(cf_node, prefix_search_key.as_bytes())
            .map_err(ShinkaiDBError::RocksDBError)?;

        match value {
            Some(subscription_bytes) => {
                let subscription: ShinkaiSubscription =
                    bincode::deserialize(&subscription_bytes).map_err(ShinkaiDBError::BincodeError)?;
                Ok(subscription)
            }
            None => Err(ShinkaiDBError::DataNotFound),
        }
    }

    /// Removes a subscriber from a shared folder.
    pub fn remove_subscriber(&self, subscription_id: &SubscriptionId) -> Result<(), ShinkaiDBError> {
        let shared_folder = subscription_id
            .extract_shared_folder()
            .map_err(|_| ShinkaiDBError::InvalidData)?;
        let sub_node_name_str = subscription_id
            .extract_subscriber_node()
            .map_err(|_| ShinkaiDBError::InvalidData)?
            .get_node_name_string();
        let sub_profile_name_str = subscription_id
            .extract_subscriber_profile()
            .map_err(|_| ShinkaiDBError::InvalidData)?;

        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // Construct the keys for deletion based on the same format used in add_subscriber
        let prefix_all = format!(
            "user_shared_folders_subscriptions_abcde_prefix_{}",
            subscription_id.get_unique_id()
        );

        let prefix_folder = Self::generate_prefix_folder(&shared_folder, &sub_node_name_str, &sub_profile_name_str);

        // Perform the deletion from the database
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_node, prefix_all.as_bytes());
        batch.delete_cf(cf_node, prefix_folder.as_bytes());
        self.db.write(batch)?;

        Ok(())
    }

    /// Retrieves all subscribers along with their subscription details.
    pub fn all_subscribers_subscription(&self) -> Result<Vec<ShinkaiSubscription>, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix_search_key = "user_shared_folders_subscriptions_abcde_prefix_".as_bytes();

        let mut subscriptions = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_node, prefix_search_key);

        for item in iterator {
            let (_, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let subscription: ShinkaiSubscription =
                bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            subscriptions.push(subscription);
        }

        Ok(subscriptions)
    }

    /// Generates a prefix folder string.
    fn generate_prefix_folder(shared_folder: &str, sub_node_name_str: &str, sub_node_profile_str: &str) -> String {
        format!(
            "subscriptions_{}_{}_{}",
            Self::folder_name_to_hash(shared_folder.to_string()),
            sub_node_name_str,
            sub_node_profile_str
        )
    }
}
