use shinkai_message_primitives::shinkai_utils::utils::random_string;

use super::kai_files::{KaiFile, KaiSchemaType};
use crate::network::Node;
use std::{error, fmt};

#[derive(Debug)]
pub enum KaiFileManagerError {
    ProfileNotFound,
    SerdeJsonError(serde_json::Error),
    CustomError(String),
}

impl fmt::Display for KaiFileManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KaiFileManagerError::ProfileNotFound => write!(f, "Profile not found"),
            KaiFileManagerError::SerdeJsonError(e) => write!(f, "Serde JSON error: {}", e),
            KaiFileManagerError::CustomError(e) => write!(f, "Custom error: {}", e),
            // Handle other kinds of errors here
        }
    }
}

impl error::Error for KaiFileManagerError {}

impl From<serde_json::Error> for KaiFileManagerError {
    fn from(err: serde_json::Error) -> KaiFileManagerError {
        KaiFileManagerError::SerdeJsonError(err)
    }
}

impl From<String> for KaiFileManagerError {
    fn from(err: String) -> KaiFileManagerError {
        KaiFileManagerError::CustomError(err)
    }
}

impl From<&str> for KaiFileManagerError {
    fn from(err: &str) -> KaiFileManagerError {
        KaiFileManagerError::CustomError(err.to_string())
    }
}

pub struct KaiFileManager;

impl KaiFileManager {
    pub async fn execute(kai_file: KaiFile, node: &Node) -> Result<(), KaiFileManagerError> {
        eprintln!("KaiFileManager::execute");
        match kai_file.schema {
            KaiSchemaType::CronJobRequest(cron_task_request) => {
                // Nothing to do
                eprintln!("KaiSchemaType::CronJobRequest: {:?}", cron_task_request);
            }
            KaiSchemaType::CronJobResponse(cron_task_response) => {
                eprintln!("KaiSchemaType::CronJobResponse: {:?}", cron_task_response);
                // Execute code for CronJobResponse
                // You can use cron_task_response which is of type CronTaskResponse

                match &node.cron_manager {
                    Some(cron_manager) => {
                        eprintln!("Cron manager found");
                        let random_hash = random_string();

                        let url = cron_task_response
                            .cron_task_request
                            .object_description
                            .unwrap_or("".to_string());
                        let profile = match kai_file.shinkai_profile.ok_or(KaiFileManagerError::ProfileNotFound) {
                            Ok(profile) => profile.extract_profile().map_err(KaiFileManagerError::from)?,
                            Err(e) => return Err(e),
                        };

                        let cron_manager = cron_manager.lock().await;
                        cron_manager
                            .add_cron_task(
                                profile.to_string(),
                                random_hash,
                                cron_task_response.cron_description,
                                cron_task_response.cron_task_request.task_description,
                                "".to_string(),
                                url,
                                true, // TODO: Remove or maybe we should be able to extract this from the PDDL?
                                "agent_id".to_string(), // Agent should match the one used for the job
                            )
                            .await;
                    }
                    None => {
                        eprintln!("Cron manager not found");
                    }
                }
            }
        }
        eprintln!("KaiFileManager::execute: Done (right before OK)");
        Ok(())
    }
}
