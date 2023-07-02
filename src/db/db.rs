use rocksdb::{Options, DB, Error, ColumnFamilyDescriptor, DBWithColumnFamilies};
use crate::shinkai_message_proto::ShinkaiMessage;
use std::convert::TryInto;

// Define the Topics enum
pub enum Topic {
    Peers,
    ScheduledMessage,
    AllMessages,
    AllMessagesTimeKeyed,
}

impl Topic {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Peers => "peers",
            Self::ScheduledMessage => "scheduled_message",
            Self::AllMessages => "all_messages",
            Self::AllMessagesTimeKeyed => "all_messages_time_keyed",
        }
    }
}

pub struct ShinkaiMessageDB {
    db: DBWithColumnFamilies,
}

impl ShinkaiMessageDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let mut options = Options::default();
        options.create_if_missing(true);

        // Define column families
        let default_cf = ColumnFamilyDescriptor::default();
        let peers_cf = ColumnFamilyDescriptor::new(Topic::Peers.as_str(), Options::default());
        let to_send_cf = ColumnFamilyDescriptor::new(Topic::ToSend.as_str(), Options::default());
        let all_messages_cf = ColumnFamilyDescriptor::new(Topic::AllMessages.as_str(), Options::default());
        let all_messages_time_keyed_cf = ColumnFamilyDescriptor::new(Topic::AllMessagesTimeKeyed.as_str(), Options::default());

        let cfs_vec = vec![default_cf, peers_cf, to_send_cf, all_messages_cf, all_messages_time_keyed_cf];

        let db = DB::open_cf_descriptors(&options, db_path, cfs_vec)?;

        Ok(ShinkaiMessageDB { db })
    }

    pub fn insert(&self, key: String, message: &ShinkaiMessage, topic: Topic) -> Result<(), Error> {
        // As protobuf uses bytes to serialize data, we can use this to store into RocksDB
        let message_bytes = message.write_to_bytes().unwrap();
        let cf = self.db.cf_handle(topic.as_str()).ok_or_else(|| Error::new("Unknown column family"))?;
        self.db.put_cf(cf, key, message_bytes)
    }

    pub fn get(&self, key: String, topic: Topic) -> Result<Option<ShinkaiMessage>, Error> {
        let cf = self.db.cf_handle(topic.as_str()).ok_or_else(|| Error::new("Unknown column family"))?;
        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let message = ShinkaiMessage::parse_from_bytes(&bytes).unwrap();
                Ok(Some(message))
            },
            None => Ok(None)
        }
    }

    pub fn insert_message(&self, message: &ShinkaiMessage) -> Result<(), Error> {
        // Calculate the hash of the message for the key
        let hash_key = ShinkaiMessageHandler::calculate_hash(message);
        
        // Calculate the scheduled time or current time
        let time_key = match message.scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => message.scheduled_time.clone(),
        };

        // Create a write batch
        let mut batch = rocksdb::WriteBatch::default();

        // Define the data for AllMessages
        let all_messages_cf = self.db.cf_handle(Topic::AllMessages.as_str()).ok_or_else(|| Error::new("Unknown column family"))?;
        let message_bytes = message.write_to_bytes().unwrap();
        batch.put_cf(all_messages_cf, &hash_key, &message_bytes);

        // Define the data for AllMessagesTimeKeyed
        let all_messages_time_keyed_cf = self.db.cf_handle(Topic::AllMessagesTimeKeyed.as_str()).ok_or_else(|| Error::new("Unknown column family"))?;
        batch.put_cf(all_messages_time_keyed_cf, &time_key, &hash_key);

        // Atomically apply the updates
        self.db.write(batch)?;

        Ok(())
    }

    pub fn schedule_message(&self, message: &ShinkaiMessage) -> Result<(), Error> {
        // Calculate the scheduled time or current time
        let time_key = match message.scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => message.scheduled_time.clone(),
        };

        // Convert ShinkaiMessage into bytes for storage
        let message_bytes = message.write_to_bytes().unwrap();

        // Retrieve the handle to the "ToSend" column family
        let to_send_cf = self.db.cf_handle(Topic::ScheduledMessage.as_str()).ok_or_else(|| Error::new("Unknown column family"))?;

        // Insert the message into the "ToSend" column family using the time key
        self.db.put_cf(to_send_cf, time_key, message_bytes)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::Builder;

    fn get_test_db_path() -> String {
        let temp_dir = Builder::new()
            .prefix("test_db")
            .rand_bytes(5)
            .tempdir()
            .unwrap();
        temp_dir.into_path().to_str().unwrap().to_string()
    }

    fn get_test_message() -> ShinkaiMessage {
        let (secret_key, public_key) = unsafe_deterministic_private_key(0);

        // Replace this with actual field data
        let fields = HashMap::new();

        // Build the ShinkaiMessage
        ShinkaiMessageBuilder::new(&secret_key, &public_key)
            .body("body content".to_string())
            .encryption("no_encryption".to_string())
            .message_schema_type("schema type".to_string(), &fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata(&public_key)
            .build()
            .unwrap()
    }

    #[test]
    fn test_insert_get() {
        let db_path = get_test_db_path();
        let db = ShinkaiMessageDB::new(&db_path).unwrap();
        let message = get_test_message();

        // Insert the message in AllMessages topic
        let key = ShinkaiMessageHandler::calculate_hash(&message);
        db.insert(key.clone(), &message, Topic::AllMessages).unwrap();

        // Retrieve the message and validate it
        let retrieved_message = db.get(key, Topic::AllMessages).unwrap().unwrap();
        assert_eq!(message, retrieved_message);
    }

    #[test]
    fn test_insert_message() {
        let db_path = get_test_db_path();
        let db = ShinkaiMessageDB::new(&db_path).unwrap();
        let message = get_test_message();

        // Insert the message
        db.insert_message(&message).unwrap();

        // Retrieve the message from AllMessages and validate it
        let all_messages_key = ShinkaiMessageHandler::calculate_hash(&message);
        let retrieved_message = db.get(all_messages_key, Topic::AllMessages).unwrap().unwrap();
        assert_eq!(message, retrieved_message);

        // Retrieve the pointer from AllMessagesTimeKeyed and validate it
        let time_keyed_key = if message.scheduled_time.is_empty() {
            ShinkaiMessageHandler::generate_time_now()
        } else {
            message.scheduled_time.clone()
        };
        let retrieved_key = db.get(time_keyed_key, Topic::AllMessagesTimeKeyed).unwrap().unwrap();
        assert_eq!(all_messages_key, retrieved_key);
    }

    #[test]
    fn test_schedule_message() {
        let db_path = get_test_db_path();
        let db = ShinkaiMessageDB::new(&db_path).unwrap();
        let message = get_test_message();

        // Schedule the message
        db.schedule_message(&message).unwrap();

        // Retrieve the scheduled message and validate it
        let scheduled_key = if message.scheduled_time.is_empty() {
            ShinkaiMessageHandler::generate_time_now()
        } else {
            message.scheduled_time.clone()
        };
        let retrieved_message = db.get(scheduled_key, Topic::ScheduledMessage).unwrap().unwrap();
        assert_eq!(message, retrieved_message);
    }
}
