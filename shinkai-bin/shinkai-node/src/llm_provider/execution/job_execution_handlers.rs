use std::sync::Arc;

use ed25519_dalek::SigningKey;
use serde_json::to_string;
use shinkai_message_primitives::{
    schemas::{
        llm_providers::serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider},
        shinkai_name::ShinkaiName,
    },
    shinkai_utils::{
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::clone_signature_secret_key,
    },
};
use shinkai_vector_resources::utils::random_string;
use tokio::sync::Mutex;

use crate::{
    db::{db_errors::ShinkaiDBError, ShinkaiDB},
    llm_provider::{error::LLMProviderError, job::Job, job_manager::JobManager},
    network::ws_manager::WSUpdateHandler,
    vector_fs::vector_fs::VectorFS,
};

impl JobManager {
    /// Processes the provided image file
    #[allow(clippy::too_many_arguments)]
    pub async fn handle_image_file(
        db: Arc<ShinkaiDB>,
        agent_found: Option<SerializedLLMProvider>,
        full_job: Job,
        task: String,
        content: Vec<u8>,
        profile: ShinkaiName,
        identity_secret_key: SigningKey,
        file_extension: String,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), LLMProviderError> {
        let prev_execution_context = full_job.execution_context.clone();

        let base64_image = match &agent_found {
            Some(agent) => match agent.model {
                LLMProviderInterface::OpenAI(_) => {
                    format!("data:image/{};base64,{}", file_extension, base64::encode(&content))
                }
                LLMProviderInterface::ShinkaiBackend(_) => {
                    format!("data:image/{};base64,{}", file_extension, base64::encode(&content))
                }
                _ => base64::encode(&content),
            },
            None => base64::encode(&content),
        };

        // TODO: fix the new_execution_context
        let (inference_response_content, _) = JobManager::image_analysis_chain(
            db.clone(),
            full_job.clone(),
            agent_found.clone(),
            prev_execution_context.clone(),
            Some(profile.clone()),
            task.clone(),
            base64_image,
            0,
            3,
            ws_manager.clone(),
        )
        .await?;

        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            full_job.job_id.to_string(),
            inference_response_content.clone().to_string(),
            "".to_string(),
            clone_signature_secret_key(&identity_secret_key),
            profile.node_name.clone(),
            profile.node_name.clone(),
        )
        .unwrap();

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("process_image_file> shinkai_message: {:?}", shinkai_message).as_str(),
        );

        // Save response data to DB
        db.add_step_history(
            full_job.job_id.clone(),
            "".to_string(),
            inference_response_content.to_string(),
            None,
        )?;
        db.add_message_to_job_inbox(&full_job.job_id.clone(), &shinkai_message, None, ws_manager)
            .await?;
        db.set_job_execution_context(full_job.job_id.clone(), prev_execution_context, None)?;

        Ok(())
    }
}
