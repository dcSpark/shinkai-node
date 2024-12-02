use serde::{Deserialize, Serialize};
use crate::shinkai_message::shinkai_message_schemas::JobMessage;

use super::job_config::JobConfig;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CronTask {
    pub task_id: i32,
    pub cron: String,
    pub created_at: String,
    pub last_modified: String,
    pub action: CronTaskAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CronTaskAction {
    SendMessageToJob {
        job_id: String,
        message: JobMessage,
    },
    CreateJobWithConfigAndMessage {
        config: JobConfig,
        message: JobMessage,
    },
}
