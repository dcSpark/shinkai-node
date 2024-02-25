use super::db_errors::ShinkaiDBError;
use crate::network::ws_manager::{WSUpdateHandler, WebSocketManager};
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
use std::time::Instant;
use std::{path::Path, sync::Arc};
use tokio::sync::Mutex;

pub enum Topic {
    Inbox,
    ScheduledMessage,
    AllMessages,
    Toolkits,
    MessagesToRetry,
    TempFilesInbox,
    JobQueues,
    CronQueues,
    NodeAndUsers,
    MessageBoxSymmetricKeys,
}

impl Topic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::ScheduledMessage => "scheduled_message",
            Self::AllMessages => "all_messages",
            Self::Toolkits => "toolkits",
            Self::MessagesToRetry => "messages_to_retry",
            Self::TempFilesInbox => "temp_files_inbox",
            Self::JobQueues => "job_queues",
            Self::CronQueues => "cron_queues",
            Self::NodeAndUsers => "node_and_users",
            Self::MessageBoxSymmetricKeys => "message_box_symmetric_keys",
        }
    }
}

impl fmt::Debug for ShinkaiDB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShinkaiDB")
            .field("db", &self.db)
            .field("path", &self.path)
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
        let start = Instant::now();
        let db_opts = Self::create_cf_options(None);

        let cf_names = if Path::new(db_path).exists() {
            // If the database file exists, get the list of column families from the database
            DB::list_cf(&db_opts, db_path)?
        } else {
            // If the database file does not exist, use the default list of column families
            vec![
                Topic::Inbox.as_str().to_string(), // Merge with Jobs (!)
                // Topic::ProfilesEncryptionKey.as_str().to_string(), // Merge to NodeAndUsers
                // Topic::ProfilesIdentityKey.as_str().to_string(), // Merge to NodeAndUsers
                // Topic::DevicesEncryptionKey.as_str().to_string(), // Merge to NodeAndUsers
                // Topic::DevicesIdentityKey.as_str().to_string(), // Merge to NodeAndUsers
                // Topic::DevicesPermissions.as_str().to_string(), // Merge to NodeAndUsers
                Topic::ScheduledMessage.as_str().to_string(), // Merge with MessagesToRetry
                Topic::AllMessages.as_str().to_string(),
                // Topic::AllMessagesTimeKeyed.as_str().to_string(), // Merge with AllMessages
                // Topic::OneTimeRegistrationCodes.as_str().to_string(), // Merge with NodeAndUsers
                // Topic::ProfilesIdentityType.as_str().to_string(), // Merge with NodeAndUsers
                // Topic::ProfilesPermission.as_str().to_string(), // Merge with NodeAndUsers
                // Topic::ExternalNodeIdentityKey.as_str().to_string(), // Merge with NodeAndUsers
                // Topic::ExternalNodeEncryptionKey.as_str().to_string(), // Merge with NodeAndUsers
                // Topic::Agents.as_str().to_string(), // Merge with NodeAndUsers
                Topic::Toolkits.as_str().to_string(),
                Topic::MessageBoxSymmetricKeys.as_str().to_string(),
                // Topic::MessageBoxSymmetricKeysTimes.as_str().to_string(), // Mege with MessageBoxSymmetricKeys
                // Topic::InternalComms.as_str().to_string(), // Merge with NodeAndUsers
                // Filtered
                Topic::MessagesToRetry.as_str().to_string(),
                Topic::TempFilesInbox.as_str().to_string(),
                Topic::JobQueues.as_str().to_string(),
                Topic::CronQueues.as_str().to_string(),
                Topic::NodeAndUsers.as_str().to_string(),
            ]
        };

        let mut cfs = vec![];
        for cf_name in &cf_names {
            let prefix_length = match cf_name.as_str() {
                "inbox" => Some(47), // Specify 47 bytes for the Inbox CF
                _ => None, // No prefix extractor for other CFs
            };
            let db_opts = Self::create_cf_options(prefix_length);
            let cf_desc = ColumnFamilyDescriptor::new(cf_name.to_string(), db_opts);
            cfs.push(cf_desc);
        }

        let db = DB::open_cf_descriptors(&db_opts, db_path, cfs)?;
        let elapsed = start.elapsed(); // Check how much time has elapsed
        println!("### RocksDB loaded in: {:?}", elapsed); // Print or log the elapsed time

        let shinkai_db = ShinkaiDB {
            db,
            path: db_path.to_string(),
            ws_manager: None,
        };

        // let result = shinkai_db.compact_db();
        // eprintln!("Compact DB result: {:?}", result);

        // Assuming db_opts is configured and used to open the database
        let stats = db_opts.get_statistics().expect("Statistics should be enabled");

        // After opening the database
        // println!("RocksDB stats: {}", stats);

        Ok(shinkai_db)
    }

    pub fn create_cf_options(prefix_length: Option<usize>) -> Options {
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);

        cf_opts.set_allow_concurrent_memtable_write(true);
        cf_opts.set_enable_write_thread_adaptive_yield(true);
        cf_opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        cf_opts.set_max_write_buffer_number(3);
        cf_opts.set_min_write_buffer_number_to_merge(1);
        cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        cf_opts.increase_parallelism(std::cmp::max(1, num_cpus::get() as i32 / 2));
        cf_opts.enable_statistics();

        let mut block_based_options = rocksdb::BlockBasedOptions::default();
        let cache_size = 64 * 1024 * 1024; // 64 MB for Block Cache
        let block_cache = rocksdb::Cache::new_lru_cache(cache_size);
        block_based_options.set_block_cache(&block_cache);
        block_based_options.set_bloom_filter(10.0, true);
        cf_opts.set_block_based_table_factory(&block_based_options);

        if let Some(length) = prefix_length {
            // Set the prefix_extractor for a fixed prefix length
            let prefix_extractor = rocksdb::SliceTransform::create_fixed_prefix(length);
            cf_opts.set_prefix_extractor(prefix_extractor);
        }

        cf_opts
    }

    pub fn compact_db(&self) -> Result<(), Error> {
        self.db.compact_range(None::<&[u8]>, None::<&[u8]>); // Compact the entire database
        Ok(())
    }

    /// Required for intra-communications between node UI and node
    pub fn read_needs_reset(&self) -> Result<bool, Error> {
        let cf = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        match self.db.get_cf(cf, b"needs_reset") {
            Ok(Some(value)) => Ok(value == b"true"),
            Ok(None) => Ok(false),
            Err(e) => {
                eprintln!("Error reading needs_reset: {:?}", e);
                Err(e)
            }
        }
    }

    /// Required for intra-communications between node UI and node
    pub fn reset_needs_reset(&self) -> Result<(), Error> {
        let cf = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
        self.db.put_cf(cf, b"needs_reset", b"false")
    }

    /// Sets the needs_reset value to true
    pub fn set_needs_reset(&self) -> Result<(), Error> {
        let cf = self.get_cf_handle(Topic::NodeAndUsers).unwrap();
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
        eprintln!("insert_message_to_all> hash_key: {:?}", hash_key);

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

        // Instead of using Topic::AllMessagesTimeKeyed, use Topic::AllMessages with a prefix
        let all_messages_time_keyed_key = format!("all_messages_time_keyed_{}", composite_key);
        batch.put_cf(all_messages_cf, all_messages_time_keyed_key.as_bytes(), &hash_key);

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
        let messages_cf = self.get_cf_handle(Topic::AllMessages).unwrap();

        // Use a prefix search for keys starting with "all_messages_time_keyed_"
        let prefix = "all_messages_time_keyed_";
        let iter = self.db.prefix_iterator_cf(messages_cf, prefix);

        let mut messages = Vec::new();
        for item in iter.take(n) {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, value)) => {
                    // The value is the hash key used in the AllMessages CF
                    let message_key = value.to_vec();

                    // Fetch the message from the AllMessages CF using the hash key
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
