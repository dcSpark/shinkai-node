use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use shinkai_message_primitives::schemas::shinkai_subscription::ShinkaiSubscription;

impl ShinkaiDB {
    pub fn add_my_subscription(&self, subscription: ShinkaiSubscription) -> Result<(), ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // 47 characters are required so prefix search works
        let prefix_all = format!(
            "user_personal_subscriptions_abcdefghijk_prefix_{}",
            subscription.subscription_id.get_unique_id()
        );

        let subscription_bytes = bincode::serialize(&subscription).expect("Failed to serialize payment");
        self.db.put_cf(cf_node, prefix_all.as_bytes(), subscription_bytes)?;

        Ok(())
    }

    /// Removes a subscription.
    pub fn remove_my_subscription(&self, subscription_id: &str) -> Result<(), ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // Construct the key for the subscription to be removed
        let prefix_all = format!("user_personal_subscriptions_abcdefghijk_prefix_{}", subscription_id);

        // Perform the deletion from the database
        self.db.delete_cf(cf_node, prefix_all.as_bytes())?;

        Ok(())
    }

    /// Retrieves all of my subscriptions.
    pub fn list_all_my_subscriptions(&self) -> Result<Vec<ShinkaiSubscription>, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        let prefix_search_key = "user_personal_subscriptions_abcdefghijk_prefix_".as_bytes();

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

    /// Updates a subscription.
    pub fn update_my_subscription(&self, new_subscription: ShinkaiSubscription) -> Result<(), ShinkaiDBError> {
        // Use shared CFs
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        let prefix_all = format!(
            "user_personal_subscriptions_abcdefghijk_prefix_{}",
            new_subscription.subscription_id.get_unique_id()
        );

        let subscription_bytes = bincode::serialize(&new_subscription).expect("Failed to serialize subscription");

        // Update the subscription in the database
        self.db.put_cf(cf_node, prefix_all.as_bytes(), subscription_bytes)?;

        Ok(())
    }

    /// Retrieves a subscription by its ID.
    pub fn get_my_subscription(&self, subscription_id: &str) -> Result<ShinkaiSubscription, ShinkaiDBError> {
        let cf_node = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        let prefix_all = format!("user_personal_subscriptions_abcdefghijk_prefix_{}", subscription_id);

        match self.db.get_cf(cf_node, prefix_all.as_bytes()) {
            Ok(Some(value)) => {
                let subscription: ShinkaiSubscription =
                    bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;
                Ok(subscription)
            }
            Ok(None) => Err(ShinkaiDBError::DataNotFound),
            Err(e) => Err(ShinkaiDBError::RocksDBError(e)),
        }
    }
}
