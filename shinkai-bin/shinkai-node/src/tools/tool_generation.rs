use ed25519_dalek::SigningKey;

use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::managers::IdentityManager;
use crate::{llm_provider::job_manager::JobManager, network::Node};

pub async fn v2_create_and_send_job_message(
    bearer: String,
    job_creation_info: JobCreationInfo,
    llm_provider: String,
    content: String,
    db_clone: Arc<RwLock<SqliteManager>>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<String, APIError> {
    // Create job
    let (res_sender, res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_create_new_job(
        db_clone.clone(),
        node_name_clone.clone(),
        identity_manager_clone.clone(),
        job_manager_clone.clone(),
        bearer.clone(),
        job_creation_info,
        llm_provider,
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

    // Get the current job config
    let (config_res_sender, config_res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_api_get_job_config(
        db_clone.clone(),
        bearer.clone(),
        job_id.clone(),
        config_res_sender,
    )
    .await;

    let current_config = config_res_receiver
        .recv()
        .await
        .map_err(|e| Node::generic_api_error(&e.to_string()))??;

    // Merge the current config with the new config setting use_tools to false
    let new_config = JobConfig {
        use_tools: Some(false),
        ..current_config
    };

    let (update_res_sender, update_res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_api_update_job_config(
        db_clone.clone(),
        bearer.clone(),
        job_id.clone(),
        new_config,
        update_res_sender,
    )
    .await;

    update_res_receiver
        .recv()
        .await
        .map_err(|e| Node::generic_api_error(&e.to_string()))??;

    // Use the new function to send the message
    v2_send_basic_job_message_for_existing_job(
        bearer,
        job_id.clone(),
        content,
        db_clone,
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

pub async fn v2_send_basic_job_message_for_existing_job(
    bearer: String,
    job_id: String,
    content: String,
    db_clone: Arc<RwLock<SqliteManager>>,
    node_name_clone: ShinkaiName,
    identity_manager_clone: Arc<Mutex<IdentityManager>>,
    job_manager_clone: Arc<Mutex<JobManager>>,
    encryption_secret_key_clone: EncryptionStaticKey,
    encryption_public_key_clone: EncryptionPublicKey,
    signing_secret_key_clone: SigningKey,
) -> Result<(), APIError> {
    // Send message
    let job_message = JobMessage {
        job_id: job_id.clone(),
        content,
        files_inbox: "".to_string(),
        parent: None,
        sheet_job_data: None,
        callback: None,
        metadata: None,
        tool_key: None,
    };

    let (res_sender, res_receiver) = async_channel::bounded(1);

    let _ = Node::v2_job_message(
        db_clone,
        node_name_clone,
        identity_manager_clone,
        job_manager_clone,
        bearer,
        job_message,
        encryption_secret_key_clone,
        encryption_public_key_clone,
        signing_secret_key_clone,
        res_sender,
    )
    .await;

    res_receiver
        .recv()
        .await
        .map_err(|e| Node::generic_api_error(&e.to_string()))??;
    Ok(())
}
