use std::collections::HashMap;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use crate::agent::queue::job_queue_manager::{JobForProcessing, JobQueueManager};
use rocksdb::{ColumnFamily, IteratorMode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

impl ShinkaiDB {

}