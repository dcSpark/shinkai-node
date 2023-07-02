use prost::Message;
use rocksdb::{Options, DB, Error, ColumnFamilyDescriptor};
use crate::{shinkai_message_proto::ShinkaiMessage, shinkai_message::shinkai_message_handler::ShinkaiMessageHandler};
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
    db: DB,
}

impl ShinkaiMessageDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let mut options = Options::default();
        options.create_if_missing(true);


        let peers_cf = ColumnFamilyDescriptor::new(Topic::Peers.as_str(), Options::default());
        let to_send_cf = ColumnFamilyDescriptor::new(Topic::ScheduledMessage.as_str(), Options::default());
        let all_messages_cf = ColumnFamilyDescriptor::new(Topic::AllMessages.as_str(), Options::default());
        let all_messages_time_keyed_cf = ColumnFamilyDescriptor::new(Topic::AllMessagesTimeKeyed.as_str(), Options::default());

        let cfs_vec = vec![peers_cf, to_send_cf, all_messages_cf, all_messages_time_keyed_cf];

        let db = DB::open_cf_descriptors(&options, db_path, cfs_vec)?;

        Ok(ShinkaiMessageDB { db })
    }

    pub fn insert(&self, key: String, message: &ShinkaiMessage, topic: Topic) -> Result<(), Error> {
        // As protobuf uses bytes to serialize data, we can use this to store into RocksDB
        let message_bytes = ShinkaiMessageHandler::encode_message(message.clone());
        let cf = self.db.cf_handle(topic.as_str()).unwrap();
        self.db.put_cf(cf, key, message_bytes)
    }

    pub fn get(&self, key: String, topic: Topic) -> Result<Option<ShinkaiMessage>, Error> {
        let cf = self.db.cf_handle(topic.as_str()).unwrap();
        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let message = ShinkaiMessageHandler::decode_message(bytes.to_vec()).unwrap();
                Ok(Some(message))
            },
            None => Ok(None)
        }
    }

    pub fn insert_message(&self, message: &ShinkaiMessage) -> Result<(), Error> {
        // Calculate the hash of the message for the key
        let hash_key = ShinkaiMessageHandler::calculate_hash(message);
        
        // Calculate the scheduled time or current time
        let time_key = match message.external_metadata.unwrap().scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => message.external_metadata.unwrap().scheduled_time.clone(),
        };

        // Create a write batch
        let mut batch = rocksdb::WriteBatch::default();

        // Define the data for AllMessages
        let all_messages_cf = self.db.cf_handle(Topic::AllMessages.as_str()).unwrap();
        let message_bytes = ShinkaiMessageHandler::encode_message(message.clone());
        batch.put_cf(all_messages_cf, &hash_key, &message_bytes);

        // Define the data for AllMessagesTimeKeyed
        let all_messages_time_keyed_cf = self.db.cf_handle(Topic::AllMessagesTimeKeyed.as_str()).unwrap();
        batch.put_cf(all_messages_time_keyed_cf, &time_key, &hash_key);

        // Atomically apply the updates
        self.db.write(batch)?;

        Ok(())
    }

    pub fn schedule_message(&self, message: &ShinkaiMessage) -> Result<(), Error> {
        // Calculate the scheduled time or current time
        let time_key = match message.external_metadata.unwrap().scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => message.external_metadata.unwrap().scheduled_time.clone(),
        };

        // Convert ShinkaiMessage into bytes for storage
        let message_bytes = ShinkaiMessageHandler::encode_message(message.clone());

        // Retrieve the handle to the "ToSend" column family
        let to_send_cf = self.db.cf_handle(Topic::ScheduledMessage.as_str()).unwrap();

        // Insert the message into the "ToSend" column family using the time key
        self.db.put_cf(to_send_cf, time_key, message_bytes)?;

        Ok(())
    }
}
