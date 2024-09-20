use std::collections::HashMap;

use super::{db_errors::ShinkaiDBError, ShinkaiDB};

use rocksdb::IteratorMode;
use serde::{de::DeserializeOwned, Serialize};

impl ShinkaiDB {
    pub fn persist_job_queues<T: Serialize>(
        &self,
        cf_name: &str,
        job_id: &str,
        queue: &Vec<T>,
        prefix: Option<String>,
    ) -> Result<(), ShinkaiDBError> {
        let cf_handle = self
            .db
            .cf_handle(cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_name.to_string()))?;
        let full_job_id = match &prefix {
            Some(p) => format!("{}{}", p, job_id),
            None => job_id.to_string(),
        };
        let serialized_queue = bincode::serialize(queue).map_err(ShinkaiDBError::BincodeError)?;
        self.db.put_cf(cf_handle, full_job_id.as_bytes(), serialized_queue)?;
        Ok(())
    }

    pub fn get_job_queues<T: DeserializeOwned>(
        &self,
        cf_name: &str,
        job_id: &str,
        prefix: Option<String>,
    ) -> Result<Vec<T>, ShinkaiDBError> {
        let cf_handle = self
            .db
            .cf_handle(cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_name.to_string()))?;
        let full_job_id = match &prefix {
            Some(p) => format!("{}{}", p, job_id),
            None => job_id.to_string(),
        };
        let serialized_queue = self
            .db
            .get_cf(cf_handle, full_job_id.as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let queue: Vec<T> = bincode::deserialize(&serialized_queue).map_err(ShinkaiDBError::BincodeError)?;
        Ok(queue)
    }

    pub fn get_all_queues<T: DeserializeOwned>(
        &self,
        cf_name: &str,
        prefix: Option<String>,
    ) -> Result<HashMap<String, Vec<T>>, ShinkaiDBError> {
        let cf_handle = self
            .db
            .cf_handle(cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_name.to_string()))?;
        let mut queues = HashMap::new();

        // Use prefix_iterator_cf if a prefix is provided, otherwise use a full iterator
        let iterator = match &prefix {
            Some(p) => self.db.prefix_iterator_cf(cf_handle, p.as_bytes()),
            None => self.db.iterator_cf(cf_handle, IteratorMode::Start),
        };

        for res in iterator {
            let (key, value) = res.map_err(ShinkaiDBError::RocksDBError)?;
            let mut job_id = String::from_utf8(key.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
            // If a prefix is provided, remove it from the job_id
            if let Some(p) = &prefix {
                if job_id.starts_with(p) {
                    job_id = job_id[p.len()..].to_string();
                }
            }
            let queue: Vec<T> = bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;
            queues.insert(job_id, queue);
        }

        Ok(queues)
    }

    pub fn persist_queue<T: Serialize>(
        &self,
        cf_name: &str,
        job_id: &str,
        queue: &Vec<T>,
        prefix: Option<String>,
    ) -> Result<(), ShinkaiDBError> {
        let cf_handle = self
            .db
            .cf_handle(cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_name.to_string()))?;

        let full_job_id = match &prefix {
            Some(p) => format!("{}{}", p, job_id),
            None => job_id.to_string(),
        };

        let serialized_queue = bincode::serialize(queue).map_err(ShinkaiDBError::BincodeError)?;
        self.db.put_cf(cf_handle, full_job_id.as_bytes(), serialized_queue)?;
        Ok(())
    }
}
