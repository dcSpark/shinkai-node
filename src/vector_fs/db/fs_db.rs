use super::super::{vector_fs_error::VectorFSError, vector_fs_internals::VectorFSInternals};
use crate::db::db::ProfileBoundWriteBatch;
use rand::Rng;
use rand::{distributions::Alphanumeric, thread_rng};
use rocksdb::{
    AsColumnFamilyRef, ColumnFamily, ColumnFamilyDescriptor, DBCommon, DBCompressionType, DBIteratorWithThreadMode,
    Error, IteratorMode, Options, SingleThreaded, WriteBatch, DB,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use std::path::Path;

pub enum FSTopic {
    VectorResources,
    FileSystem,
    SourceFiles,
    ReadAccessLogs,
    WriteAccessLogs,
}

impl FSTopic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VectorResources => "resources",
            Self::FileSystem => "filesystem",
            Self::SourceFiles => "sourcefiles",
            Self::ReadAccessLogs => "readacesslogs",
            Self::WriteAccessLogs => "writeaccesslogs",
        }
    }
}

pub struct VectorFSDB {
    pub db: DB,
    pub path: String,
}

impl VectorFSDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        // if we want to enable compression
        db_opts.set_compression_type(DBCompressionType::Lz4);

        let cf_names = if Path::new(db_path).exists() {
            // If the database file exists, get the list of column families from the database
            DB::list_cf(&db_opts, db_path)?
        } else {
            // If the database file does not exist, use the default list of column families
            vec![
                FSTopic::VectorResources.as_str().to_string(),
                FSTopic::FileSystem.as_str().to_string(),
                FSTopic::SourceFiles.as_str().to_string(),
                FSTopic::ReadAccessLogs.as_str().to_string(),
                FSTopic::WriteAccessLogs.as_str().to_string(),
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
        Ok(Self {
            db,
            path: db_path.to_string(),
        })
    }

    /// Creates a new empty DB, intended only to be used as a mock for tests
    pub fn new_empty() -> Self {
        let random_string: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();
        let db_path = format!("db_tests/empty_vec_fs_db_{}", random_string);
        Self {
            db: DB::open_default(&db_path).unwrap(),
            path: db_path,
        }
    }

    /// Fetches the ColumnFamily handle.
    pub fn get_cf_handle(&self, topic: FSTopic) -> Result<&ColumnFamily, VectorFSError> {
        Ok(self
            .db
            .cf_handle(topic.as_str())
            .ok_or(VectorFSError::FailedFetchingCF)?)
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
        cf: &impl AsColumnFamilyRef,
    ) -> Result<DBIteratorWithThreadMode<'a, DB>, VectorFSError> {
        Ok(self.db.iterator_cf(cf, IteratorMode::Start))
    }

    /// Iterates over the provided column family profile-bounded, meaning that
    /// we filter out all keys in the iterator which are not profile-bounded to the
    /// correct profile, before returning the iterator.
    pub fn iterator_cf_pb<'a>(
        &'a self,
        cf: &impl AsColumnFamilyRef,
        profile: &ShinkaiName,
    ) -> Result<impl Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>> + 'a, VectorFSError> {
        let profile_prefix = Self::get_profile_name(profile)?.into_bytes();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let filtered_iter = iter.filter(move |result| match result {
            Ok((key, _)) => key.starts_with(&profile_prefix),
            Err(_) => false,
        });
        Ok(filtered_iter)
    }

    /// Saves the value inside of the key at the provided column family
    pub fn put_cf<K, V>(&self, cf: &impl AsColumnFamilyRef, key: K, value: V) -> Result<(), VectorFSError>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        Ok(self.db.put_cf(cf, key, value)?)
    }

    /// Saves the value inside of the key (profile-bound) at the provided column family.
    pub fn put_cf_pb<V>(
        &self,
        cf: &impl AsColumnFamilyRef,
        key: &str,
        value: V,
        profile: &ShinkaiName,
    ) -> Result<(), VectorFSError>
    where
        V: AsRef<[u8]>,
    {
        let new_key = Self::generate_profile_bound_key(key, profile)?;
        self.put_cf(cf, new_key, value)
    }

    /// Deletes the key from the provided column family
    pub fn delete_cf<K>(&self, cf: &impl AsColumnFamilyRef, key: K) -> Result<(), VectorFSError>
    where
        K: AsRef<[u8]>,
    {
        Ok(self.db.delete_cf(cf, key)?)
    }

    /// Deletes the key (profile-bound) from the provided column family.
    pub fn delete_cf_pb(
        &self,
        cf: &impl AsColumnFamilyRef,
        key: &str,
        profile: &ShinkaiName,
    ) -> Result<(), VectorFSError> {
        let new_key = Self::generate_profile_bound_key(key, profile)?;
        self.delete_cf(cf, new_key)
    }

    /// Fetches the ColumnFamily handle.
    pub fn cf_handle(&self, name: &str) -> Result<&ColumnFamily, VectorFSError> {
        self.db.cf_handle(name).ok_or(VectorFSError::FailedFetchingCF)
    }
    /// Saves the WriteBatch to the database
    pub fn write(&self, batch: WriteBatch) -> Result<(), VectorFSError> {
        Ok(self.db.write(batch)?)
    }

    /// Profile-bound saves the WriteBatch to the database
    pub fn write_pb(&self, pb_batch: ProfileBoundWriteBatch) -> Result<(), VectorFSError> {
        self.write(pb_batch.write_batch)
    }

    /// Validates if the key has the provided profile name properly prepended to it
    pub fn validate_profile_bound_key(key: &str, profile: &ShinkaiName) -> Result<bool, VectorFSError> {
        let profile_name = Self::get_profile_name(profile)?;
        Ok(key.starts_with(&profile_name))
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key(key: &str, profile: &ShinkaiName) -> Result<String, VectorFSError> {
        let mut prof_name = Self::get_profile_name(profile)?;
        Ok(Self::generate_profile_bound_key_from_str(key, &prof_name))
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key_from_str(key: &str, profile_name: &str) -> String {
        let mut res = profile_name.to_string();
        res.push_str(key);
        res
    }

    /// Extracts the profile name with VectorFSError wrapping
    pub fn get_profile_name(profile: &ShinkaiName) -> Result<String, VectorFSError> {
        profile.get_profile_name().ok_or(VectorFSError::ShinkaiNameLacksProfile)
    }
}
