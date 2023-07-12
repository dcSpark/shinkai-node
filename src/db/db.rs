use std::collections::HashMap;

use crate::{
    network::subidentities::Subidentity,
    shinkai_message::{
        encryption::{
            encryption_public_key_to_string, encryption_public_key_to_string_ref, string_to_encryption_public_key,
        },
        shinkai_message_handler::ShinkaiMessageHandler,
        signatures::{
            signature_public_key_to_string, signature_public_key_to_string_ref, string_to_signature_public_key,
        },
    },
    shinkai_message_proto::ShinkaiMessage,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use rocksdb::{ColumnFamilyDescriptor, Error, IteratorMode, Options, ReadOptions, DB};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::{db_errors::ShinkaiMessageDBError};

pub enum Topic {
    Inbox,
    Peers,
    ProfilesIdentityKey,
    ProfilesEncryptionKey,
    ScheduledMessage,
    AllMessages,
    AllMessagesTimeKeyed,
    OneTimeRegistrationCodes,
}

impl Topic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Peers => "peers",
            Self::ProfilesIdentityKey => "profiles_identity_key",
            Self::ProfilesEncryptionKey => "profiles_encryption_key",
            Self::ScheduledMessage => "scheduled_message",
            Self::AllMessages => "all_messages",
            Self::AllMessagesTimeKeyed => "all_messages_time_keyed",
            Self::OneTimeRegistrationCodes => "one_time_registration_codes",
        }
    }
}

pub struct ShinkaiMessageDB {
    pub db: DB,
    pub path: String,
}

impl ShinkaiMessageDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let cf_names = vec![
            Topic::Inbox.as_str(),
            Topic::Peers.as_str(),
            Topic::ProfilesEncryptionKey.as_str(),
            Topic::ProfilesIdentityKey.as_str(),
            Topic::ScheduledMessage.as_str(),
            Topic::AllMessages.as_str(),
            Topic::AllMessagesTimeKeyed.as_str(),
            Topic::OneTimeRegistrationCodes.as_str(),
        ];

        let mut cfs = vec![];
        for cf_name in &cf_names {
            // Create Options for ColumnFamily
            let mut cf_opts = Options::default();
            cf_opts.create_if_missing(true);
            cf_opts.create_missing_column_families(true);

            // Create ColumnFamilyDescriptor for each ColumnFamily
            let cf_desc = ColumnFamilyDescriptor::new(cf_name.to_string(), cf_opts);
            cfs.push(cf_desc);
        }

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        let db = DB::open_cf_descriptors(&db_opts, db_path, cfs)?;

        Ok(ShinkaiMessageDB {
            db,
            path: db_path.to_string(),
        })
    }

    pub fn write_to_peers(&self, key: &str, address: &str) -> Result<(), Error> {
        let cf = self.db.cf_handle(Topic::Peers.as_str()).unwrap();
        self.db.put_cf(cf, key, address.as_bytes())
    }

    pub fn get_all_peers(&self) -> Result<Vec<(String, String)>, Error> {
        let cf = self.db.cf_handle(Topic::Peers.as_str()).unwrap();
        let mut result = Vec::new();

        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, value)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();
                    let value_str = String::from_utf8(value.to_vec()).unwrap();
                    result.push((key_str, value_str));
                }
                Err(e) => return Err(e),
            }
        }

        Ok(result)
    }

    pub fn insert_message_to_all(&self, message: &ShinkaiMessage) -> Result<(), Error> {
        // Calculate the hash of the message for the key
        let hash_key = ShinkaiMessageHandler::calculate_hash(&message);

        // Clone the external_metadata first, then unwrap
        let cloned_external_metadata = message.external_metadata.clone();
        let ext_metadata = cloned_external_metadata.unwrap();

        // Calculate the scheduled time or current time
        let time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
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
        let time_key = match message.external_metadata.clone().unwrap().scheduled_time.is_empty() {
            true => ShinkaiMessageHandler::generate_time_now(),
            false => message.external_metadata.clone().unwrap().scheduled_time.clone(),
        };

        // Convert ShinkaiMessage into bytes for storage
        let message_bytes = ShinkaiMessageHandler::encode_message(message.clone());

        // Retrieve the handle to the "ToSend" column family
        let to_send_cf = self.db.cf_handle(Topic::ScheduledMessage.as_str()).unwrap();

        // Insert the message into the "ToSend" column family using the time key
        self.db.put_cf(to_send_cf, time_key, message_bytes)?;

        Ok(())
    }

    pub fn get_scheduled_due_messages(&self) -> Result<Vec<ShinkaiMessage>, ShinkaiMessageDBError> {
        // Generate the current time key
        let current_time_key = ShinkaiMessageHandler::generate_time_now();

        // Retrieve the handle to the "ScheduledMessage" column family
        let scheduled_message_cf = self.db.cf_handle(Topic::ScheduledMessage.as_str()).unwrap();

        // Prepare a ReadOptions to set the iterator upper bound
        let mut read_opts = ReadOptions::default();
        read_opts.set_iterate_upper_bound(current_time_key);

        // Get an iterator over the column family with read options
        let iter = self
            .db
            .iterator_cf_opt(scheduled_message_cf, read_opts, IteratorMode::Start);

        // Collect all messages before current time
        let mut messages = Vec::new();
        for item in iter {
            // Unwrap the Result
            let (key, value) = item.map_err(ShinkaiMessageDBError::from)?;

            // Decode the message
            let message = ShinkaiMessageHandler::decode_message(value.to_vec()).map_err(ShinkaiMessageDBError::from)?;
            messages.push(message);
        }

        Ok(messages)
    }

    pub fn get_last_messages_from_all(&self, n: usize) -> Result<Vec<ShinkaiMessage>, ShinkaiMessageDBError> {
        let time_keyed_cf = self.db.cf_handle(Topic::AllMessagesTimeKeyed.as_str()).unwrap();
        let messages_cf = self.db.cf_handle(Topic::AllMessages.as_str()).unwrap();

        let iter = self.db.iterator_cf(time_keyed_cf, rocksdb::IteratorMode::End);

        let mut messages = Vec::new();
        for item in iter.take(n) {
            // Handle the Result returned by the iterator
            match item {
                Ok((_, value)) => {
                    // The value of the AllMessagesTimeKeyed CF is the key in the AllMessages CF
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
}
