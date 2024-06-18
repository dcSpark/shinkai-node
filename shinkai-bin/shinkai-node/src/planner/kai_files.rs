use serde::{Serialize, Deserialize};
use serde_json::Value;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use crate::{cron_tasks::web_scrapper::{CronTaskRequest, CronTaskRequestResponse}, db::db_cron_task::CronTask};

// Define your schema types here
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum KaiSchemaType {
    #[serde(rename = "cronjobrequest")]
    JobRequest(CronTaskRequest),
    #[serde(rename = "cronjobresponse")]
    JobRequestResponse(CronTaskRequestResponse),
    #[serde(rename = "cronjob")]
    CronJob(CronTask),
}

// Define your KaiJobFile struct here
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KaiJobFile {
    pub schema: KaiSchemaType,
    pub shinkai_profile: Option<ShinkaiName>,
    pub llm_provider_id: String
}

impl KaiJobFile {
    pub fn parse_content(&self) -> Result<Value, serde_json::Error> {
        match &self.schema {
            KaiSchemaType::JobRequest(cron_task_request) => {
                serde_json::to_value(cron_task_request)
            },
            KaiSchemaType::JobRequestResponse(cron_task_response) => {
                serde_json::to_value(cron_task_response)
            },
            KaiSchemaType::CronJob(cron_job) => {
                serde_json::to_value(cron_job)
            },
        }
    }

    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> Result<String, serde_json::Error> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}