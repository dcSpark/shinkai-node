use shinkai_message_primitives::shinkai_utils::utils::random_string;

use super::kai_files::{KaiJobFile, KaiSchemaType};
use crate::network::Node;
use std::{error, fmt};

#[derive(Debug)]
pub enum KaiJobFileManagerError {
    ProfileNotFound,
    SerdeJsonError(serde_json::Error),
    CustomError(String),
}

impl fmt::Display for KaiJobFileManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KaiJobFileManagerError::ProfileNotFound => write!(f, "Profile not found"),
            KaiJobFileManagerError::SerdeJsonError(e) => write!(f, "Serde JSON error: {}", e),
            KaiJobFileManagerError::CustomError(e) => write!(f, "Custom error: {}", e),
            // Handle other kinds of errors here
        }
    }
}

impl error::Error for KaiJobFileManagerError {}

impl From<serde_json::Error> for KaiJobFileManagerError {
    fn from(err: serde_json::Error) -> KaiJobFileManagerError {
        KaiJobFileManagerError::SerdeJsonError(err)
    }
}

impl From<String> for KaiJobFileManagerError {
    fn from(err: String) -> KaiJobFileManagerError {
        KaiJobFileManagerError::CustomError(err)
    }
}

impl From<&str> for KaiJobFileManagerError {
    fn from(err: &str) -> KaiJobFileManagerError {
        KaiJobFileManagerError::CustomError(err.to_string())
    }
}

pub struct KaiJobFileManager;

impl KaiJobFileManager {
    pub async fn execute(kai_file: KaiJobFile, node: &Node) -> Result<(), KaiJobFileManagerError> {
        eprintln!("KaiJobFileManager::execute");
        match kai_file.schema {
            KaiSchemaType::CronJobRequest(cron_task_request) => {
                // Nothing to do
                eprintln!("KaiSchemaType::CronJobRequest: {:?}", cron_task_request);
            }
            KaiSchemaType::CronJob(_) => {
                // Add your logic for CronJob here, or ignore it
                eprintln!("KaiSchemaType::CronJob variant matched but not handled");
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
                        let profile = match kai_file.shinkai_profile.ok_or(KaiJobFileManagerError::ProfileNotFound) {
                            Ok(profile) => profile.extract_profile().map_err(KaiJobFileManagerError::from)?,
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
                                kai_file.agent_id,
                            )
                            .await;
                    }
                    None => {
                        eprintln!("Cron manager not found");
                    }
                }
            }
        }
        eprintln!("KaiJobFileManager::execute: Done (right before OK)");
        Ok(())
    }
}
