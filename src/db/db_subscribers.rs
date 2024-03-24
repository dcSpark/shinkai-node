use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::{
    shinkai_name::ShinkaiName,
    shinkai_subscription_req::{FolderSubscription, SubscriptionPayment},
};

impl ShinkaiDB {
    /// Returns the first half of the blake3 hash of the folder name value
    pub fn folder_name_to_hash(folder_name: String) -> String {
        let full_hash = blake3::hash(folder_name.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    pub fn add_subscriber(
        &mut self,
        shared_folder: &str,
        node_name: ShinkaiName,
        payment: SubscriptionPayment,
    ) -> Result<(), ShinkaiDBError> {
        let node_name_str = node_name.get_node_name();

        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // 47 characters are required so prefix search works
        let prefix_all = format!(
            "user_shared_folders_subscriptions_abcde_prefix_{}_{}",
            node_name_str, shared_folder
        );

        let prefix_folder = format!(
            "subscriptions_{}_{}",
            Self::folder_name_to_hash(shared_folder.to_string()),
            node_name_str
        );

        let payment_bytes = bincode::serialize(&payment).expect("Failed to serialize payment");

        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_node, prefix_all.as_bytes(), &payment_bytes);
        batch.put_cf(cf_node, prefix_folder.as_bytes(), &payment_bytes);
        self.db.write(batch)?;

        Ok(())
    }

    /// Retrieves all subscribers for a given shared folder.
    pub fn all_subscribers_for_folder(
        &self,
        shared_folder: &str,
    ) -> Result<Vec<(String, SubscriptionPayment)>, ShinkaiDBError> {
        let folder_hash = Self::folder_name_to_hash(shared_folder.to_string());
        let prefix_search_key = format!("subscriptions_{}_", folder_hash);
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        let mut subscribers = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_node, prefix_search_key.as_bytes());

        for item in iterator {
            let (key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let key_str = String::from_utf8_lossy(&key);
            let parts: Vec<&str> = key_str.split('_').collect();
            if parts.len() < 3 {
                continue; // Skip if the key format is not as expected
            }
            let node_name_str = parts[2]; // Assuming the node name is the third part of the key
            let payment: SubscriptionPayment =
                bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            subscribers.push((node_name_str.to_string(), payment));
        }

        Ok(subscribers)
    }

    /// Removes a subscriber to a folder for a specific node_name.
    pub fn remove_subscriber(&mut self, shared_folder: &str, node_name: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        let node_name_str = node_name.get_node_name();
        let folder_hash = Self::folder_name_to_hash(shared_folder.to_string());

        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // Construct the keys for deletion based on the same format used in add_subscriber
        let prefix_all = format!(
            "user_shared_folders_subscriptions_abcde_prefix_{}_{}",
            node_name_str, shared_folder
        );

        let prefix_folder = format!("subscriptions_{}_{}", folder_hash, node_name_str);

        // Perform the deletion from the database
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_node, prefix_all.as_bytes());
        batch.delete_cf(cf_node, prefix_folder.as_bytes());
        self.db.write(batch)?;

        Ok(())
    }

    /// Retrieves all subscribers along with their subscription details.
    pub fn all_subscribers_subscription(
        &self,
    ) -> Result<Vec<(String, String, SubscriptionPayment)>, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix_search_key = "user_shared_folders_subscriptions_abcde_prefix_".as_bytes();

        let mut subscriptions = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_node, prefix_search_key);

        for item in iterator {
            let (key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let key_str = String::from_utf8_lossy(&key);
            let parts: Vec<&str> = key_str.split('_').collect();
            if parts.len() < 8 {
                continue; // Skip if the key format is not as expected
            }
            // Adjusting indices according to the prefix format
            let node_name_str = parts[6];
            let folder = parts[7];
            let payment: SubscriptionPayment =
                bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            subscriptions.push((node_name_str.to_string(), folder.to_string(), payment));
        }

        Ok(subscriptions)
    }
}
