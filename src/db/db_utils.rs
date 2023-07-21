use rocksdb::IteratorMode;
use rocksdb::{Error, Options};

use crate::{
    shinkai_message::shinkai_message_handler::ShinkaiMessageHandler,
    shinkai_message_proto::ShinkaiMessage,
};

use super::{db::Topic, db_errors::ShinkaiMessageDBError, ShinkaiMessageDB};

impl ShinkaiMessageDB {
    #[cfg(debug_assertions)]
    pub fn print_all_from_cf(&self, cf_name: &str) -> Result<(), ShinkaiMessageDBError> {
        println!("printing all for {}", cf_name);
        // Fetch column family handle
        let cf = self.db.cf_handle(cf_name).ok_or(ShinkaiMessageDBError::InboxNotFound)?;

        // Create an iterator for the column family
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        // A boolean flag to check if the bucket is empty
        let mut is_empty = true;

        // Iterate over all entries in the column family
        for item in iter {
            match item {
                Ok((key, value)) => {
                    // Convert Vec<u8> to String
                    let key_str = std::str::from_utf8(&key).unwrap_or("Invalid UTF-8 sequence");
                    let value_str = std::str::from_utf8(&value).unwrap_or("Invalid UTF-8 sequence");

                    println!("print_all_from_cf > Key: {}, Value: {}", key_str, value_str);

                    // If we have at least one item, the bucket is not empty
                    is_empty = false;
                }
                Err(e) => println!("Error reading from column family: {}", e),
            }
        }

        // If the bucket is empty, print a message
        if is_empty {
            println!("print_all_from_cf {}: empty bucket", cf_name);
        }
        Ok(())
    }
}
