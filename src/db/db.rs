use rocksdb::{Options, DB, Error};
use crate::shinkai_message_proto::ShinkaiMessage;
use std::convert::TryInto;

pub struct ShinkaiMessageDB {
    db: DB,
}

impl ShinkaiMessageDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let mut options = Options::default();
        options.create_if_missing(true);
        let db = DB::open(&options, db_path)?;

        Ok(ShinkaiMessageDB { db })
    }

    pub fn insert(&self, key: String, message: &ShinkaiMessage) -> Result<(), Error> {
        // As protobuf uses bytes to serialize data, we can use this to store into RocksDB
        let message_bytes = message.write_to_bytes().unwrap();
        self.db.put(key, message_bytes)
    }

    pub fn get(&self, key: String) -> Result<Option<ShinkaiMessage>, Error> {
        match self.db.get(key)? {
            Some(bytes) => {
                let message = ShinkaiMessage::parse_from_bytes(&bytes).unwrap();
                Ok(Some(message))
            },
            None => Ok(None)
        }
    }
}
