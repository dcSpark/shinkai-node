use crate::shinkai_message::shinkai_message_schemas::{JobCreationInfo, JobMessage};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::job_config::JobConfig;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CronTask {
    pub name: String,
    pub description: Option<String>,
    pub task_id: i32,
    pub cron: String,
    pub created_at: String,
    pub last_modified: String,
    pub action: CronTaskAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum CronTaskAction {
    SendMessageToJob {
        job_id: String,
        message: JobMessage,
    },
    CreateJobWithConfigAndMessage {
        config: JobConfig,
        message: JobMessage,
        job_creation_info: JobCreationInfo,
        llm_provider: String,
    },
}
