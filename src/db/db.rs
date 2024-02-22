use super::db_errors::ShinkaiDBError;
use crate::network::ws_manager::{WSUpdateHandler, WebSocketManager};
use crate::vector_fs::vector_fs_error::VectorFSError;
use chrono::{DateTime, Utc};
use rocksdb::{
    AsColumnFamilyRef, ColumnFamily, ColumnFamilyDescriptor, DBCommon, DBIteratorWithThreadMode, Error, IteratorMode,
    Options, SingleThreaded, WriteBatch, DB,
};
use shinkai_message_primitives::{
    schemas::{shinkai_name::ShinkaiName, shinkai_time::ShinkaiStringTime},
    shinkai_message::shinkai_message::ShinkaiMessage,
};
use std::fmt;
use std::{path::Path, sync::Arc};
use tokio::sync::Mutex;

pub enum Topic {
    Inbox,
    Peers,
    ProfilesIdentityKey,
    ProfilesEncryptionKey,
    DevicesIdentityKey,
    DevicesEncryptionKey,
    DevicesPermissions,
    ScheduledMessage,
    AllMessages,
    AllMessagesTimeKeyed,
    OneTimeRegistrationCodes,
    // Links a specific Profile with its device type (global, device, agent)
    ProfilesIdentityType,
    ProfilesPermission,
    ExternalNodeIdentityKey,
    ExternalNodeEncryptionKey,
    AllJobsTimeKeyed,
    VectorResources,
    Agents,
    Toolkits,
    MessagesToRetry,
    MessageBoxSymmetricKeys,
    MessageBoxSymmetricKeysTimes,
    TempFilesInbox,
    JobQueues,
    CronQueues,
    InternalComms,
}

impl Topic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Peers => "peers",
            Self::ProfilesIdentityKey => "profiles_identity_key",
            Self::ProfilesEncryptionKey => "profiles_encryption_key",
            Self::DevicesIdentityKey => "devices_identity_key",
            Self::DevicesEncryptionKey => "devices_encryption_key",
            Self::DevicesPermissions => "devices_permissions",
            Self::ScheduledMessage => "scheduled_message",
            Self::AllMessages => "all_messages",
            Self::AllMessagesTimeKeyed => "all_messages_time_keyed",
            Self::OneTimeRegistrationCodes => "one_time_registration_codes",
            Self::ProfilesIdentityType => "profiles_identity_type",
            Self::ProfilesPermission => "profiles_permission",
            Self::ExternalNodeIdentityKey => "external_node_identity_key",
            Self::ExternalNodeEncryptionKey => "external_node_encryption_key",
            Self::AllJobsTimeKeyed => "all_jobs_time_keyed",
            Self::VectorResources => "resources",
            Self::Agents => "agents",
            Self::Toolkits => "toolkits",
            Self::MessagesToRetry => "messages_to_retry",
            Self::MessageBoxSymmetricKeys => "message_box_symmetric_keys",
            Self::MessageBoxSymmetricKeysTimes => "message_box_symmetric_keys_times",
            Self::TempFilesInbox => "temp_files_inbox",
            Self::JobQueues => "job_queues",
            Self::CronQueues => "cron_queues",
            Self::InternalComms => "internal_comms",
        }
    }
}

impl fmt::Debug for ShinkaiDB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShinkaiDB")
            .field("db", &self.db)
            .field("path", &self.path)
            // You can decide what you want to print for ws_manager
            .field("ws_manager", &"WSUpdateHandler implementation")
            .finish()
    }
}

pub struct ShinkaiDB {
    pub db: DB,
    pub path: String,
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
}

impl ShinkaiDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        // if we want to enable compression
        // db_opts.set_compression_type(DBCompressionType::Lz4);

        let cf_names = if Path::new(db_path).exists() {
            // If the database file exists, get the list of column families from the database
            DB::list_cf(&db_opts, db_path)?
        } else {
            // If the database file does not exist, use the default list of column families
            vec![
                Topic::Inbox.as_str().to_string(),
                Topic::Peers.as_str().to_string(),
                Topic::ProfilesEncryptionKey.as_str().to_string(),
                Topic::ProfilesIdentityKey.as_str().to_string(),
                Topic::DevicesEncryptionKey.as_str().to_string(),
                Topic::DevicesIdentityKey.as_str().to_string(),
                Topic::DevicesPermissions.as_str().to_string(),
                Topic::ScheduledMessage.as_str().to_string(),
                Topic::AllMessages.as_str().to_string(),
                Topic::AllMessagesTimeKeyed.as_str().to_string(),
                Topic::OneTimeRegistrationCodes.as_str().to_string(),
                Topic::ProfilesIdentityType.as_str().to_string(),
                Topic::ProfilesPermission.as_str().to_string(),
                Topic::ExternalNodeIdentityKey.as_str().to_string(),
                Topic::ExternalNodeEncryptionKey.as_str().to_string(),
                Topic::AllJobsTimeKeyed.as_str().to_string(),
                Topic::VectorResources.as_str().to_string(),
                Topic::Agents.as_str().to_string(),
                Topic::Toolkits.as_str().to_string(),
                Topic::MessagesToRetry.as_str().to_string(),
                Topic::MessageBoxSymmetricKeys.as_str().to_string(),
                Topic::MessageBoxSymmetricKeysTimes.as_str().to_string(),
                Topic::TempFilesInbox.as_str().to_string(),
                Topic::JobQueues.as_str().to_string(),
                Topic::CronQueues.as_str().to_string(),
                Topic::InternalComms.as_str().to_string(),
            ]
        };

        let mut cfs = vec![];
        for cf_name in &cf_names {
            let mut cf_opts = Options::default();
            cf_opts.create_if_missing(true);
            cf_opts.create_missing_column_families(true);

            let cf_desc = ColumnFamilyDescriptor::new(cf_name.to_string(), cf_opts);
            cfs.push(cf_desc);
        }

        let db = DB::open_cf_descriptors(&db_opts, db_path, cfs)?;

        Ok(ShinkaiDB {
            db,
            path: db_path.to_string(),
            ws_manager: None,
        })
    }

    /// Required for intra-communications between node UI and node
    pub fn read_needs_reset(&self) -> Result<bool, Error> {
        let cf = self.get_cf_handle(Topic::InternalComms).unwrap();
        match self.db.get_cf(cf, b"needs_reset") {
            Ok(Some(value)) => Ok(value == b"true"),
            Ok(None) => Ok(false),
            Err(e) => {
                eprintln!("Error reading needs_reset: {:?}", e);
                Err(e)
            },
        }
    }

    /// Required for intra-communications between node UI and node
    pub fn reset_needs_reset(&self) -> Result<(), Error> {
        let cf = self.get_cf_handle(Topic::InternalComms).unwrap();
        self.db.put_cf(cf, b"needs_reset", b"false")
    }

    /// Sets the needs_reset value to true
    pub fn set_needs_reset(&self) -> Result<(), Error> {
        let cf = self.get_cf_handle(Topic::InternalComms).unwrap();
        self.db.put_cf(cf, b"needs_reset", b"true")
    }

    pub fn set_ws_manager(&mut self, ws_manager: Arc<Mutex<dyn WSUpdateHandler + Send>>) {
        self.ws_manager = Some(ws_manager);
    }

    /// Extracts the profile name with ShinkaiDBError wrapping
    pub fn get_profile_name(profile: &ShinkaiName) -> Result<String, ShinkaiDBError> {
        profile
            .get_profile_name()
            .ok_or(ShinkaiDBError::ShinkaiNameLacksProfile)
    }

    pub fn insert_peer(&self, key: &str, address: &str) -> Result<(), Error> {
        let cf = self.get_cf_handle(Topic::Peers).unwrap();
        self.db.put_cf(cf, key, address.as_bytes())
    }

    /// Fetches all peers from the Peers topic
    pub fn get_peers(&self) -> Result<Vec<(String, String)>, Error> {
        let cf = self.get_cf_handle(Topic::Peers).unwrap();
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

    // We are using a composite_key to avoid the problem that two messages could had
    // been generated at the same time adding the hash of the message to the
    // key, we can ensure that the key is unique the key is composed by the time
    // the message was generated and the hash of the message so the key is in
    // the format: "20230702T20533481346:hash" we could have an empty value for
    // the key, but we are currently using the hash that could be extracted from the
    // message maybe this saves parsing time for a big quantity of messages
    // (maybe)
    pub fn insert_message_to_all(&self, message: &ShinkaiMessage) -> Result<(), ShinkaiDBError> {
        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash_for_pagination();

        // Clone the external_metadata first, then unwrap
        let cloned_external_metadata = message.external_metadata.clone();
        let ext_metadata = cloned_external_metadata;

        // Calculate the scheduled time or current time
        let time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiStringTime::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
        };

        // Create a composite key by concatenating the time_key and the hash_key, with a
        // separator
        let composite_key = format!("{}:::{}", time_key, hash_key);

        // Create a write batch
        let mut batch = rocksdb::WriteBatch::default();

        // Define the data for AllMessages
        let all_messages_cf = self.get_cf_handle(Topic::AllMessages).unwrap();
        let message_bytes = match message.encode_message() {
            Ok(bytes) => bytes,
            Err(e) => {
                println!("Error encoding message: {:?}", e);
                return Err(ShinkaiDBError::MessageEncodingError(format!(
                    "Error encoding message: {:?}",
                    e
                )));
            }
        };
        batch.put_cf(all_messages_cf, &hash_key, &message_bytes);

        // Define the data for AllMessagesTimeKeyed
        let all_messages_time_keyed_cf = self.get_cf_handle(Topic::AllMessagesTimeKeyed).unwrap();
        batch.put_cf(all_messages_time_keyed_cf, &composite_key, &hash_key);

        // Atomically apply the updates
        self.db.write(batch)?;

        Ok(())
    }

    pub fn schedule_message(&self, message: &ShinkaiMessage) -> Result<(), ShinkaiDBError> {
        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash_for_pagination();

        // Calculate the scheduled time or current time
        let time_key = match message.external_metadata.clone().scheduled_time.is_empty() {
            true => ShinkaiStringTime::generate_time_now(),
            false => message.external_metadata.clone().scheduled_time.clone(),
        };

        // Create a composite key by concatenating the time_key and the hash_key, with a
        // separator
        let composite_key = format!("{}:::{}", time_key, hash_key);

        // Convert ShinkaiMessage into bytes for storage
        let message_bytes = message.encode_message()?;

        // Retrieve the handle to the "ToSend" column family
        let to_send_cf = self.get_cf_handle(Topic::ScheduledMessage).unwrap();

        // Insert the message into the "ToSend" column family using the composite key
        self.db.put_cf(to_send_cf, composite_key, message_bytes)?;

        Ok(())
    }

    // Format: "2023-07-02T20:53:33Z" or
    // Utc::now().format("%Y-%m-%dT%H:%M:%S.%").to_string();
    // Check out ShinkaiMessageHandler::generate_time_now() for more details.
    // Note: If you pass just a date like "2023-07-02" without the time component,
    // then the function would interpret this as "2023-07-02T00:00:00.000Z", i.e., the
    // start of the day.
    pub fn get_due_scheduled_messages(&self, up_to_time: String) -> Result<Vec<ShinkaiMessage>, ShinkaiDBError> {
        // Retrieve the handle to the "ScheduledMessage" column family
        let scheduled_message_cf = self.get_cf_handle(Topic::ScheduledMessage).unwrap();

        // Get an iterator over the column family from the start
        let iter = self.db.iterator_cf(scheduled_message_cf, IteratorMode::Start);

        // Parse up_to_time into a DateTime object
        let up_to_time = DateTime::parse_from_rfc3339(&up_to_time)
            .map_err(|_| ShinkaiDBError::InvalidData)?
            .with_timezone(&Utc);

        // Collect all messages before the up_to_time
        let mut messages = Vec::new();
        for item in iter {
            // Unwrap the Result
            let (key, value) = item.map_err(ShinkaiDBError::from)?;

            // Convert the Vec<u8> key into a string
            let key_str = std::str::from_utf8(&key).map_err(|_| ShinkaiDBError::InvalidData)?;

            // Split the composite key to get the time component
            let time_key_str = key_str.split(":::").next().ok_or(ShinkaiDBError::InvalidData)?;

            // Parse the time_key into a DateTime object
            let time_key = DateTime::parse_from_rfc3339(time_key_str)
                .map_err(|_| ShinkaiDBError::InvalidData)?
                .with_timezone(&Utc);

            // Compare the time key with the up_to_time
            if time_key > up_to_time {
                // Break the loop if we've started seeing messages scheduled for later
                break;
            }

            // Decode the message
            let message = ShinkaiMessage::decode_message_result(value.to_vec())?;
            messages.push(message);
        }

        Ok(messages)
    }

    pub fn get_last_messages_from_all(&self, n: usize) -> Result<Vec<ShinkaiMessage>, ShinkaiDBError> {
        let time_keyed_cf = self.get_cf_handle(Topic::AllMessagesTimeKeyed).unwrap();
        let messages_cf = self.get_cf_handle(Topic::AllMessages).unwrap();

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
                            let message = ShinkaiMessage::decode_message_result(bytes)?;
                            messages.push(message);
                        }
                        None => return Err(ShinkaiDBError::MessageNotFound),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(messages)
    }
}
