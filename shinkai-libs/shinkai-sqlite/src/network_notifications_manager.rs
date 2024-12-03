use rusqlite::params;
use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_network::UserNetworkNotification};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn write_notification(&self, user_profile: ShinkaiName, message: String) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO network_notifications (full_name, message, timestamp)
                    VALUES (?1, ?2, ?3)",
            params![user_profile.full_name, message, chrono::Utc::now().to_rfc3339()],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_last_notifications(
        &self,
        user_profile: ShinkaiName,
        count: usize,
        timestamp: Option<String>,
    ) -> Result<Vec<UserNetworkNotification>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = if let Some(ref _ts) = timestamp {
            conn.prepare(
                "SELECT message, timestamp
                FROM network_notifications
                WHERE full_name = ?1 AND timestamp > ?2
                ORDER BY timestamp DESC
                LIMIT ?3",
            )?
        } else {
            conn.prepare(
                "SELECT message, timestamp
                FROM network_notifications
                WHERE full_name = ?1
                ORDER BY timestamp DESC
                LIMIT ?2",
            )?
        };

        let mut rows = if let Some(ref ts) = timestamp {
            stmt.query(params![user_profile.full_name, ts, count])?
        } else {
            stmt.query(params![user_profile.full_name, count])?
        };
        let mut notifications = Vec::new();

        while let Some(row) = rows.next()? {
            let datetime: String = row.get(1)?;
            notifications.push(UserNetworkNotification {
                message: row.get(0)?,
                datetime: datetime.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                        e.to_string(),
                    )))
                })?,
            });
        }

        Ok(notifications)
    }

    pub fn get_notifications_before_timestamp(
        &self,
        user_profile: ShinkaiName,
        timestamp: String,
        count: usize,
    ) -> Result<Vec<UserNetworkNotification>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT message, timestamp
                    FROM network_notifications
                    WHERE full_name = ?1 AND timestamp < ?2
                    ORDER BY timestamp ASC
                    LIMIT ?3",
        )?;

        let mut rows = stmt.query(params![user_profile.full_name, timestamp, count])?;
        let mut notifications = Vec::new();

        while let Some(row) = rows.next()? {
            let datetime: String = row.get(1)?;
            notifications.push(UserNetworkNotification {
                message: row.get(0)?,
                datetime: datetime.parse::<chrono::DateTime<chrono::Utc>>().map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                        e.to_string(),
                    )))
                })?,
            });
        }

        Ok(notifications)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::{path::PathBuf, thread::sleep};
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
    fn test_write_notification() {
        let db = setup_test_db();
        let user_profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        let result = db.write_notification(user_profile.clone(), "Test message".to_string());
        assert!(result.is_ok());
        let notifications = db.get_last_notifications(user_profile.clone(), 1, None).unwrap();

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "Test message");
    }

    #[test]
    fn test_get_last_notifications() {
        let db = setup_test_db();
        let user_profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();

        let messages = vec![
            "Test message 1".to_string(),
            "Test message 2".to_string(),
            "Test message 3".to_string(),
            "Test message 4".to_string(),
        ];

        for message in &messages {
            db.write_notification(user_profile.clone(), message.clone()).unwrap();
            sleep(std::time::Duration::from_millis(1));
        }

        let notifications = db.get_last_notifications(user_profile.clone(), 2, None).unwrap();
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].message, "Test message 4");
        assert_eq!(notifications[1].message, "Test message 3");

        let notifications = db.get_last_notifications(user_profile, 3, None).unwrap();
        assert_eq!(notifications.len(), 3);
        assert_eq!(notifications[0].message, "Test message 4");
        assert_eq!(notifications[1].message, "Test message 3");
        assert_eq!(notifications[2].message, "Test message 2");
    }

    #[test]
    fn test_get_last_notifications_with_timestamp() {
        let db = setup_test_db();
        let user_profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();
        let message1 = "Test message 1".to_string();
        let message2 = "Test message 2".to_string();
        let message3 = "Test message 3".to_string();
        let message4 = "Test message 4".to_string();

        db.write_notification(user_profile.clone(), message1).unwrap();
        sleep(std::time::Duration::from_millis(1));
        let timestamp = Utc::now().to_rfc3339();
        sleep(std::time::Duration::from_millis(1));
        db.write_notification(user_profile.clone(), message2).unwrap();
        sleep(std::time::Duration::from_millis(1));
        db.write_notification(user_profile.clone(), message3).unwrap();
        sleep(std::time::Duration::from_millis(1));
        db.write_notification(user_profile.clone(), message4).unwrap();

        let notifications = db.get_last_notifications(user_profile, 2, Some(timestamp)).unwrap();
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].message, "Test message 4");
        assert_eq!(notifications[1].message, "Test message 3");
    }

    #[test]
    fn test_get_notifications_before_timestamp() {
        let db = setup_test_db();
        let user_profile = ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap();
        let message1 = "Test message 1".to_string();
        let message2 = "Test message 2".to_string();
        let message3 = "Test message 3".to_string();

        db.write_notification(user_profile.clone(), message1).unwrap();
        sleep(std::time::Duration::from_millis(1));
        db.write_notification(user_profile.clone(), message2).unwrap();
        sleep(std::time::Duration::from_millis(1));
        let timestamp = Utc::now().to_rfc3339();
        sleep(std::time::Duration::from_millis(1));
        db.write_notification(user_profile.clone(), message3).unwrap();

        let notifications = db
            .get_notifications_before_timestamp(user_profile, timestamp, 3)
            .unwrap();
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].message, "Test message 1");
        assert_eq!(notifications[1].message, "Test message 2");
    }
}
