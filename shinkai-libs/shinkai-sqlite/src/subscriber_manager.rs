use rusqlite::params;
use shinkai_message_primitives::schemas::{
    shinkai_name::ShinkaiName,
    shinkai_subscription::{ShinkaiSubscription, SubscriptionId},
};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn add_subscriber_subscription(&self, subscription: ShinkaiSubscription) -> Result<(), SqliteManagerError> {
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
            "INSERT INTO shinkai_subscriptions (
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

    pub fn all_subscribers_for_folder(
        &self,
        shared_folder: &str,
    ) -> Result<Vec<ShinkaiSubscription>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM shinkai_subscriptions WHERE shared_folder = ?1")?;
        let mut rows = stmt.query(params![shared_folder])?;

        let mut subscriptions = Vec::new();
        while let Some(row) = rows.next()? {
            let subscription_id_data: Vec<u8> = row.get(1)?;
            let subscription_id = serde_json::from_slice(&subscription_id_data).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
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
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let subscriber_node = ShinkaiName::new(subscriber_node).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let payment = serde_json::from_str(&payment).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let state = serde_json::from_str(&state).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            subscriptions.push(ShinkaiSubscription {
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
        }

        Ok(subscriptions)
    }

    pub fn get_subscription_by_id(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<ShinkaiSubscription, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM shinkai_subscriptions WHERE subscription_id = ?1")?;
        let mut rows = stmt.query(params![subscription_id.get_unique_id()])?;

        if let Some(row) = rows.next()? {
            let subscription_id_data: Vec<u8> = row.get(1)?;
            let subscription_id = serde_json::from_slice(&subscription_id_data).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
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
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let subscriber_node = ShinkaiName::new(subscriber_node).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let payment = serde_json::from_str(&payment).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let state = serde_json::from_str(&state).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
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
        } else {
            Err(SqliteManagerError::DataNotFound)
        }
    }

    pub fn remove_subscriber(&self, subscription_id: &SubscriptionId) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            "DELETE FROM shinkai_subscriptions WHERE subscription_id = ?1",
            params![subscription_id.get_unique_id()],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn all_subscribers_subscription(&self) -> Result<Vec<ShinkaiSubscription>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM shinkai_subscriptions")?;
        let mut rows = stmt.query([])?;

        let mut subscriptions = Vec::new();
        while let Some(row) = rows.next()? {
            let subscription_id_data: Vec<u8> = row.get(1)?;
            let subscription_id = serde_json::from_slice(&subscription_id_data).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
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
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let subscriber_node = ShinkaiName::new(subscriber_node).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let payment = serde_json::from_str(&payment).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let state = serde_json::from_str(&state).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            subscriptions.push(ShinkaiSubscription {
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
        }

        Ok(subscriptions)
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

    #[test]
    fn test_add_subscriber_subscription() {
        let db = setup_test_db();
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

        let result = db.add_subscriber_subscription(subscription.clone());
        assert!(result.is_ok());

        let result = db.get_subscription_by_id(&subscription.subscription_id).unwrap();
        assert_eq!(result.subscription_id, subscription.subscription_id);
    }

    #[test]
    fn test_all_subscribers_for_folder() {
        let db = setup_test_db();
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

        let result = db.add_subscriber_subscription(subscription1.clone());
        assert!(result.is_ok());

        let result = db.all_subscribers_for_folder("shared_folder").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].subscription_id, subscription1.subscription_id);
    }

    #[test]
    fn test_remove_subscription() {
        let db = setup_test_db();
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

        let result = db.add_subscriber_subscription(subscription.clone());
        assert!(result.is_ok());

        let result = db.remove_subscriber(&subscription.subscription_id);
        assert!(result.is_ok());

        let result = db.get_subscription_by_id(&subscription.subscription_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_subscribers_subscription() {
        let db = setup_test_db();
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

        let result = db.add_subscriber_subscription(subscription1.clone());
        assert!(result.is_ok());

        let result = db.add_subscriber_subscription(subscription2.clone());
        assert!(result.is_ok());

        let result = db.all_subscribers_subscription().unwrap();
        assert_eq!(result.len(), 2);
        assert!(result
            .iter()
            .any(|s| s.subscription_id.get_unique_id() == subscription1.subscription_id.get_unique_id()));
        assert!(result
            .iter()
            .any(|s| s.subscription_id.get_unique_id() == subscription2.subscription_id.get_unique_id()));
    }
}
