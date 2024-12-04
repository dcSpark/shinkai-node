use super::super::vector_fs_error::VectorFSError;
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

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key(key: &str, profile: &ShinkaiName) -> Result<String, VectorFSError> {
        let prof_name = Self::get_profile_name_string(profile)?;
        Ok(Self::generate_profile_bound_key_from_str(key, &prof_name))
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key_from_str(key: &str, profile_name: &str) -> String {
        let mut prof_name = profile_name.to_string() + ":";
        prof_name.push_str(key);
        prof_name
    }

    /// Extracts the profile name with VectorFSError wrapping
    pub fn get_profile_name_string(profile: &ShinkaiName) -> Result<String, VectorFSError> {
        profile
            .get_profile_name_string()
            .ok_or(VectorFSError::ShinkaiNameLacksProfile)
    }
}

/// A struct that offers a profile-bounded interface for write operations.
/// All keys are prefixed with the profile name.
pub struct ProfileBoundWriteBatch {
    pub operations: Vec<TransactionOperation>,
    pub profile_name: String,
}

impl ProfileBoundWriteBatch {
    /// Create a new ProfileBoundWriteBatch with ShinkaiDBError wrapping
    pub fn new(profile: &ShinkaiName) -> Result<Self, VectorFSError> {
        // Also validates that the name includes a profile
        let profile_name = Self::get_profile_name_string(profile)?;
        // Create write batch
        let operations = Vec::new();
        Ok(Self {
            profile_name,
            operations,
        })
    }

    /// Create a new ProfileBoundWriteBatch with VectorFSError wrapping
    pub fn new_vfs_batch(profile: &ShinkaiName) -> Result<Self, VectorFSError> {
        // Also validates that the name includes a profile
        match Self::get_profile_name_string(profile) {
            Ok(profile_name) => {
                Ok(Self {
                    operations: Vec::new(), // Initialize the operations vector
                    profile_name,
                })
            }
            Err(e) => Err(VectorFSError::FailedCreatingProfileBoundWriteBatch(e.to_string())),
        }
    }

    /// Extracts the profile name with ShinkaiDBError wrapping
    pub fn get_profile_name_string(profile: &ShinkaiName) -> Result<String, VectorFSError> {
        profile
            .get_profile_name_string()
            .ok_or(VectorFSError::ShinkaiNameLacksProfile)
    }

    /// Saves the value inside of the key (profile-bound) at the provided column family.
    pub fn pb_put_cf<V>(&mut self, cf_name: &str, key: &str, value: V)
    where
        V: AsRef<[u8]>,
    {
        let new_key = self.gen_pb_key(key);
        self.operations.push(TransactionOperation::Write(
            cf_name.to_string(),
            new_key,
            value.as_ref().to_vec(),
        ));
    }

    /// Removes the value inside of the key (profile-bound) at the provided column family.
    pub fn pb_delete_cf(&mut self, cf_name: &str, key: &str) {
        let new_key = self.gen_pb_key(key);
        self.operations
            .push(TransactionOperation::Delete(cf_name.to_string(), new_key));
    }

    /// Given an input key, generates the profile bound key using the internal profile.
    pub fn gen_pb_key(&self, key: &str) -> String {
        Self::generate_profile_bound_key_from_str(key, &self.profile_name)
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key_from_str(key: &str, profile_name: &str) -> String {
        let mut prof_name = profile_name.to_string() + ":";
        prof_name.push_str(key);
        prof_name
    }
}
