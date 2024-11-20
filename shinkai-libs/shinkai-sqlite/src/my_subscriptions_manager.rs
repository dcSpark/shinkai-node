use rusqlite::params;
use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_subscription::ShinkaiSubscription};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn add_my_subscription(&self, subscription: ShinkaiSubscription) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let subscription_id = subscription.subscription_id.get_unique_id();
        let subscription_id_data = serde_json::to_vec(&subscription.subscription_id).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        let payment = serde_json::to_string(&subscription.payment).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        let state = serde_json::to_string(&subscription.state).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;

        tx.execute(
            "INSERT INTO my_subscriptions (
                subscription_id,
                subscription_id_data,
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
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                subscription_id,
                subscription_id_data,
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
                let subscription_id_data: Vec<u8> = row.get(1)?;
                let subscription_id = serde_json::from_slice(&subscription_id_data).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let shared_folder: String = row.get(2)?;
                let streaming_node: String = row.get(3)?;
                let streaming_profile: String = row.get(4)?;
                let subscription_description: Option<String> = row.get(5)?;
                let subscriber_destination_path: Option<String> = row.get(6)?;
                let subscriber_node: String = row.get(7)?;
                let subscriber_profile: String = row.get(8)?;
                let payment: String = row.get(9)?;
                let state: String = row.get(10)?;
                let date_created: String = row.get(11)?;
                let last_modified: String = row.get(12)?;
                let last_sync: Option<String> = row.get(13)?;
                let http_preferred: Option<bool> = row.get(14)?;

                let streaming_node = ShinkaiName::new(streaming_node).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let subscriber_node = ShinkaiName::new(subscriber_node).map_err(|e| {
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
                    date_created: date_created.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                    last_modified: last_modified.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                    last_sync: last_sync
                        .map(|dt| dt.parse::<chrono::DateTime<chrono::Utc>>())
                        .transpose()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    http_preferred: http_preferred,
                })
            })?
            .collect::<Result<Vec<ShinkaiSubscription>, _>>()?;

        Ok(subscriptions)
    }

    pub fn update_my_subscription(&self, new_subscription: ShinkaiSubscription) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let subscription_id = new_subscription.subscription_id.get_unique_id();
        let subscription_id_data = serde_json::to_vec(&new_subscription.subscription_id).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        let payment = serde_json::to_string(&new_subscription.payment).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        let state = serde_json::to_string(&new_subscription.state).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;

        tx.execute(
            "UPDATE my_subscriptions SET
                subscription_id_data = ?2,
                shared_folder = ?3,
                streaming_node = ?4,
                streaming_profile = ?5,
                subscription_description = ?6,
                subscriber_destination_path = ?7,
                subscriber_node = ?8,
                subscriber_profile = ?9,
                payment = ?10,
                state = ?11,
                date_created = ?12,
                last_modified = ?13,
                last_sync = ?14,
                http_preferred = ?15
                WHERE subscription_id = ?1",
            params![
                subscription_id,
                subscription_id_data,
                new_subscription.shared_folder,
                new_subscription.streaming_node.full_name,
                new_subscription.streaming_profile,
                new_subscription.subscription_description,
                new_subscription.subscriber_destination_path,
                new_subscription.subscriber_node.full_name,
                new_subscription.subscriber_profile,
                payment,
                state,
                &new_subscription.date_created.to_rfc3339(),
                &new_subscription.last_modified.to_rfc3339(),
                &new_subscription.last_sync.map(|dt| dt.to_rfc3339()),
                new_subscription.http_preferred,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_my_subscription(&self, subscription_id: &str) -> Result<ShinkaiSubscription, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM my_subscriptions WHERE subscription_id = ?")?;

        let subscription = stmt
            .query_row(params![subscription_id], |row| {
                let subscription_id_data: Vec<u8> = row.get(1)?;
                let subscription_id = serde_json::from_slice(&subscription_id_data).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let shared_folder: String = row.get(2)?;
                let streaming_node: String = row.get(3)?;
                let streaming_profile: String = row.get(4)?;
                let subscription_description: Option<String> = row.get(5)?;
                let subscriber_destination_path: Option<String> = row.get(6)?;
                let subscriber_node: String = row.get(7)?;
                let subscriber_profile: String = row.get(8)?;
                let payment: String = row.get(9)?;
                let state: String = row.get(10)?;
                let date_created: String = row.get(11)?;
                let last_modified: String = row.get(12)?;
                let last_sync: Option<String> = row.get(13)?;
                let http_preferred: Option<bool> = row.get(14)?;

                let streaming_node = ShinkaiName::new(streaming_node).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                let subscriber_node = ShinkaiName::new(subscriber_node).map_err(|e| {
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
                    date_created: date_created.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                    last_modified: last_modified.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                    last_sync: last_sync
                        .map(|dt| dt.parse::<chrono::DateTime<chrono::Utc>>())
                        .transpose()
                        .map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                                e.to_string(),
                            )))
                        })?,
                    http_preferred,
                })
            })
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    SqliteManagerError::SubscriptionNotFound(subscription_id.to_string())
                } else {
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        Ok(subscription)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::{
        shinkai_name::ShinkaiName,
        shinkai_subscription::{ShinkaiSubscriptionStatus, SubscriptionId},
        shinkai_subscription_req::SubscriptionPayment,
    };
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_add_my_subscription() {
        let manager = setup_test_db();
        let subscription = ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
                "streaming_profile".to_string(),
                "shared_folder".to_string(),
                ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
                "subscriber_profile".to_string(),
            ),
            shared_folder: "shared_folder".to_string(),
            streaming_node: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            streaming_profile: "streaming_profile".to_string(),
            subscription_description: Some("subscription_description".to_string()),
            subscriber_destination_path: Some("subscriber_destination_path".to_string()),
            subscriber_node: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            subscriber_profile: "subscriber_profile".to_string(),
            payment: Some(SubscriptionPayment::Free),
            state: ShinkaiSubscriptionStatus::SubscriptionRequested,
            date_created: chrono::Utc::now(),
            last_modified: chrono::Utc::now(),
            last_sync: Some(chrono::Utc::now()),
            http_preferred: Some(true),
        };

        let result = manager.add_my_subscription(subscription.clone());
        assert!(result.is_ok());

        let result = manager.get_my_subscription(subscription.subscription_id.get_unique_id());
        assert!(result.is_ok());
        let result = result.unwrap();

        assert_eq!(
            result.subscription_id.get_unique_id(),
            subscription.subscription_id.get_unique_id()
        );
        assert_eq!(result.shared_folder, subscription.shared_folder);
        assert_eq!(result.streaming_node.full_name, subscription.streaming_node.full_name);
        assert_eq!(result.streaming_profile, subscription.streaming_profile);
        assert_eq!(result.subscription_description, subscription.subscription_description);
        assert_eq!(
            result.subscriber_destination_path,
            subscription.subscriber_destination_path
        );
        assert_eq!(result.subscriber_node.full_name, subscription.subscriber_node.full_name);
        assert_eq!(result.subscriber_profile, subscription.subscriber_profile);
        assert_eq!(result.payment, subscription.payment);
        assert_eq!(result.state, subscription.state);
        assert_eq!(result.date_created, subscription.date_created);
        assert_eq!(result.last_modified, subscription.last_modified);
        assert_eq!(result.last_sync, subscription.last_sync);
        assert_eq!(result.http_preferred, subscription.http_preferred);
    }

    #[tokio::test]
    async fn test_remove_my_subscription() {
        let manager = setup_test_db();
        let subscription = ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
                "streaming_profile".to_string(),
                "shared_folder".to_string(),
                ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
                "subscriber_profile".to_string(),
            ),
            shared_folder: "shared_folder".to_string(),
            streaming_node: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            streaming_profile: "streaming_profile".to_string(),
            subscription_description: Some("subscription_description".to_string()),
            subscriber_destination_path: Some("subscriber_destination_path".to_string()),
            subscriber_node: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            subscriber_profile: "subscriber_profile".to_string(),
            payment: Some(SubscriptionPayment::Free),
            state: ShinkaiSubscriptionStatus::SubscriptionRequested,
            date_created: chrono::Utc::now(),
            last_modified: chrono::Utc::now(),
            last_sync: Some(chrono::Utc::now()),
            http_preferred: Some(true),
        };

        let result = manager.add_my_subscription(subscription.clone());
        assert!(result.is_ok());

        let result = manager.remove_my_subscription(subscription.subscription_id.get_unique_id());
        assert!(result.is_ok());

        let result = manager.get_my_subscription(subscription.subscription_id.get_unique_id());
        assert!(matches!(result, Err(SqliteManagerError::SubscriptionNotFound(_))));
    }

    #[tokio::test]
    async fn test_list_my_subscriptions() {
        let manager = setup_test_db();
        let subscription1 = ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
                "streaming_profile".to_string(),
                "shared_folder".to_string(),
                ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
                "subscriber_profile".to_string(),
            ),
            shared_folder: "shared_folder".to_string(),
            streaming_node: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            streaming_profile: "streaming_profile".to_string(),
            subscription_description: Some("subscription_description".to_string()),
            subscriber_destination_path: Some("subscriber_destination_path".to_string()),
            subscriber_node: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            subscriber_profile: "subscriber_profile".to_string(),
            payment: Some(SubscriptionPayment::Free),
            state: ShinkaiSubscriptionStatus::SubscriptionRequested,
            date_created: chrono::Utc::now(),
            last_modified: chrono::Utc::now(),
            last_sync: Some(chrono::Utc::now()),
            http_preferred: Some(true),
        };

        let subscription2 = ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                ShinkaiName::new("@@node3.shinkai/main_profile_node3".to_string()).unwrap(),
                "streaming_profile".to_string(),
                "shared_folder".to_string(),
                ShinkaiName::new("@@node4.shinkai/main_profile_node4".to_string()).unwrap(),
                "subscriber_profile".to_string(),
            ),
            shared_folder: "shared_folder".to_string(),
            streaming_node: ShinkaiName::new("@@node3.shinkai/main_profile_node3".to_string()).unwrap(),
            streaming_profile: "streaming_profile".to_string(),
            subscription_description: Some("subscription_description".to_string()),
            subscriber_destination_path: Some("subscriber_destination_path".to_string()),
            subscriber_node: ShinkaiName::new("@@node4.shinkai/main_profile_node4".to_string()).unwrap(),
            subscriber_profile: "subscriber_profile".to_string(),
            payment: Some(SubscriptionPayment::Free),
            state: ShinkaiSubscriptionStatus::SubscriptionRequested,
            date_created: chrono::Utc::now(),
            last_modified: chrono::Utc::now(),
            last_sync: Some(chrono::Utc::now()),
            http_preferred: Some(true),
        };

        let result = manager.add_my_subscription(subscription1.clone());
        assert!(result.is_ok());

        let result = manager.add_my_subscription(subscription2.clone());
        assert!(result.is_ok());

        let result = manager.list_all_my_subscriptions();
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.len(), 2);
        assert!(result
            .iter()
            .any(|s| s.subscription_id.get_unique_id() == subscription1.subscription_id.get_unique_id()));
        assert!(result
            .iter()
            .any(|s| s.subscription_id.get_unique_id() == subscription2.subscription_id.get_unique_id()));
    }

    #[tokio::test]
    async fn test_update_my_subscription() {
        let manager = setup_test_db();
        let subscription = ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
                "streaming_profile".to_string(),
                "shared_folder".to_string(),
                ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
                "subscriber_profile".to_string(),
            ),
            shared_folder: "shared_folder".to_string(),
            streaming_node: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            streaming_profile: "streaming_profile".to_string(),
            subscription_description: Some("subscription_description".to_string()),
            subscriber_destination_path: Some("subscriber_destination_path".to_string()),
            subscriber_node: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            subscriber_profile: "subscriber_profile".to_string(),
            payment: Some(SubscriptionPayment::Free),
            state: ShinkaiSubscriptionStatus::SubscriptionRequested,
            date_created: chrono::Utc::now(),
            last_modified: chrono::Utc::now(),
            last_sync: Some(chrono::Utc::now()),
            http_preferred: Some(true),
        };

        let result = manager.add_my_subscription(subscription.clone());
        assert!(result.is_ok());

        let new_subscription = ShinkaiSubscription {
            subscription_id: SubscriptionId::new(
                ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
                "streaming_profile".to_string(),
                "shared_folder".to_string(),
                ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
                "subscriber_profile".to_string(),
            ),
            shared_folder: "shared_folder".to_string(),
            streaming_node: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            streaming_profile: "streaming_profile".to_string(),
            subscription_description: Some("new_subscription_description".to_string()),
            subscriber_destination_path: Some("new_subscriber_destination_path".to_string()),
            subscriber_node: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            subscriber_profile: "new_subscriber_profile".to_string(),
            payment: Some(SubscriptionPayment::Free),
            state: ShinkaiSubscriptionStatus::SubscriptionConfirmed,
            date_created: subscription.date_created,
            last_modified: chrono::Utc::now(),
            last_sync: Some(chrono::Utc::now()),
            http_preferred: Some(true),
        };

        let result = manager.update_my_subscription(new_subscription.clone());
        assert!(result.is_ok());

        let result = manager.get_my_subscription(subscription.subscription_id.get_unique_id());
        assert!(result.is_ok());
        let result = result.unwrap();

        assert_eq!(
            result.subscription_id.get_unique_id(),
            new_subscription.subscription_id.get_unique_id()
        );
        assert_eq!(result.shared_folder, new_subscription.shared_folder);
        assert_eq!(
            result.streaming_node.full_name,
            new_subscription.streaming_node.full_name
        );
        assert_eq!(result.streaming_profile, new_subscription.streaming_profile);
        assert_eq!(
            result.subscription_description,
            new_subscription.subscription_description
        );
        assert_eq!(
            result.subscriber_destination_path,
            new_subscription.subscriber_destination_path
        );
        assert_eq!(
            result.subscriber_node.full_name,
            new_subscription.subscriber_node.full_name
        );
        assert_eq!(result.subscriber_profile, new_subscription.subscriber_profile);
        assert_eq!(result.payment, new_subscription.payment);
        assert_eq!(result.state, new_subscription.state);
        assert_eq!(result.date_created, new_subscription.date_created);
        assert_eq!(result.last_modified, new_subscription.last_modified);
        assert_eq!(result.last_sync, new_subscription.last_sync);
        assert_eq!(result.http_preferred, new_subscription.http_preferred);
    }
}
