use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkToolNotification {
    pub invoice_id: String,
    pub message: Value,
    pub datetime: DateTime<Utc>,
}

impl ShinkaiDB {
    /// Sets a network tool notification
    pub fn set_network_tool_notification(&self, invoice_id: String, message: Value) -> Result<(), ShinkaiDBError> {
        // Get the current timestamp
        let datetime = Utc::now();

        // Create the notification struct
        let notification = NetworkToolNotification {
            invoice_id: invoice_id.clone(),
            message,
            datetime,
        };

        // Serialize the notification
        let serialized_notification = serde_json::to_vec(&notification)?;

        // Create the composite key with the specified prefix and invoice_id
        let composite_key = format!(
            "network_tool_notifications_abcdefghijkl_prefix_{}",
            invoice_id.to_lowercase()
        );

        // Retrieve the handle to the "Inbox" column family
        let inbox_cf = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // Insert the serialized notification into the "Inbox" column family using the composite key
        self.db.put_cf(inbox_cf, composite_key, serialized_notification)?;

        Ok(())
    }

    /// Gets a network tool notification
    pub fn get_network_tool_notification(
        &self,
        invoice_id: String,
    ) -> Result<Option<NetworkToolNotification>, ShinkaiDBError> {
        // Create the composite key with the specified prefix and invoice_id
        let composite_key = format!(
            "network_tool_notifications_abcdefghijkl_prefix_{}",
            invoice_id.to_lowercase()
        );

        // Retrieve the handle to the "Inbox" column family
        let inbox_cf = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // Get the serialized notification from the "Inbox" column family using the composite key
        match self.db.get_cf(inbox_cf, composite_key)? {
            Some(serialized_notification) => {
                let notification: NetworkToolNotification = serde_json::from_slice(&serialized_notification)?;
                Ok(Some(notification))
            }
            None => Ok(None),
        }
    }

    /// Deletes a network tool notification
    pub fn delete_network_tool_notification(&self, invoice_id: String) -> Result<(), ShinkaiDBError> {
        // Create the composite key with the specified prefix and invoice_id
        let composite_key = format!(
            "network_tool_notifications_abcdefghijkl_prefix_{}",
            invoice_id.to_lowercase()
        );

        // Retrieve the handle to the "Inbox" column family
        let inbox_cf = self.get_cf_handle(Topic::NodeAndUsers).unwrap();

        // Delete the notification from the "Inbox" column family using the composite key
        self.db.delete_cf(inbox_cf, composite_key)?;

        Ok(())
    }
}
