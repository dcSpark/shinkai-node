use rocksdb::Error as RocksDBError;
use rocksdb::{ColumnFamilyDescriptor, Error, IteratorMode, LogLevel, Options, DB};
use std::{fmt, path::Path, sync::Arc};
use std::{path::PathBuf, time::Instant};

use crate::synchronizer::SyncingFolder;

#[derive(Debug)]
pub enum ShinkaiMirrorDBError {
    ColumnFamilyNotFound(String),
    SerializationError(String),
    DeserializationError(String),
    DBError(RocksDBError),
}

impl fmt::Display for ShinkaiMirrorDBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShinkaiMirrorDBError::ColumnFamilyNotFound(cf_name) => {
                write!(f, "Column family not found: {}", cf_name)
            }
            ShinkaiMirrorDBError::SerializationError(err) => write!(f, "Serialization error: {}", err),
            ShinkaiMirrorDBError::DeserializationError(err) => write!(f, "Deserialization error: {}", err),
            ShinkaiMirrorDBError::DBError(err) => write!(f, "RocksDB error: {}", err),
        }
    }
}

impl std::error::Error for ShinkaiMirrorDBError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShinkaiMirrorDBError::DBError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<RocksDBError> for ShinkaiMirrorDBError {
    fn from(err: RocksDBError) -> ShinkaiMirrorDBError {
        ShinkaiMirrorDBError::DBError(err)
    }
}

pub enum MirrorTopic {
    FileMirror,
}

impl MirrorTopic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileMirror => "file_mirror",
        }
    }
}
pub struct ShinkaiMirrorDB {
    pub db: DB,
    pub path: String,
}

impl ShinkaiMirrorDB {
    pub fn new(db_path: &str) -> Result<Self, Error> {
        let start = Instant::now();
        let db_opts = Self::create_cf_options(None);

        let cf_names = if Path::new(db_path).exists() {
            // If the database file exists, get the list of column families from the database
            DB::list_cf(&db_opts, db_path)?
        } else {
            // If the database file does not exist, use the default list of column families
            vec![MirrorTopic::FileMirror.as_str().to_string()]
        };

        let mut cfs = vec![];
        for cf_name in &cf_names {
            let prefix_length = match cf_name.as_str() {
                "file_mirror" => Some(47),
                _ => None, // No prefix extractor for other CFs
            };
            let db_opts = Self::create_cf_options(prefix_length);
            let cf_desc = ColumnFamilyDescriptor::new(cf_name.to_string(), db_opts);
            cfs.push(cf_desc);
        }

        let db = DB::open_cf_descriptors(&db_opts, db_path, cfs)?;

        if std::env::var("DEBUG_TIMING").unwrap_or_default() == "true" {
            let elapsed = start.elapsed();
            println!("### RocksDB loaded in: {:?}", elapsed);

            // Assuming db_opts is configured and used to open the database
            let stats = db_opts.get_statistics().expect("Statistics should be enabled");

            // After opening the database
            println!("RocksDB stats: {}", stats);
        }

        let shinkai_db = ShinkaiMirrorDB {
            db,
            path: db_path.to_string(),
        };

        Ok(shinkai_db)
    }

    pub fn create_cf_options(prefix_length: Option<usize>) -> Options {
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);
        cf_opts.set_log_level(LogLevel::Debug);

        cf_opts.set_allow_concurrent_memtable_write(true);
        cf_opts.set_enable_write_thread_adaptive_yield(true);
        cf_opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        cf_opts.set_max_write_buffer_number(3);
        cf_opts.set_min_write_buffer_number_to_merge(1);
        cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        // cf_opts.increase_parallelism(std::cmp::max(1, num_cpus::get() as i32 / 2));
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

        if std::env::var("DEBUG_TIMING").unwrap_or_default() == "true" {
            cf_opts.set_db_log_dir("./rocksdb_logs");
        }

        cf_opts
    }

    // Adds or updates the state of a file mirror with a profile name
    pub fn add_or_update_file_mirror_state(
        &self,
        profile_name: String,
        key: PathBuf,
        value: SyncingFolder,
    ) -> Result<(), ShinkaiMirrorDBError> {
        let cf_handle = self
            .db
            .cf_handle(MirrorTopic::FileMirror.as_str())
            .ok_or_else(|| ShinkaiMirrorDBError::ColumnFamilyNotFound(MirrorTopic::FileMirror.as_str().to_string()))?;
        let combined_key = format!("{}_{}", profile_name, key.to_string_lossy());
        let key = serde_json::to_vec(&combined_key)
            .map_err(|_| ShinkaiMirrorDBError::SerializationError(combined_key.clone()))?;
        let value = serde_json::to_vec(&value)
            .map_err(|_| ShinkaiMirrorDBError::SerializationError("Value serialization failed".to_string()))?;
        self.db
            .put_cf(cf_handle, key, value)
            .map_err(ShinkaiMirrorDBError::from)?;
        Ok(())
    }

    // Retrieves the state of a file mirror with a profile name
    pub fn get_file_mirror_state(
        &self,
        profile_name: String,
        key: PathBuf,
    ) -> Result<Option<SyncingFolder>, ShinkaiMirrorDBError> {
        let cf_handle = self
            .db
            .cf_handle(MirrorTopic::FileMirror.as_str())
            .ok_or_else(|| ShinkaiMirrorDBError::ColumnFamilyNotFound(MirrorTopic::FileMirror.as_str().to_string()))?;
        let combined_key = format!("{}_{}", profile_name, key.to_string_lossy());
        let key = serde_json::to_vec(&combined_key)
            .map_err(|_| ShinkaiMirrorDBError::SerializationError(combined_key.clone()))?;
        match self.db.get_cf(cf_handle, key)? {
            Some(value) => {
                let syncing_folder: SyncingFolder = serde_json::from_slice(&value).map_err(|_| {
                    ShinkaiMirrorDBError::DeserializationError("Failed to deserialize SyncingFolder".to_string())
                })?;
                Ok(Some(syncing_folder))
            }
            None => Ok(None),
        }
    }

    // Retrieves the states of all file mirrors with a profile name
    pub fn all_file_mirror_states(&self) -> Result<Vec<(PathBuf, SyncingFolder)>, ShinkaiMirrorDBError> {
        let cf_handle = self
            .db
            .cf_handle(MirrorTopic::FileMirror.as_str())
            .ok_or_else(|| ShinkaiMirrorDBError::ColumnFamilyNotFound(MirrorTopic::FileMirror.as_str().to_string()))?;
        let iterator = self.db.iterator_cf(cf_handle, IteratorMode::Start).flatten();

        let mut results = Vec::new();
        for (key, value) in iterator {
            let combined_key: String = serde_json::from_slice(&key)
                .map_err(|_| ShinkaiMirrorDBError::DeserializationError("Failed to deserialize key".to_string()))?;
            if let Some(pos) = combined_key.find('_') {
                let (_, key_str) = combined_key.split_at(pos + 1);
                let key: PathBuf = key_str.into();
                let value: SyncingFolder = serde_json::from_slice(&value).map_err(|_| {
                    ShinkaiMirrorDBError::DeserializationError("Failed to deserialize SyncingFolder".to_string())
                })?;
                results.push((key, value));
            }
        }
        Ok(results)
    }
}
