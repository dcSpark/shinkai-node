use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_network::UserNetworkNotification};

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    /// Writes a notification to the Inbox with a specific prefix
    pub fn write_notification(&self, user_profile: ShinkaiName, message: String) -> Result<(), ShinkaiDBError> {
        // Get the profile name string
        let profile_name = Self::get_profile_name_string(&user_profile)?;

        // Calculate the half hash of the profile name
        let half_hash = Self::hex_blake3_to_half_hash(&profile_name);

        // Get the current timestamp
        let datetime = Utc::now();

        // Calculate reverse time key by subtracting from Unix time of 2420-01-01
        let future_time = DateTime::parse_from_rfc3339("2420-01-01T00:00:00Z")
            .unwrap()
            .timestamp_millis();
        let reverse_time_key = future_time - datetime.timestamp_millis();

        // Create the composite key with the specified prefix and reverse time key
        let composite_key = format!("network_notif_{}_{}", half_hash, reverse_time_key);

        // Create the notification struct
        let notification = UserNetworkNotification { message, datetime };

        // Serialize the notification
        let serialized_notification = serde_json::to_vec(&notification)?;

        // Retrieve the handle to the "Inbox" column family
        let inbox_cf = self.get_cf_handle(Topic::Inbox).unwrap();

        // Insert the serialized notification into the "Inbox" column family using the composite key
        self.db.put_cf(inbox_cf, composite_key, serialized_notification)?;

        Ok(())
    }

    pub fn get_last_notifications(
        &self,
        user_profile: ShinkaiName,
        count: usize,
        timestamp: Option<String>,
    ) -> Result<Vec<UserNetworkNotification>, ShinkaiDBError> {
        // Get the profile name string
        let profile_name = Self::get_profile_name_string(&user_profile)?;

        // Calculate the half hash of the profile name
        let half_hash = Self::hex_blake3_to_half_hash(&profile_name);

        // Retrieve the handle to the "Inbox" column family
        let inbox_cf = self.get_cf_handle(Topic::Inbox).unwrap();

        // Create the prefix to search for
        let prefix = format!("network_notif_{}_", half_hash);

        // Create an iterator to scan the "Inbox" column family with prefix
        let iter = self.db.prefix_iterator_cf(inbox_cf, prefix.as_bytes());

        // Calculate the future time for reverse time key calculation
        let future_time = DateTime::parse_from_rfc3339("2420-01-01T00:00:00Z")
            .unwrap()
            .timestamp_millis();

        // Collect the last `count` notifications after the specified timestamp
        let mut notifications = Vec::new();
        for item in iter {
            let (key, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let key_str = String::from_utf8(key.to_vec()).unwrap();
            if let Some(ref ts) = timestamp {
                let ts_datetime = DateTime::parse_from_rfc3339(ts).unwrap().with_timezone(&Utc);
                let ts_reverse_time_key = future_time - ts_datetime.timestamp_millis();

                let ts_composite_key = format!("{}{}", prefix, ts_reverse_time_key);

                if key_str > ts_composite_key {
                    continue;
                }
            }
            let notification: UserNetworkNotification = serde_json::from_slice(&value)?;
            notifications.push(notification);
            if notifications.len() == count {
                break;
            }
        }

        Ok(notifications)
    }

    /// Retrieves the previous X notifications before a specified timestamp for a given user profile
    pub fn get_notifications_before_timestamp(
        &self,
        user_profile: ShinkaiName,
        timestamp: String,
        count: usize,
    ) -> Result<Vec<UserNetworkNotification>, ShinkaiDBError> {
        // Get the profile name string
        let profile_name = Self::get_profile_name_string(&user_profile)?;

        // Calculate the half hash of the profile name
        let half_hash = Self::hex_blake3_to_half_hash(&profile_name);

        // Retrieve the handle to the "Inbox" column family
        let inbox_cf = self.get_cf_handle(Topic::Inbox).unwrap();

        // Create the prefix to search for
        let prefix = format!("network_notif_{}_", half_hash);

        // Create an iterator to scan the "Inbox" column family with prefix
        let iter = self.db.prefix_iterator_cf(inbox_cf, prefix.as_bytes());

        // Calculate the future time for reverse time key calculation
        let future_time = DateTime::parse_from_rfc3339("2420-01-01T00:00:00Z")
            .unwrap()
            .timestamp_millis();

        // Convert timestamp to reverse time key
        let ts_datetime = DateTime::parse_from_rfc3339(&timestamp).unwrap().with_timezone(&Utc);
        let ts_reverse_time_key = future_time - ts_datetime.timestamp_millis();

        // Collect notifications up to the specified timestamp
        let mut notifications = Vec::new();
        for item in iter {
            let (key, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let key_str = String::from_utf8(key.to_vec()).unwrap();

            if key_str < format!("{}{}", prefix, ts_reverse_time_key) {
                continue;
            }

            let notification: UserNetworkNotification = serde_json::from_slice(&value)?;
            notifications.push(notification);
            if notifications.len() == count {
                break;
            }
        }

        // Reverse the order of the notifications
        notifications.reverse();

        Ok(notifications)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::utils::hash_string;
    use std::fs;
    use std::path::Path;
    use std::thread::sleep;

    fn setup() -> ShinkaiDB {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(path);

        let node1_db_path = format!("db_tests/{}", hash_string("churrasco italiano"));
        ShinkaiDB::new(node1_db_path.as_str()).unwrap()
    }

    fn test_user() -> ShinkaiName {
        ShinkaiName::new("@@test_user.shinkai/main".to_string()).unwrap()
    }

    fn write_unprefixed_message(
        db: &ShinkaiDB,
        _user_profile: ShinkaiName,
        message: String,
    ) -> Result<(), ShinkaiDBError> {
        // Get the current timestamp in ISO 8601 format with timezone
        let timestamp = Utc::now().to_rfc3339();

        // Create the composite key without the specified prefix
        let composite_key = format!("unprefixed_{}", timestamp);

        // Retrieve the handle to the "Inbox" column family
        let inbox_cf = db.get_cf_handle(Topic::Inbox).unwrap();

        // Insert the message into the "Inbox" column family using the composite key
        db.db.put_cf(inbox_cf, composite_key, message)?;

        Ok(())
    }

    #[test]
    fn test_write_notification() {
        let db = setup();
        let user_profile = test_user();
        let message = "Test message".to_string();

        let result = db.write_notification(user_profile, message);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_last_notifications() {
        let db = setup();
        let user_profile = test_user();
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

        // Write an unprefixed message
        write_unprefixed_message(&db, user_profile.clone(), "Unprefixed message".to_string()).unwrap();

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
        let db = setup();
        let user_profile = test_user();
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

        // Write an unprefixed message
        write_unprefixed_message(&db, user_profile.clone(), "Unprefixed message".to_string()).unwrap();

        let notifications = db.get_last_notifications(user_profile, 2, Some(timestamp)).unwrap();
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].message, "Test message 4");
        assert_eq!(notifications[1].message, "Test message 3");
    }

    #[test]
    fn test_get_notifications_before_timestamp() {
        let db = setup();
        let user_profile = test_user();
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

        // Write an unprefixed message
        write_unprefixed_message(&db, user_profile.clone(), "Unprefixed message".to_string()).unwrap();

        let notifications = db
            .get_notifications_before_timestamp(user_profile, timestamp, 3)
            .unwrap();
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].message, "Test message 1");
        assert_eq!(notifications[1].message, "Test message 2");
    }
}
