use std::net::SocketAddr;

use super::{db_errors::ShinkaiDBError, db_main::Topic, ShinkaiDB};
use chrono::{DateTime, Utc};
use rocksdb::IteratorMode;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryMessage {
    pub retry_count: u32,
    pub message: ShinkaiMessage,
    pub save_to_db_flag: bool,
    pub peer: (SocketAddr, String),
}

impl ShinkaiDB {
    /// Adds a message to the MessagesToRetry column family.
    pub fn add_message_to_retry(
        &self,
        retry_message: &RetryMessage,
        retry_time: DateTime<Utc>,
    ) -> Result<(), ShinkaiDBError> {
        // Calculate the hash of the message for the key
        let hash_key = retry_message.message.calculate_message_hash_for_pagination();

        // Create a composite key by concatenating the retry_time, retry_count and the hash_key, with a separator
        let composite_key = format!(
            "{}:::{}:::{}",
            retry_time.to_rfc3339(),
            retry_message.retry_count,
            hash_key
        );

        // Serialize RetryMessage into bytes for storage
        let retry_message_bytes = bincode::serialize(retry_message).map_err(|_| ShinkaiDBError::InvalidData)?;

        // Retrieve the handle to the "MessagesToRetry" column family
        let messages_to_retry_cf = self.get_cf_handle(Topic::MessagesToRetry).unwrap();

        // Insert the RetryMessage into the "MessagesToRetry" column family using the composite key
        self.db
            .put_cf(messages_to_retry_cf, composite_key, retry_message_bytes)?;

        Ok(())
    }

    /// Removes a message from the MessagesToRetry column family.
    pub fn remove_message_from_retry(&self, message: &ShinkaiMessage) -> Result<(), ShinkaiDBError> {
        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash_for_pagination();

        // Retrieve the handle to the "MessagesToRetry" column family
        let messages_to_retry_cf = self.get_cf_handle(Topic::MessagesToRetry).unwrap();

        // Get an iterator over the column family from the start
        let iter = self.db.iterator_cf(messages_to_retry_cf, IteratorMode::Start);

        for item in iter {
            // Unwrap the Result
            let (key, _value) = item.map_err(ShinkaiDBError::from)?;

            // Convert the Vec<u8> key into a string
            let key_str = std::str::from_utf8(&key).map_err(|_| ShinkaiDBError::InvalidData)?;

            // Split the composite key to get the time component, retry count and hash key
            let mut parts = key_str.split(":::");
            let time_key_str = parts.next().ok_or(ShinkaiDBError::InvalidData)?;
            let retry_count_str = parts.next().ok_or(ShinkaiDBError::InvalidData)?;
            let hash_key_str = parts.next().ok_or(ShinkaiDBError::InvalidData)?;

            // If the hash_key matches, delete the message
            if hash_key_str == hash_key {
                // Create a composite key by concatenating the time_key, retry_count_str and the hash_key_str, with a separator
                let composite_key = format!("{}:::{}:::{}", time_key_str, retry_count_str, hash_key_str);

                // Delete the message from the "MessagesToRetry" column family using the composite key
                self.db.delete_cf(messages_to_retry_cf, composite_key)?;

                break;
            }
        }

        Ok(())
    }

    /// Fetches all messages from the MessagesToRetry column family that were scheduled for retry before a given time.
    pub fn get_messages_to_retry_before(
        &self,
        up_to_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<RetryMessage>, ShinkaiDBError> {
        // Retrieve the handle to the "MessagesToRetry" column family
        let messages_to_retry_cf = self.get_cf_handle(Topic::MessagesToRetry).unwrap();

        // Get an iterator over the column family from the start
        let iter = self.db.iterator_cf(messages_to_retry_cf, IteratorMode::Start);

        // Use the current time if no time is provided
        let up_to_time = up_to_time.unwrap_or_else(Utc::now);

        // Collect all messages before the up_to_time
        let mut retry_messages = Vec::new();
        for item in iter {
            // Unwrap the Result
            let (key, value) = item.map_err(ShinkaiDBError::from)?;

            // Convert the Vec<u8> key into a string
            let key_str = std::str::from_utf8(&key).map_err(|_| ShinkaiDBError::InvalidData)?;

            // Split the composite key to get the time component, retry count and hash key
            let mut parts = key_str.split(":::");
            let time_key_str = parts.next().ok_or(ShinkaiDBError::InvalidData)?;

            // Parse the time_key into a DateTime object
            let time_key = DateTime::parse_from_rfc3339(time_key_str)
                .map_err(|_| ShinkaiDBError::InvalidData)?
                .with_timezone(&Utc);

            // Compare the time key with the up_to_time
            if time_key > up_to_time {
                // Break the loop if we've started seeing messages scheduled for later
                break;
            }

            // Deserialize the RetryMessage
            let retry_message = bincode::deserialize(&value).map_err(|_| ShinkaiDBError::InvalidData)?;

            retry_messages.push(retry_message);
        }

        Ok(retry_messages)
    }
}
