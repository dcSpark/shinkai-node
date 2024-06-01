use super::super::vector_fs_error::VectorFSError;
use crate::db::db_profile_bound::ProfileBoundWriteBatch;
use crate::db::ShinkaiDB;
use rand::Rng;
use rand::{distributions::Alphanumeric, thread_rng};
use rocksdb::{
    AsColumnFamilyRef, ColumnFamily, ColumnFamilyDescriptor, DBCompressionType, IteratorMode, Options, SingleThreaded,
};
use rocksdb::{Error, OptimisticTransactionDB};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum FSTopic {
    VectorResources,
    FileSystem,
    SourceFiles,
    ReadAccessLogs,
    WriteAccessLogs,
    TempFilesInbox,
}

impl FSTopic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VectorResources => "resources",
            Self::FileSystem => "filesystem",
            Self::SourceFiles => "sourcefiles",
            Self::ReadAccessLogs => "readacesslogs",
            Self::WriteAccessLogs => "writeaccesslogs",
            Self::TempFilesInbox => "tempfilesinbox",
        }
    }
}

/// Represents an operation to be performed in a transaction.
#[derive(Debug, Clone)]
pub enum TransactionOperation {
    Write(String, String, Vec<u8>), // Represents a key-value pair to write
    Delete(String, String),         // Represents a key to delete
}

#[derive(Debug)]
pub struct VectorFSDB {
    pub db: OptimisticTransactionDB,
    pub path: String,
}

impl VectorFSDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        // if we want to enable compression
        db_opts.set_compression_type(DBCompressionType::Lz4);

        // More info: https://github.com/facebook/rocksdb/wiki/BlobDB
        db_opts.set_enable_blob_files(true);
        db_opts.set_min_blob_size(1024 * 100); // 100kb
        db_opts.set_keep_log_file_num(10);
        db_opts.set_blob_compression_type(DBCompressionType::Lz4);

        let cf_names = if Path::new(db_path).exists() {
            // If the database file exists, get the list of column families from the database
            OptimisticTransactionDB::<SingleThreaded>::list_cf(&db_opts, db_path)?
        } else {
            // If the database file does not exist, use the default list of column families
            vec![
                FSTopic::VectorResources.as_str().to_string(),
                FSTopic::FileSystem.as_str().to_string(),
                FSTopic::SourceFiles.as_str().to_string(),
                FSTopic::ReadAccessLogs.as_str().to_string(),
                FSTopic::WriteAccessLogs.as_str().to_string(),
                FSTopic::TempFilesInbox.as_str().to_string(),
            ]
        };

        let mut cfs = vec![];
        for cf_name in &cf_names {
            let mut cf_opts = Options::default();
            cf_opts.create_if_missing(true);
            cf_opts.create_missing_column_families(true);
            cf_opts.set_enable_blob_files(true);
            cf_opts.set_min_blob_size(1024 * 100); // 100kb
            cf_opts.set_blob_compression_type(DBCompressionType::Lz4);
            cf_opts.set_keep_log_file_num(10);

            // Set a prefix extractor for the TempFilesInbox column family
            if cf_name == FSTopic::TempFilesInbox.as_str() {
                let prefix_length = 47; // Adjust the prefix length as needed
                let prefix_extractor = rocksdb::SliceTransform::create_fixed_prefix(prefix_length);
                cf_opts.set_prefix_extractor(prefix_extractor);
            }

            let cf_desc = ColumnFamilyDescriptor::new(cf_name.to_string(), cf_opts);
            cfs.push(cf_desc);
        }

        let db = OptimisticTransactionDB::open_cf_descriptors(&db_opts, db_path, cfs)?;
        Ok(Self {
            db,
            path: db_path.to_string(),
        })
    }

    pub fn new_empty() -> Result<Self, Error> {
        let random_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();
        let db_path = format!("db_tests/empty_vector_fs_db_{}", random_string);

        // Set up default options for the database
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true); // Ensure the database is created if it does not exist

        // Open an OptimisticTransactionDB with the specified options
        let db = OptimisticTransactionDB::<SingleThreaded>::open(&db_opts, &db_path)?;

        Ok(Self { db, path: db_path })
    }

    /// Fetches the ColumnFamily handle.
    pub fn get_cf_handle(&self, topic: FSTopic) -> Result<&ColumnFamily, VectorFSError> {
        let handle = self
            .db
            .cf_handle(topic.as_str())
            .ok_or(VectorFSError::FailedFetchingCF)?;
        Ok(handle)
    }

    /// Fetches the value of a KV pair and returns it as a Vector of bytes.
    pub fn get_cf<K: AsRef<[u8]>>(&self, topic: FSTopic, key: K) -> Result<Vec<u8>, VectorFSError> {
        let colfam = self.get_cf_handle(topic)?;
        let bytes = self.db.get_cf(colfam, key)?.ok_or(VectorFSError::FailedFetchingValue)?;
        Ok(bytes)
    }

    /// Fetching the value of a KV pair that is profile-bound, returning it as a Vector of bytes.
    /// In practice this means the profile name is prepended to the supplied key before
    /// performing the fetch.
    pub fn get_cf_pb(&self, topic: FSTopic, key: &str, profile: &ShinkaiName) -> Result<Vec<u8>, VectorFSError> {
        let new_key = Self::generate_profile_bound_key(key, profile)?;
        self.get_cf(topic, new_key)
    }

    /// Iterates over the provided column family
    pub fn iterator_cf<'a>(
        &'a self,
        cf_name: &str,
    ) -> Result<impl Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), VectorFSError>> + 'a, VectorFSError> {
        let cf_handle = self.db.cf_handle(cf_name).ok_or(VectorFSError::FailedFetchingCF)?;
        let iterator = self.db.iterator_cf(cf_handle, IteratorMode::Start);

        // Create a new iterator that maps over the original iterator, converting any RocksDB errors into VectorFSError
        let mapped_iterator = iterator.map(|result| result.map_err(VectorFSError::from));

        Ok(mapped_iterator)
    }

    /// Iterates over the provided column family profile-bounded, meaning that
    /// we filter out all keys in the iterator which are not profile-bounded to the
    /// correct profile, before returning the iterator.
    pub fn iterator_cf_pb<'a>(
        &'a self,
        cf: &impl AsColumnFamilyRef,
        profile: &ShinkaiName,
    ) -> Result<impl Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>> + 'a, VectorFSError> {
        let profile_prefix = Self::get_profile_name_string(profile)?.into_bytes();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let filtered_iter = iter.filter(move |result| match result {
            Ok((key, _)) => key.starts_with(&profile_prefix),
            Err(_) => false,
        });
        Ok(filtered_iter)
    }

    /// Saves the value inside of the key at the provided column family
    pub fn put_cf<K, V>(&self, cf_name: &str, key: K, value: V) -> Result<(), VectorFSError>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let cf_handle = self.db.cf_handle(cf_name).ok_or(VectorFSError::FailedFetchingCF)?;
        let txn = self.db.transaction();
        // Use the column family reference directly
        txn.put_cf(cf_handle, key.as_ref(), value.as_ref())
            .map_err(VectorFSError::from)?;
        // Attempt to commit the transaction
        txn.commit().map_err(VectorFSError::from)?;
        Ok(())
    }

    /// Saves the value inside of the key (profile-bound) at the provided column family.
    pub fn put_cf_pb<V>(&self, cf_name: &str, key: &str, value: V, profile: &ShinkaiName) -> Result<(), VectorFSError>
    where
        V: AsRef<[u8]>,
    {
        let new_key = Self::generate_profile_bound_key(key, profile)?;
        self.put_cf(cf_name, new_key, value) // Ensure new_key is passed as a reference if required
    }

    /// Deletes the key from the provided column family
    pub fn delete_cf<K>(&self, cf_name: &str, key: K) -> Result<(), VectorFSError>
    where
        K: AsRef<[u8]>,
    {
        let cf_handle = self.db.cf_handle(cf_name).ok_or(VectorFSError::FailedFetchingCF)?;
        let txn = self.db.transaction();
        txn.delete_cf(cf_handle, key.as_ref()).map_err(VectorFSError::from)?;
        txn.commit().map_err(VectorFSError::from)?;
        Ok(())
    }

    /// Deletes the key (profile-bound) from the provided column family.
    pub fn delete_cf_pb(&self, cf: &str, key: &str, profile: &ShinkaiName) -> Result<(), VectorFSError> {
        let new_key = Self::generate_profile_bound_key(key, profile)?;
        self.delete_cf(cf, new_key)
    }

    /// Fetches the ColumnFamily handle.
    pub fn cf_handle(&self, name: &str) -> Result<&ColumnFamily, VectorFSError> {
        self.db.cf_handle(name).ok_or(VectorFSError::FailedFetchingCF)
    }

    /// Commits a series of operations as a single transaction.
    pub fn commit_operations(&self, operations: Vec<TransactionOperation>) -> Result<(), VectorFSError> {
        let txn = self.db.transaction();

        for op in operations.clone() {
            match op {
                TransactionOperation::Write(cf_name, key, value) => {
                    let cf_handle = self.db.cf_handle(&cf_name).ok_or(VectorFSError::FailedFetchingCF)?;
                    txn.put_cf(cf_handle, key.as_bytes(), &value)
                        .map_err(VectorFSError::from)?;
                }
                TransactionOperation::Delete(cf_name, key) => {
                    let cf_handle = self.db.cf_handle(&cf_name).ok_or(VectorFSError::FailedFetchingCF)?;
                    txn.delete_cf(cf_handle, key.as_bytes()).map_err(VectorFSError::from)?;
                }
                _ => {
                    eprintln!("Unsupported operation: {:?}", op);
                }
            }
        }

        let result = txn.commit();
        result.map_err(VectorFSError::from)?;

        Ok(())
    }

    /// Profile-bound saves the WriteBatch to the database
    pub fn write_pb(&self, pb_batch: ProfileBoundWriteBatch) -> Result<(), VectorFSError> {
        let operations: Vec<TransactionOperation> = pb_batch.operations;

        // Now, use the commit_operations method to commit these operations as a single transaction
        self.commit_operations(operations)
    }

    /// Validates if the key has the provided profile name properly prepended to it
    pub fn validate_profile_bound_key(key: &str, profile: &ShinkaiName) -> Result<bool, VectorFSError> {
        let profile_name = Self::get_profile_name_string(profile)?;
        Ok(key.starts_with(&profile_name))
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key(key: &str, profile: &ShinkaiName) -> Result<String, VectorFSError> {
        let prof_name = Self::get_profile_name_string(profile)?;
        Ok(Self::generate_profile_bound_key_from_str(key, &prof_name))
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key_from_str(key: &str, profile_name: &str) -> String {
        ShinkaiDB::generate_profile_bound_key_from_str(key, profile_name)
    }

    /// Extracts the profile name with VectorFSError wrapping
    pub fn get_profile_name_string(profile: &ShinkaiName) -> Result<String, VectorFSError> {
        profile
            .get_profile_name_string()
            .ok_or(VectorFSError::ShinkaiNameLacksProfile)
    }

    /// Debugging method to print all keys, their values' lengths, and their column families across all columns.
    pub fn debug_print_all_columns(&self) -> Result<(), VectorFSError> {
        let topics = [
            FSTopic::VectorResources,
            FSTopic::FileSystem,
            FSTopic::SourceFiles,
            FSTopic::ReadAccessLogs,
            FSTopic::WriteAccessLogs,
        ];

        for topic in topics.clone().iter() {
            let cf_handle = self.get_cf_handle(topic.clone())?;
            let iterator = self.db.iterator_cf(cf_handle, IteratorMode::Start);
            eprintln!("Iterating over keys in the {:?} column family:", topic.as_str());
            for item in iterator {
                match item {
                    Ok((key, value)) => {
                        eprintln!(
                            "Column: {:?}, Key: {:?}, Value Length: {}",
                            topic.as_str(),
                            String::from_utf8_lossy(&key),
                            value.len()
                        );
                    }
                    Err(e) => eprintln!("Error reading from iterator: {:?}", e),
                }
            }
        }
        Ok(())
    }
}
