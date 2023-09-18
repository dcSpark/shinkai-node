use chrono::{DateTime, Utc};
use rocksdb::{
    AsColumnFamilyRef, ColumnFamily, ColumnFamilyDescriptor, DBCommon, DBIteratorWithThreadMode, Error, IteratorMode,
    Options, SingleThreaded, WriteBatch, DB,
};
use shinkai_message_primitives::{
    schemas::{shinkai_name::ShinkaiName, shinkai_time::ShinkaiTime},
    shinkai_message::shinkai_message::ShinkaiMessage,
};
use std::path::Path;
use crate::db::Topic;
use crate::db::db::ProfileBoundWriteBatch;

use super::{db_errors::ShinkaiDBError, ShinkaiDB};

pub struct ShinkaiDBPerAgent {
    pub shinkai_db: ShinkaiDB,
    pub profile: ShinkaiName,
    pub agent_id: String,
}

impl ShinkaiDBPerAgent {
    pub fn new(db_path: &str, profile: ShinkaiName, agent_id: String) -> Result<Self, Error> {
        let db = ShinkaiDB::new(db_path)?;

        Ok(ShinkaiDBPerAgent {
            shinkai_db: db,
            profile,
            agent_id,
        })
    }

    pub fn read_results(&self) -> Result<Vec<(String, String)>, ShinkaiDBError> {
        // Create the column family name
        let cf_name = format!("agentdb_{}_{}", self.profile, self.agent_id);
    
        // Get the column family for the topic
        let cf = match self.shinkai_db.db.cf_handle(&cf_name) {
            Some(cf) => cf,
            None => return Ok(Vec::new()),
        };
    
        // Create an iterator over the column family
        let iter = self.shinkai_db.db.iterator_cf(cf, IteratorMode::Start);
    
        // Collect all key-value pairs into a vector
        let results: Result<Vec<(String, String)>, _> = iter.map(|item| {
            let (key, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let key = String::from_utf8(key.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
            let value = String::from_utf8(value.to_vec()).map_err(|_| ShinkaiDBError::Utf8ConversionError)?;
            Ok((key, value))
        }).collect();
    
        results
    }

    pub fn add_result(&self, context: String, result: String) -> Result<(), ShinkaiDBError> {
        // Create the column family name
        let cf_name = format!("agentdb_{}_{}", self.profile, self.agent_id);

        // Get the column family for the topic
        let cf = self
            .shinkai_db
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_name.clone()))?;

        // Write the result using the context as a key
        self.shinkai_db.put_cf(cf, context.as_bytes(), result.as_bytes())?;

        Ok(())
    }

    pub fn remove_result(&self, context: String) -> Result<(), ShinkaiDBError> {
        // Create the topic name
        let topic_name = format!("agentdb_{}_{}", self.profile, self.agent_id);
    
        // Check if the topic exists and if not, return an error
        let cf = match self.shinkai_db.db.cf_handle(&topic_name) {
            Some(cf) => cf,
            None => return Err(ShinkaiDBError::InboxNotFound(topic_name)),
        };
    
        // Delete the result using the context as a key
        self.shinkai_db.db.delete_cf(cf, context)?;
    
        Ok(())
    }
}
