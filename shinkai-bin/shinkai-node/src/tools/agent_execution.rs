use ed25519_dalek::SigningKey;

use reqwest::StatusCode;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::Mutex;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::{llm_provider::job_manager::JobManager, network::Node};

use super::tool_generation::v2_send_basic_job_message_for_existing_job;

pub async fn v2_create_and_send_job_message_for_agent(
    db: Arc<SqliteManager>,
    agent_id: String,
    prompt: String,
    // fs_file_paths: Option<Vec<String>>, // TODO: add this later
    // job_filenames: Option<Vec<String>>, // TODO: add this later
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<String, APIError> {
    // If the environment variable is not set, read from the database
    let bearer = match db.read_api_v2_key() {
        Ok(Some(api_key)) => api_key,
        Ok(None) | Err(_) => {
            let api_error = APIError {
                code: StatusCode::UNAUTHORIZED.as_u16(),
                error: "Unauthorized".to_string(),
                message: "Invalid bearer token".to_string(),
            };
            return Err(api_error);
        }
    };

    // Create job
    let (res_sender, res_receiver) = async_channel::bounded(1);

    let job_creation_info = JobCreationInfo {
        scope: MinimalJobScope::default(),
        is_hidden: Some(true),
        associated_ui: None,
    };

    let _ = Node::v2_create_new_job(
        db.clone(),
        node_name_clone.clone(),
        identity_manager_clone.clone(),
        job_manager_clone.clone(),
        bearer.clone(),
        job_creation_info,
        agent_id.clone(),
        encryption_secret_key_clone.clone(),
        encryption_public_key_clone.clone(),
        signing_secret_key_clone.clone(),
        res_sender,
    )
    .await;

    let job_id = res_receiver
        .recv()
        .await
        .map_err(|e| Node::generic_api_error(&e.to_string()))??;

    // Use the new function to send the message
    v2_send_basic_job_message_for_existing_job(
        bearer,
        job_id.clone(),
        prompt,
        None,
        None,
        None,
        db,
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
    )
    .await?;

    Ok(job_id)
}
