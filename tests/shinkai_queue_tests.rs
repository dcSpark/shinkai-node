impl ShinkaiDB {
    pub fn persist_job_queues<T>(&self, key: &str, queue: &Queue<T>) -> Result<(), ShinkaiDBError> 
    where
        T: Serialize + Send + Sync + Clone,
    {
        let serialized_queue = bincode::serialize(queue).map_err(|_| ShinkaiDBError::SerializationError)?;
        let cf_handle = self.db.cf_handle(Topic::JobQueues.as_str()).ok_or(ShinkaiDBError::ColumnFamilyNotFound(Topic::JobQueues.as_str().to_string()))?;
        self.db.put_cf(cf_handle, key.as_bytes(), &serialized_queue)?;
        Ok(())
    }

    pub fn get_job_queues<T>(&self, key: &str) -> Result<Queue<T>, ShinkaiDBError> 
    where
        T: Deserialize<'static> + Send + Sync + Clone,
    {
        let cf_handle = self.db.cf_handle(Topic::JobQueues.as_str()).ok_or(ShinkaiDBError::ColumnFamilyNotFound(Topic::JobQueues.as_str().to_string()))?;
        let serialized_queue = self.db.get_cf(cf_handle, key.as_bytes())?.ok_or(ShinkaiDBError::DataNotFound)?;
        let queue: Queue<T> = bincode::deserialize(&serialized_queue).map_err(|_| ShinkaiDBError::DeserializationError)?;
        Ok(queue)
    }

    pub fn get_all_queues<T>(&self) -> Result<HashMap<String, Queue<T>>, ShinkaiDBError> 
    where
        T: Deserialize<'static> + Send + Sync + Clone,
    {
        let cf_handle = self.db.cf_handle(Topic::JobQueues.as_str()).ok_or(ShinkaiDBError::ColumnFamilyNotFound(Topic::JobQueues.as_str().to_string()))?;
        let mut queues = HashMap::new();

        for (key, value) in self.db.iterator_cf(cf_handle, IteratorMode::Start) {
            let key = String::from_utf8(key.to_vec()).map_err(|_| ShinkaiDBError::KeyParseError)?;
            let queue: Queue<T> = bincode::deserialize(&value).map_err(|_| ShinkaiDBError::DeserializationError)?;
            queues.insert(key, queue);
        }

        Ok(queues)
    }
}