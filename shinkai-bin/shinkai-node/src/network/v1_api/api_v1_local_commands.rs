use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::network::Node;
use async_channel::Sender;
use ed25519_dalek::SigningKey;
use log::error;

use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::identity::Identity;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::{
    schemas::{llm_providers::serialized_llm_provider::SerializedLLMProvider, shinkai_name::ShinkaiName}, shinkai_message::{
        shinkai_message::ShinkaiMessage, shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType}
    }
};
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::Mutex;

impl Node {
    pub async fn local_get_last_messages_from_inbox(
        db: Arc<SqliteManager>,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    ) {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = Self::internal_get_last_messages_from_inbox(db, inbox_name, limit, offset_key).await;

        let single_msg_array_array = result.into_iter().filter_map(|msg| msg.first().cloned()).collect();

        // Send the retrieved messages back to the requester.
        if let Err(e) = res.send(single_msg_array_array).await {
            error!("Failed to send last messages from inbox: {}", e);
        }
    }

    pub async fn local_get_last_messages_from_inbox_with_branches(
        db: Arc<SqliteManager>,
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<Vec<ShinkaiMessage>>>,
    ) {
        // Query the database for the last `limit` number of messages from the specified inbox.
        let result = Self::internal_get_last_messages_from_inbox(db, inbox_name, limit, offset_key).await;

        // Send the retrieved messages back to the requester.
        if let Err(e) = res.send(result).await {
            error!("Failed to send last messages from inbox: {}", e);
        }
    }

    pub async fn local_create_and_send_registration_code(
        db: Arc<SqliteManager>,
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
        res: Sender<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let code = match db.generate_registration_new_code(permissions, code_type) {
            Ok(code) => code,
            Err(e) => {
                error!("Failed to generate registration new code: {}", e);
                "".to_string()
            }
        };
        if let Err(e) = res.send(code).await {
            error!("Failed to send code: {}", e);
            return Err(Box::new(e));
        }
        Ok(())
    }

    pub async fn local_get_all_subidentities_devices_and_llm_providers(
        identity_manager: Arc<Mutex<IdentityManager>>,
        res: Sender<Result<Vec<Identity>, APIError>>,
    ) {
        let identity_manager = identity_manager.lock().await;
        let result = identity_manager.get_all_subidentities_devices_and_llm_providers();

        if let Err(e) = res.send(Ok(result)).await {
            error!("Failed to send result: {}", e);
            let error = APIError {
                code: 500,
                error: "ChannelSendError".to_string(),
                message: "Failed to send data through the channel".to_string(),
            };
            let _ = res.send(Err(error)).await;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn local_add_llm_provider(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        agent: SerializedLLMProvider,
        profile: &ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<String>,
    ) {
        let result = Self::internal_add_llm_provider(
            db,
            identity_manager,
            job_manager,
            identity_secret_key,
            agent,
            profile,
            ws_manager,
        )
        .await;
        let result_str = match result {
            Ok(_) => "true".to_string(),
            Err(e) => format!("Error: {:?}", e),
        };
        let _ = res.send(result_str).await;
    }

    pub async fn local_available_llm_providers(
        db: Arc<SqliteManager>,
        node_name: &ShinkaiName,
        full_profile_name: String,
        res: Sender<Result<Vec<SerializedLLMProvider>, String>>,
    ) {
        match Self::internal_get_llm_providers_for_profile(db, node_name.clone().node_name, full_profile_name).await {
            Ok(llm_providers) => {
                let _ = res.send(Ok(llm_providers)).await;
            }
            Err(err) => {
                let _ = res.send(Err(format!("Internal Server Error: {}", err))).await;
            }
        }
    }

    pub async fn local_is_pristine(db: Arc<SqliteManager>, res: Sender<bool>) {
        let has_any_profile = db.has_any_profile().unwrap_or(false);
        let _ = res.send(!has_any_profile).await;
    }

    pub async fn local_scan_ollama_models(res: Sender<Result<Vec<serde_json::Value>, String>>) {
        let result = Self::internal_scan_ollama_models().await;
        let _ = res.send(result.map_err(|e| e.message)).await;
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn local_add_ollama_models(
        db: Arc<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager: Arc<Mutex<JobManager>>,
        identity_secret_key: SigningKey,
        input_models: Vec<String>,
        requester: ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        res: Sender<Result<(), String>>,
    ) {
        let result = Self::internal_add_ollama_models(
            db,
            identity_manager,
            job_manager,
            identity_secret_key,
            input_models,
            requester,
            ws_manager,
        )
        .await;
        let _ = res.send(result).await;
    }
}
