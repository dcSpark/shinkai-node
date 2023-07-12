use rocksdb::{Error, Options};

use crate::{shinkai_message::shinkai_message_handler::ShinkaiMessageHandler, shinkai_message_proto::ShinkaiMessage};

use super::{db::Topic, db_errors::ShinkaiMessageDBError, ShinkaiMessageDB};

#[derive(Debug, PartialEq, PartialOrd)]
pub enum Permission {
    Read,  // it contains None
    Write, // it contains Read
    Admin, // it contains Write
}

impl Permission {
    fn to_i32(&self) -> i32 {
        match self {
            Permission::Read => 1,
            Permission::Write => 2,
            Permission::Admin => 3,
        }
    }

    fn from_i32(val: i32) -> Result<Self, ShinkaiMessageDBError> {
        match val {
            1 => Ok(Permission::Read),
            2 => Ok(Permission::Write),
            3 => Ok(Permission::Admin),
            _ => Err(ShinkaiMessageDBError::SomeError),
        }
    }
}

impl ShinkaiMessageDB {
    pub fn create_inbox(&mut self, inbox_name: String) -> Result<(), Error> {
        println!("Creating inbox: {}", inbox_name);
        // Create Options for ColumnFamily
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);

        // Create ColumnFamilyDescriptors for inbox and permission lists
        let cf_name_inbox = inbox_name.clone();
        let cf_name_global_perms = format!("{}_global_perms", &inbox_name);
        let cf_name_device_perms = format!("{}_device_perms", &inbox_name);
        let cf_name_agent_perms = format!("{}_agent_perms", &inbox_name);
        let cf_name_unread_list = format!("{}_unread_list", &inbox_name);

        // Create column families
        self.db.create_cf(&cf_name_inbox, &cf_opts)?;
        self.db.create_cf(&cf_name_global_perms, &cf_opts)?;
        self.db.create_cf(&cf_name_device_perms, &cf_opts)?;
        self.db.create_cf(&cf_name_agent_perms, &cf_opts)?;
        self.db.create_cf(&cf_name_unread_list, &cf_opts)?;

        // Add inbox name to the list in the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::Inbox.as_str())
            .expect("to be able to access Topic::Inbox");
        self.db.put_cf(cf_inbox, &inbox_name, &inbox_name)?;

        Ok(())
    }

    pub fn insert_message(&mut self, message: &ShinkaiMessage) -> Result<(), ShinkaiMessageDBError> {
        let inbox_name = ShinkaiMessageHandler::get_inbox_name(&message.clone())?;

        // Insert the message
        let _ = self.insert_message_to_all(&message.clone())?;

        // Check if the inbox topic exists and if not create it
        if self.db.cf_handle(&inbox_name).is_none() {
            self.create_inbox(inbox_name.clone())?;
        }

        // Calculate the hash of the message for the key
        let hash_key = ShinkaiMessageHandler::calculate_hash(&message);
        println!("About to insert message with hash key: {}", hash_key);

        // Clone the external_metadata first, then unwrap
        let cloned_external_metadata = message.external_metadata.clone();
        let ext_metadata = cloned_external_metadata.expect("Failed to clone external metadata");

        // Get the scheduled time or calculate current time
        let time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
        };

        // Create the composite key by concatenating the time_key and the hash_key, with a separator
        let composite_key = format!("{}:{}", time_key, hash_key);

        let mut batch = rocksdb::WriteBatch::default();

        // Add the message to the inbox
        let cf_inbox = self
            .db
            .cf_handle(&inbox_name)
            .expect("Failed to get cf handle for inbox");
        batch.put_cf(cf_inbox, &composite_key, &hash_key);

        // Add the message to the unread_list inbox
        let cf_unread_list = self
            .db
            .cf_handle(&format!("{}_unread_list", inbox_name))
            .expect("Failed to get cf handle for unread_list");
        batch.put_cf(cf_unread_list, &composite_key, &hash_key);

        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_last_messages_from_inbox(
        &self,
        inbox_name: String,
        n: usize,
    ) -> Result<Vec<ShinkaiMessage>, ShinkaiMessageDBError> {
        // Fetch the column family for the specified inbox
        let inbox_cf = match self.db.cf_handle(&inbox_name) {
            Some(cf) => cf,
            None => return Err(ShinkaiMessageDBError::InboxNotFound),
        };

        // Fetch the column family for all messages
        let messages_cf = self.db.cf_handle(Topic::AllMessages.as_str()).unwrap();

        // Create an iterator for the specified inbox, starting from the end
        let iter = self.db.iterator_cf(inbox_cf, rocksdb::IteratorMode::End);

        let mut messages = Vec::new();
        for item in iter.take(n) {
            // Handle the Result returned by the iterator
            match item {
                Ok((_, value)) => {
                    // The value of the inbox CF is the key in the AllMessages CF
                    let message_key = value.to_vec();

                    // Fetch the message from the AllMessages CF
                    match self.db.get_cf(messages_cf, &message_key)? {
                        Some(bytes) => {
                            let message = ShinkaiMessageHandler::decode_message(bytes.to_vec())?;
                            messages.push(message);
                        }
                        None => return Err(ShinkaiMessageDBError::MessageNotFound),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(messages)
    }

    pub fn mark_as_read_up_to(&mut self, inbox_name: String, up_to_time: String) -> Result<(), ShinkaiMessageDBError> {
        // Fetch the column family for the specified unread_list
        let cf_name_unread_list = format!("{}_unread_list", inbox_name);
        let unread_list_cf = match self.db.cf_handle(&cf_name_unread_list) {
            Some(cf) => cf,
            None => return Err(ShinkaiMessageDBError::InboxNotFound),
        };

        // Create an iterator for the specified unread_list, starting from the beginning
        let iter = self.db.iterator_cf(unread_list_cf, rocksdb::IteratorMode::Start);

        // Convert up_to_time to &str
        let up_to_time = &up_to_time;

        // Iterate through the unread_list and delete all messages up to the specified time
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, _)) => {
                    let key_str = match String::from_utf8(key.to_vec()) {
                        Ok(s) => s,
                        Err(_) => return Err(ShinkaiMessageDBError::SomeError),
                    };

                    // Split the key_str to separate timestamp and hash
                    let split_key: Vec<&str> = key_str.split(':').collect();

                    if let Some(timestamp_str) = split_key.get(0) {
                        if *timestamp_str <= &**up_to_time {
                            // Delete the message from the unread_list
                            self.db.delete_cf(unread_list_cf, key)?;
                        } else {
                            // We've passed the up_to_time, so we can break the loop
                            break;
                        }
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    pub fn get_last_unread_messages_from_inbox(
        &self,
        inbox_name: String,
        n: usize,
        offset_key: Option<String>,
    ) -> Result<Vec<ShinkaiMessage>, ShinkaiMessageDBError> {
        // Fetch the column family for the specified unread_list
        let cf_name_unread_list = format!("{}_unread_list", inbox_name);
        let unread_list_cf = match self.db.cf_handle(&cf_name_unread_list) {
            Some(cf) => cf,
            None => return Err(ShinkaiMessageDBError::InboxNotFound),
        };

        // Fetch the column family for all messages
        let messages_cf = self.db.cf_handle(Topic::AllMessages.as_str()).unwrap();

        // Create an iterator for the specified unread_list
        let mut iter = match &offset_key {
            Some(offset_key) => self.db.iterator_cf(
                unread_list_cf,
                rocksdb::IteratorMode::From(offset_key.as_bytes(), rocksdb::Direction::Reverse),
            ),
            None => self.db.iterator_cf(unread_list_cf, rocksdb::IteratorMode::End),
        };

        // Skip the first entry if an offset_key was provided and it matches the current key
        if let Some(offset_key) = &offset_key {
            if let Some(Ok((key, _))) = iter.next() {
                let key_str = String::from_utf8_lossy(&key);
                if key_str != *offset_key {
                    // If the key didn't match the offset_key, recreate the iterator to start from the end
                    iter = self.db.iterator_cf(unread_list_cf, rocksdb::IteratorMode::End);
                }
            }
        }

        let mut messages = Vec::new();
        for item in iter.take(n) {
            // Handle the Result returned by the iterator
            match item {
                Ok((_, value)) => {
                    // The value of the unread_list CF is the key in the AllMessages CF
                    let message_key = value.to_vec();

                    // Fetch the message from the AllMessages CF
                    match self.db.get_cf(messages_cf, &message_key)? {
                        Some(bytes) => {
                            let message = ShinkaiMessageHandler::decode_message(bytes.to_vec())?;
                            messages.push(message);
                        }
                        None => return Err(ShinkaiMessageDBError::MessageNotFound),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(messages)
    }

    pub fn add_permission(
        &mut self,
        inbox_name: &str,
        perm_type: &str,
        identity: &str,
        perm: Permission,
    ) -> Result<(), ShinkaiMessageDBError> {
        // Fetch column family for identity
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .ok_or(ShinkaiMessageDBError::IdentityNotFound)?;
        
        // Check if the identity exists
        if self.db.get_cf(cf_identity, identity)?.is_none() {
            return Err(ShinkaiMessageDBError::IdentityNotFound);
        }
    
        // Handle the original permission addition
        let cf_name = format!("{}_{}", inbox_name, perm_type);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiMessageDBError::InboxNotFound)?;
        let perm_val = perm.to_i32().to_string(); // Convert permission to i32 and then to String
        self.db.put_cf(cf, identity, perm_val)?;
        Ok(())
    }

    pub fn remove_permission(
        &mut self,
        inbox_name: &str,
        perm_type: &str,
        identity: &str,
    ) -> Result<(), ShinkaiMessageDBError> {
        // Fetch column family for identity
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .ok_or(ShinkaiMessageDBError::IdentityNotFound)?;
        
        // Check if the identity exists
        if self.db.get_cf(cf_identity, identity)?.is_none() {
            return Err(ShinkaiMessageDBError::IdentityNotFound);
        }
    
        // Handle the original permission removal
        let cf_name = format!("{}_{}", inbox_name, perm_type);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiMessageDBError::InboxNotFound)?;
        self.db.delete_cf(cf, identity)?;
        Ok(())
    }
    
    pub fn has_permission(
        &self,
        inbox_name: &str,
        perm_type: &str,
        identity: &str,
        perm: Permission,
    ) -> Result<bool, ShinkaiMessageDBError> {
        // Fetch column family for identity
        let cf_identity = self
            .db
            .cf_handle(Topic::ProfilesIdentityKey.as_str())
            .ok_or(ShinkaiMessageDBError::IdentityNotFound)?;
    
        // Check if the identity exists
        if self.db.get_cf(cf_identity, identity)?.is_none() {
            return Err(ShinkaiMessageDBError::IdentityNotFound);
        }
    
        // Handle the original permission check
        let cf_name = format!("{}_{}", inbox_name, perm_type);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiMessageDBError::InboxNotFound)?;
        match self.db.get_cf(cf, identity)? {
            Some(val) => {
                let val_str = String::from_utf8(val.to_vec()).map_err(|_| ShinkaiMessageDBError::SomeError)?;
                let val_perm = Permission::from_i32(val_str.parse::<i32>().map_err(|_| ShinkaiMessageDBError::SomeError)?)?;
                Ok(val_perm >= perm)
            }
            None => Ok(false),
        }
    }
}
