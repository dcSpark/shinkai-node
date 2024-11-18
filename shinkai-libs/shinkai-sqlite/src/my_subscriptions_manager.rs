use rusqlite::params;
use shinkai_message_primitives::schemas::shinkai_subscription::ShinkaiSubscription;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn add_my_subscription(&self, subscription: ShinkaiSubscription) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Check if streaming_node exists in shinkai_names
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_names WHERE full_name = ?)",
            &[&subscription.streaming_node.full_name],
            |row| row.get(0),
        )?;

        if !exists {
            let subidentity_type =
                serde_json::to_string(&subscription.streaming_node.subidentity_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

            tx.execute(
                "INSERT INTO shinkai_names (full_name, node_name, profile_name, subidentity_type, subidentity_name)
                    VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    subscription.streaming_node.full_name,
                    subscription.streaming_node.node_name,
                    subscription.streaming_node.profile_name,
                    subidentity_type,
                    subscription.streaming_node.subidentity_name
                ],
            )?;
        }

        // Check if subscriber_node exists in shinkai_names
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_names WHERE full_name = ?)",
            &[&subscription.subscriber_node.full_name],
            |row| row.get(0),
        )?;

        if !exists {
            let subidentity_type =
                serde_json::to_string(&subscription.subscriber_node.subidentity_type).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

            tx.execute(
                "INSERT INTO shinkai_names (full_name, node_name, profile_name, subidentity_type, subidentity_name)
                    VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    subscription.subscriber_node.full_name,
                    subscription.subscriber_node.node_name,
                    subscription.subscriber_node.profile_name,
                    subidentity_type,
                    subscription.subscriber_node.subidentity_name
                ],
            )?;
        }

        let subscription_id = subscription.subscription_id.get_unique_id();
        let payment = serde_json::to_string(&subscription.payment).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        let state = serde_json::to_string(&subscription.state).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;

        tx.execute(
            "INSERT INTO my_subscriptions (
                subscription_id,
                shared_folder,
                streaming_node,
                streaming_profile,
                subscription_description,
                subscriber_destination_path,
                subscriber_node,
                subscriber_profile,
                payment,
                state,
                date_created,
                last_modified,
                last_sync,
                http_preferred
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                subscription_id,
                subscription.shared_folder,
                subscription.streaming_node.full_name,
                subscription.streaming_profile,
                subscription.subscription_description,
                subscription.subscriber_destination_path,
                subscription.subscriber_node.full_name,
                subscription.subscriber_profile,
                payment,
                state,
                &subscription.date_created.to_rfc3339(),
                &subscription.last_modified.to_rfc3339(),
                &subscription.last_sync.map(|dt| dt.to_rfc3339()),
                subscription.http_preferred
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_my_subscription(&self, subscription_id: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM my_subscriptions WHERE subscription_id = ?")?;

        stmt.execute(params![subscription_id])?;
        Ok(())
    }

    pub fn list_all_my_subscriptions(&self) -> Result<Vec<ShinkaiSubscription>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM my_subscriptions")?;

        let subscriptions = stmt
            .query_map([], |row| {
                let streaming_node: String = row.get(2)?;
                let subscriber_node: String = row.get(7)?;
                let payment: String = row.get(8)?;
                let state: String = row.get(9)?;

                let streaming_node = serde_json::from_str(&streaming_node).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let subscriber_node = serde_json::from_str(&subscriber_node).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let payment = serde_json::from_str(&payment).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let state = serde_json::from_str(&state).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

                Ok(ShinkaiSubscription {
                    subscription_id: row.get(0)?,
                    shared_folder: row.get(1)?,
                    streaming_node,
                    streaming_profile: row.get(3)?,
                    subscription_description: row.get(4)?,
                    subscriber_destination_path: row.get(5)?,
                    subscriber_node,
                    subscriber_profile: row.get(8)?,
                    payment,
                    state,
                    date_created: row.get(11)?,
                    last_modified: row.get(12)?,
                    last_sync: row.get(13)?,
                    http_preferred: row.get(14)?,
                })
            })?
            .collect::<Result<Vec<ShinkaiSubscription>, _>>()?;

        Ok(subscriptions)
    }
}
