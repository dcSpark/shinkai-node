use std::collections::HashMap;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use crate::agent::{queue::job_queue_manager::{JobQueueManager, JobForProcessing}};
use rocksdb::IteratorMode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

impl ShinkaiDB {
    pub fn persist_job_queues<T: Serialize>(
        &self,
        job_id: &str,
        queue: &Vec<T>,
    ) -> Result<(), ShinkaiDBError> {
        let serialized_queue = bincode::serialize(queue).map_err(|e| ShinkaiDBError::BincodeError(e))?;
        let cf_handle = self
            .db
            .cf_handle(Topic::JobQueues.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                Topic::JobQueues.as_str().to_string(),
            ))?;
        self.db.put_cf(cf_handle, job_id.as_bytes(), &serialized_queue)?;
        Ok(())
    }

    pub fn get_job_queues<T: DeserializeOwned>(&self, job_id: &str) -> Result<Vec<T>, ShinkaiDBError> {
        let cf_handle = self
            .db
            .cf_handle(Topic::JobQueues.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                Topic::JobQueues.as_str().to_string(),
            ))?;
        let serialized_queue = self
            .db
            .get_cf(cf_handle, job_id.as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let queue: Vec<T> =
            bincode::deserialize(&serialized_queue).map_err(|e| ShinkaiDBError::BincodeError(e))?;
        Ok(queue)
    }

    pub fn get_all_queues<T: DeserializeOwned>(&self) -> Result<HashMap<String, Vec<T>>, ShinkaiDBError> {
        let cf_handle = self
            .db
            .cf_handle(Topic::JobQueues.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                Topic::JobQueues.as_str().to_string(),
            ))?;
        let mut queues = HashMap::new();

        let iterator = self.db.iterator_cf(cf_handle, IteratorMode::Start);

        for res in iterator {
            let (key, value) = res.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let job_id = String::from_utf8(key.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
            let queue: Vec<T> =
                bincode::deserialize(&value).map_err(|e| ShinkaiDBError::BincodeError(e))?;
            queues.insert(job_id, queue);
        }

        Ok(queues)
    }

    pub fn persist_queue<T: Serialize>(
        &self,
        job_id: &str,
        queue: &Vec<T>,
    ) -> Result<(), ShinkaiDBError> {
        let serialized_queue = bincode::serialize(queue).map_err(|e| ShinkaiDBError::BincodeError(e))?;
        let cf_handle = self
            .db
            .cf_handle(Topic::JobQueues.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                Topic::JobQueues.as_str().to_string(),
            ))?;
        self.db.put_cf(cf_handle, job_id.as_bytes(), &serialized_queue)?;
        Ok(())
    }
}
