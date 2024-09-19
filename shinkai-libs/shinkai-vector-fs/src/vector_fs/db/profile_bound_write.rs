
use rocksdb::{AsColumnFamilyRef, ColumnFamily, DBIteratorWithThreadMode, IteratorMode, WriteBatch, DB};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use crate::vector_fs::{db::fs_db::TransactionOperation, vector_fs_error::VectorFSError};
use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

/// A struct that wraps rocksdb::WriteBatch and offers the same
/// base interface, however fully profile-bounded. In other words
/// all puts add the profile name as a prefix to all keys.
pub struct ProfileBoundWriteBatch {
    pub operations: Vec<TransactionOperation>, // TODO: Should it be RwLock?
    pub profile_name: String,
}

impl ProfileBoundWriteBatch {
    /// Create a new ProfileBoundWriteBatch with ShinkaiDBError wrapping
    pub fn new(profile: &ShinkaiName) -> Result<Self, ShinkaiDBError> {
        // Also validates that the name includes a profile
        let profile_name = ShinkaiDB::get_profile_name_string(profile)?;
        // Create write batch
        let operations = Vec::new();
        Ok(Self {
            profile_name,
            operations
        })
    }

    /// Create a new ProfileBoundWriteBatch with VectorFSError wrapping
    pub fn new_vfs_batch(profile: &ShinkaiName) -> Result<Self, VectorFSError> {
        // Also validates that the name includes a profile
        match ShinkaiDB::get_profile_name_string(profile) {
            Ok(profile_name) => {
                Ok(Self {
                    operations: Vec::new(), // Initialize the operations vector
                    profile_name,
                })
            },
            Err(_) => Err(VectorFSError::FailedCreatingProfileBoundWriteBatch(profile.to_string())),
        }
    }

    /// Saves the value inside of the key (profile-bound) at the provided column family.
    pub fn pb_put_cf<V>(&mut self, cf_name: &str, key: &str, value: V)
    where
        V: AsRef<[u8]>,
    {
        let new_key = self.gen_pb_key(key);
        self.operations.push(TransactionOperation::Write(cf_name.to_string(), new_key, value.as_ref().to_vec()));

    }

    /// Removes the value inside of the key (profile-bound) at the provided column family.
    pub fn pb_delete_cf(&mut self, cf_name: &str, key: &str) {
        let new_key = self.gen_pb_key(key);
        self.operations.push(TransactionOperation::Delete(cf_name.to_string(), new_key));

    }

    /// Given an input key, generates the profile bound key using the internal profile.
    pub fn gen_pb_key(&self, key: &str) -> String {
        ShinkaiDB::generate_profile_bound_key_from_str(key, &self.profile_name)
    }
}

impl ShinkaiDB {
    /// Fetches the ColumnFamily handle.
    pub fn get_cf_handle(&self, topic: Topic) -> Result<&ColumnFamily, ShinkaiDBError> {
        self
            .db
            .cf_handle(topic.as_str())
            .ok_or(ShinkaiDBError::FailedFetchingCF)
    }

    /// Fetches the value of a KV pair and returns it as a Vector of bytes.
    pub fn get_cf<K: AsRef<[u8]>>(&self, topic: Topic, key: K) -> Result<Vec<u8>, ShinkaiDBError> {
        let colfam = self.get_cf_handle(topic)?;
        let bytes = self
            .db
            .get_cf(colfam, key)?
            .ok_or(ShinkaiDBError::FailedFetchingValue)?;
        Ok(bytes)
    }

    /// Fetching the value of a KV pair that is profile-bound, returning it as a Vector of bytes.
    /// In practice this means the profile name is prepended to the supplied key before
    /// performing the fetch.
    pub fn pb_topic_get(&self, topic: Topic, key: &str, profile: &ShinkaiName) -> Result<Vec<u8>, ShinkaiDBError> {
        let new_key = ShinkaiDB::generate_profile_bound_key(key, profile)?;
        self.get_cf(topic, new_key.as_bytes())
    }

    /// Fetches all keys in a ColumnFamily that are profile-bound.
    pub fn pb_cf_get_all_keys(
        &self,
        cf: &impl AsColumnFamilyRef,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        let profile_prefix = ShinkaiDB::get_profile_name_string(profile)?.into_bytes();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut keys = Vec::new();

        for item in iter {
            let (key, _) = item.map_err(ShinkaiDBError::from)?;
            if key.starts_with(&profile_prefix) {
                let key_str = String::from_utf8(key[profile_prefix.len() + 1..].to_vec())
                    .map_err(|_| ShinkaiDBError::InvalidData)?;
                keys.push(key_str);
            }
        }

        Ok(keys)
    }

    /// Iterates over the provided column family
    pub fn iterator_cf<'a>(
        &'a self,
        cf: &impl AsColumnFamilyRef,
    ) -> Result<DBIteratorWithThreadMode<'a, DB>, ShinkaiDBError> {
        Ok(self.db.iterator_cf(cf, IteratorMode::Start))
    }

    /// Iterates over the provided column family profile-bounded, meaning that
    /// we filter out all keys in the iterator which are not profile-bounded to the
    /// correct profile, before returning the iterator.
    pub fn pb_iterator_cf<'a>(
        &'a self,
        cf: &impl AsColumnFamilyRef,
        profile: &ShinkaiName,
    ) -> Result<impl Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>> + 'a, ShinkaiDBError> {
        let profile_prefix = ShinkaiDB::get_profile_name_string(profile)?.into_bytes();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let filtered_iter = iter.filter(move |result| match result {
            Ok((key, _)) => key.starts_with(&profile_prefix),
            Err(_) => false,
        });
        Ok(filtered_iter)
    }

    /// Saves the value inside of the key at the provided column family
    pub fn put_cf<K, V>(&self, cf: &impl AsColumnFamilyRef, key: K, value: V) -> Result<(), ShinkaiDBError>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        Ok(self.db.put_cf(cf, key, value)?)
    }

    /// Saves the value inside of the key (profile-bound) at the provided column family.
    pub fn pb_put_cf<V>(
        &self,
        cf: &impl AsColumnFamilyRef,
        key: &str,
        value: V,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError>
    where
        V: AsRef<[u8]>,
    {
        let new_key = ShinkaiDB::generate_profile_bound_key(key, profile)?;
        self.put_cf(cf, new_key.as_bytes(), value)
    }

    /// Deletes the key from the provided column family
    pub fn delete_cf<K>(&self, cf: &impl AsColumnFamilyRef, key: K) -> Result<(), ShinkaiDBError>
    where
        K: AsRef<[u8]>,
    {
        Ok(self.db.delete_cf(cf, key)?)
    }

    /// Deletes the key (profile-bound) from the provided column family.
    pub fn pb_delete_cf(
        &self,
        cf: &impl AsColumnFamilyRef,
        key: &str,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let new_key = ShinkaiDB::generate_profile_bound_key(key, profile)?;
        self.delete_cf(cf, new_key)
    }

    /// Fetches the ColumnFamily handle.
    pub fn cf_handle(&self, name: &str) -> Result<&ColumnFamily, ShinkaiDBError> {
        self.db.cf_handle(name).ok_or(ShinkaiDBError::FailedFetchingCF)
    }
    /// Saves the WriteBatch to the database
    pub fn write(&self, batch: WriteBatch) -> Result<(), ShinkaiDBError> {
        Ok(self.db.write(batch)?)
    }

    /// Profile-bound saves the WriteBatch to the database
    pub fn write_pb(&self, _pb_batch: ProfileBoundWriteBatch) -> Result<(), ShinkaiDBError> {
        // self.write(pb_batch.write_batch)
        panic!("Not implemented");
    }

    /// Validates if the key has the provided profile name properly prepended to it
    pub fn validate_profile_bound_key(key: &str, profile: &ShinkaiName) -> Result<bool, ShinkaiDBError> {
        let profile_name = ShinkaiDB::get_profile_name_string(profile)?;
        Ok(key.starts_with(&profile_name))
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key(key: &str, profile: &ShinkaiName) -> Result<String, ShinkaiDBError> {
        let prof_name = ShinkaiDB::get_profile_name_string(profile)?;
        Ok(Self::generate_profile_bound_key_from_str(key, &prof_name))
    }

    /// Prepends the profile name to the provided key to make it "profile bound"
    pub fn generate_profile_bound_key_from_str(key: &str, profile_name: &str) -> String {
        let mut prof_name = profile_name.to_string() + ":";
        prof_name.push_str(key);
        prof_name
    }
}